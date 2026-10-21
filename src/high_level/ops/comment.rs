use std::ops::Range;

use rowan::GreenNode;

use crate::{
    ast::{trivia_from_element, AstNode, CommentKind, Document, SlotSpan, Trivia},
    high_level::{
        error::HjsonEditError,
        ops::insert::slot_at,
        path::{HjsonCommentPath, HjsonObjectPath},
        resolve::PathResolver,
        HjsonDocument, Prefer,
    },
    syntax::{GreenElement, RedNode, SyntaxKind},
};

/// コメントの取得・設定・追加・削除。
///
/// # パスの意味
///
/// - `:header` / `:footer`: 文書全体のコメント
/// - `<path>:before`: 要素の前の行のコメント（接尾辞を省略した場合も同じ）
/// - `<path>:inline`: 要素の値の後ろのコメント
/// - `<path>:footer`: コンテナの閉じ区切りの直前のコメント（要素の後の行）
///
/// `<path>` はメンバーでも配列要素でもよい。行末でしか終端できない値
/// （引用符なし文字列）の直後にはコメントを置けないため、`:inline` は
/// [`HjsonEditError::InvalidTarget`] を返す。`:before` を使うか値を引用符で囲む。
pub trait HjsonDocumentCommentEdit {
    /// パスが指す位置のコメント。複数行は `\n` で連結して返す。
    fn get_comment(&self, path: HjsonCommentPath) -> Result<Option<String>, HjsonEditError>;
    /// コメントを差し替える（既存のスタイルは保つ）。
    fn set_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    /// コメントを追記する。
    fn append_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    /// コメントを削除する（無ければ何もしない）。
    fn remove_comment(self, path: HjsonCommentPath) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocument {
    /// 指定した位置のコメント本文。複数のコメントは並び順に返す。
    pub(crate) fn comment_bodies(
        &self,
        path: &HjsonCommentPath,
    ) -> Result<Vec<String>, HjsonEditError> {
        match resolve_comment_target(path) {
            CommentTarget::Document { header } => {
                let ast = self.ast()?;
                let trivia = if header {
                    &ast.leading_trivia
                } else {
                    &ast.trailing_trivia
                };
                Ok(extract_comment_bodies(trivia))
            }
            CommentTarget::Slot { path, location } => {
                let slot = PathResolver::new(self.syntax_root().clone()).resolve_slot(path)?;
                let trivia = slot_trivia(slot.container(), slot.index(), location)?;
                Ok(extract_comment_bodies(&trivia))
            }
            CommentTarget::ContainerFooter { path } => {
                let container = resolve_container_node(self.syntax_root(), path)?;
                let children = children_of(&container);
                let range =
                    container_footer_range(&container)?.ok_or_else(|| no_container_footer(path))?;
                Ok(comments_in(&children, range))
            }
        }
    }
}

/// パスが指すコンテナ（オブジェクト・配列）のノード。
fn resolve_container_node(
    root: &RedNode,
    path: &HjsonObjectPath,
) -> Result<RedNode, HjsonEditError> {
    let not_container = || HjsonEditError::InvalidTarget {
        path: path.clone(),
        reason: "the path is not a container".to_string(),
    };

    let resolved = PathResolver::new(root.clone()).resolve_node(path)?;
    let container =
        crate::high_level::resolve::resolve_body(&resolved).map_err(|_| not_container())?;

    matches!(container.kind(), SyntaxKind::Object | SyntaxKind::Array)
        .then_some(container)
        .ok_or_else(not_container)
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get_comment(&self, path: HjsonCommentPath) -> Result<Option<String>, HjsonEditError> {
        Ok(join_comment_bodies(self.comment_bodies(&path)?))
    }

    fn set_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let comment = comment.into();
        let previous = self.clone();
        let updated = update_comment(&self, path.clone(), Some(comment.clone()))?;

        finish_comment_edit(&previous, self.root(updated), &path, Some(comment.as_str()))
    }

    fn append_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let comment = comment.into();
        let existing = self.get_comment(path.clone())?;
        let new_comment = match existing {
            Some(existing) => format!("{}\n{}", existing, comment),
            None => comment,
        };
        self.set_comment(path, new_comment)
    }

    fn remove_comment(self, path: HjsonCommentPath) -> Result<Self, HjsonEditError> {
        let previous = self.clone();
        let updated = update_comment(&self, path.clone(), None)?;

        finish_comment_edit(&previous, self.root(updated), &path, None)
    }
}

/// 文書中のコメントが付く位置。
///
/// 「同じ本文のコメントが別の位置へ移った」ことを検出できるよう、
/// コメントをテキストの集合ではなく位置ごとに識別する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommentSlot {
    DocumentHeader,
    DocumentFooter,
    Before(HjsonObjectPath),
    Inline(HjsonObjectPath),
    ContainerFooter(HjsonObjectPath),
}

impl CommentSlot {
    /// 比較と表示のための一意なキー。
    pub(crate) fn key(&self) -> String {
        match self {
            CommentSlot::DocumentHeader => ":header".to_string(),
            CommentSlot::DocumentFooter => ":footer".to_string(),
            CommentSlot::Before(path) => format!("{path}:before"),
            CommentSlot::Inline(path) => format!("{path}:inline"),
            // 文書直下のコンテナはパスが空になるため、`:footer` と衝突しない名前にする
            CommentSlot::ContainerFooter(path) if path.segments().is_empty() => {
                ".:footer".to_string()
            }
            CommentSlot::ContainerFooter(path) => format!("{path}:footer"),
        }
    }
}

/// 文書中のコメントを、付いている位置ごとに列挙する。
///
/// 値は位置順に並ぶため、コメントの並びが変われば列も変わる。
pub(crate) fn comment_slots(
    root: &RedNode,
) -> Result<Vec<(CommentSlot, Vec<String>)>, HjsonEditError> {
    let ast = Document::cast(root.clone())?;
    let mut slots = vec![
        (
            CommentSlot::DocumentHeader,
            extract_comment_bodies(&ast.leading_trivia),
        ),
        (
            CommentSlot::DocumentFooter,
            extract_comment_bodies(&ast.trailing_trivia),
        ),
    ];

    // Root の直下は Value に包まれていることがあるため、実体のコンテナまで降りる。
    // スカラー文書にはコンテナが無いため、文書全体のコメントだけを返す。
    if let Some(container) = document_container(root) {
        collect_container_comment_slots(
            &container,
            &HjsonObjectPath::from_segments(Vec::new()),
            &mut slots,
        )?;
    }

    Ok(slots)
}

/// 文書の本体がコンテナのとき、そのノード。
fn document_container(root: &RedNode) -> Option<RedNode> {
    let mut node = crate::high_level::resolve::resolve_body(root).ok()?;
    while node.kind() == SyntaxKind::Value {
        node = crate::high_level::resolve::resolve_body(&node).ok()?;
    }

    matches!(node.kind(), SyntaxKind::Object | SyntaxKind::Array).then_some(node)
}

/// 文書中のコメント本文をすべて集める（文書順）。
///
/// スロットで位置を特定できないコメントの取りこぼしを検出するために使う。
pub(crate) fn all_comment_bodies(root: &RedNode) -> Vec<String> {
    // Comment ノードとそのトークンの両方が現れるため、トークンだけを数える
    root.descendants_with_tokens()
        .filter(|element| matches!(element, rowan::NodeOrToken::Token(_)))
        .filter_map(|element| trivia_from_element(&element))
        .filter_map(|trivia| match trivia {
            Trivia::Comment(comment) => Some(comment.body),
            _ => None,
        })
        .collect()
}

/// コンテナ以下のコメントを、内側のコンテナから順に集める。
fn collect_container_comment_slots(
    container: &RedNode,
    path: &HjsonObjectPath,
    slots: &mut Vec<(CommentSlot, Vec<String>)>,
) -> Result<(), HjsonEditError> {
    let spans = crate::high_level::ops::insert::container_slots(container)?;
    let children = children_of(container);

    for (index, span) in spans.iter().enumerate() {
        let Some(element_path) = element_path(container, index, path)? else {
            continue;
        };

        let before = comments_in(&children, span.lead());
        slots.push((CommentSlot::Before(element_path.clone()), before));

        let inline = comments_in(&children, span.trail());
        slots.push((CommentSlot::Inline(element_path.clone()), inline));

        if let Some(child) = element_container(container, index) {
            collect_container_comment_slots(&child, &element_path, slots)?;
        }
    }

    // 閉じ区切りの直前（コンテナの footer）
    if let Some(range) = container_footer_range(container)? {
        let footer = comments_in(&children, range);
        slots.push((CommentSlot::ContainerFooter(path.clone()), footer));
    }

    Ok(())
}

/// コンテナ内の要素に付くコメント本文。
fn comments_in(
    children: &[rowan::NodeOrToken<RedNode, crate::RedToken>],
    range: Range<usize>,
) -> Vec<String> {
    children[range]
        .iter()
        .filter_map(trivia_from_element)
        .filter_map(|trivia| match trivia {
            Trivia::Comment(comment) => Some(comment.body),
            _ => None,
        })
        .collect()
}

/// コンテナ内の要素のパス。要素が取れない場合は `None`。
fn element_path(
    container: &RedNode,
    index: usize,
    prefix: &HjsonObjectPath,
) -> Result<Option<HjsonObjectPath>, HjsonEditError> {
    let mut segments = prefix.segments().to_vec();

    match container.kind() {
        SyntaxKind::Object => {
            let object = crate::ast::Object::cast(container.clone())?;
            let members = object.members();
            let Some(member) = members.get(index) else {
                return Ok(None);
            };
            let Some(key) = member.member_key() else {
                return Ok(None);
            };
            segments.push(crate::high_level::path::PathSegment::Key(key));
        }
        SyntaxKind::Array => {
            segments.push(crate::high_level::path::PathSegment::Array(
                crate::high_level::path::ArrayPathSegment::Index(index),
            ));
        }
        _ => return Ok(None),
    }

    Ok(Some(HjsonObjectPath::from_segments(segments)))
}

/// コンテナ内の要素が中に持つコンテナ。
fn element_container(container: &RedNode, index: usize) -> Option<RedNode> {
    let element = match container.kind() {
        SyntaxKind::Object => {
            let object = crate::ast::Object::cast(container.clone()).ok()?;
            let member = object.members().get(index)?.syntax().clone();
            member
                .children()
                .find(|child| child.kind() == SyntaxKind::Value)?
        }
        SyntaxKind::Array => {
            let array = crate::ast::Array::cast(container.clone()).ok()?;
            array.items().get(index)?.syntax().clone()
        }
        _ => return None,
    };

    let body = crate::high_level::resolve::resolve_body(&element).ok()?;
    matches!(body.kind(), SyntaxKind::Object | SyntaxKind::Array).then_some(body)
}

/// 閉じ区切りの直前にあるコメントが占める範囲。
///
/// 最後の要素の行末より後ろから閉じ区切りまでを対象にする。要素が無い場合は
/// 本文全体（開き区切りの直後から閉じ区切りまで）を対象にする。
fn container_footer_range(container: &RedNode) -> Result<Option<Range<usize>>, HjsonEditError> {
    let spans = crate::high_level::ops::insert::container_slots(container)?;
    let children = children_of(container);

    let closing = match container.kind() {
        SyntaxKind::Object => crate::ast::Object::cast(container.clone())?.delimiter_style,
        SyntaxKind::Array => crate::ast::Array::cast(container.clone())?.delimiter_style,
        found => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found,
            })
        }
    };
    if closing == crate::ast::DelimiterStyle::Braceless {
        return Ok(None);
    }

    let closing_kind = match container.kind() {
        SyntaxKind::Object => SyntaxKind::RBrace,
        _ => SyntaxKind::RBracket,
    };
    let closing_index = children
        .iter()
        .rposition(|child| matches!(child, rowan::NodeOrToken::Token(token) if token.kind() == closing_kind))
        .unwrap_or(children.len());

    let start = match spans.last() {
        Some(last) => last.trail().end,
        None => children
            .iter()
            .position(|child| {
                matches!(
                    child,
                    rowan::NodeOrToken::Token(token)
                        if matches!(token.kind(), SyntaxKind::LBrace | SyntaxKind::LBracket)
                )
            })
            .map_or(0, |position| position + 1),
    };

    Ok(Some(start..closing_index.max(start)))
}

/// コンテナの footer が存在しない（ブレースレス）ことを示すエラー。
fn no_container_footer(path: &HjsonObjectPath) -> HjsonEditError {
    HjsonEditError::InvalidTarget {
        path: path.clone(),
        reason: "a braceless object has no closing delimiter; use :footer for the document"
            .to_string(),
    }
}

/// コメント編集の後処理（変更したコンテナの記録と検証）。
fn finish_comment_edit(
    previous: &HjsonDocument,
    updated: HjsonDocument,
    path: &HjsonCommentPath,
    expected: Option<&str>,
) -> Result<HjsonDocument, HjsonEditError> {
    if previous.current_verify() {
        crate::high_level::verify::comment_edit(previous, &updated, path, expected)?;
    }

    Ok(match comment_container(path) {
        Some(container) => updated.note_touched(container),
        None => updated,
    })
}

/// コメント操作が触れたコンテナ。ヘッダー/フッターは文書全体に付くため対象外。
fn comment_container(path: &HjsonCommentPath) -> Option<HjsonObjectPath> {
    match path {
        HjsonCommentPath::Header | HjsonCommentPath::Footer => None,
        HjsonCommentPath::Before(path) | HjsonCommentPath::Inline(path) => {
            path.split_last().map(|(parent, _)| parent)
        }
        HjsonCommentPath::ContainerFooter(path) => Some(path.clone()),
    }
}

/// コメントを付ける位置。要素の前の行か、要素の行末か。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentLocation {
    Before,
    Inline,
}

enum CommentTarget<'a> {
    Document {
        header: bool,
    },
    Slot {
        path: &'a HjsonObjectPath,
        location: CommentLocation,
    },
    /// コンテナの閉じ区切りの直前。
    ContainerFooter {
        path: &'a HjsonObjectPath,
    },
}

fn resolve_comment_target(path: &HjsonCommentPath) -> CommentTarget<'_> {
    match path {
        HjsonCommentPath::Header => CommentTarget::Document { header: true },
        HjsonCommentPath::Footer => CommentTarget::Document { header: false },
        HjsonCommentPath::Before(path) => CommentTarget::Slot {
            path,
            location: CommentLocation::Before,
        },
        HjsonCommentPath::Inline(path) => CommentTarget::Slot {
            path,
            location: CommentLocation::Inline,
        },
        HjsonCommentPath::ContainerFooter(path) => CommentTarget::ContainerFooter { path },
    }
}

/// コンテナの子要素の並び。
fn children_of(container: &RedNode) -> Vec<rowan::NodeOrToken<RedNode, crate::RedToken>> {
    container.children_with_tokens().collect()
}

/// スロットの前後にあるトリビア。
fn slot_trivia(
    container: &RedNode,
    index: usize,
    location: CommentLocation,
) -> Result<Vec<Trivia>, HjsonEditError> {
    let span = slot_at(container, index)?;
    let range = match location {
        CommentLocation::Before => span.lead(),
        CommentLocation::Inline => span.trail(),
    };

    Ok(children_of(container)[range]
        .iter()
        .filter_map(trivia_from_element)
        .collect())
}

/// 既存コメントが占める範囲。無ければ `None`。
///
/// `Before` は直前のトリビアにある最初のコメントから要素の直前まで、
/// `Inline` は要素の直後にある最初のコメント（直前の空白を含む）から行末まで。
fn comment_range(
    container: &RedNode,
    index: usize,
    location: CommentLocation,
) -> Result<Option<Range<usize>>, HjsonEditError> {
    let span = slot_at(container, index)?;
    let children = children_of(container);

    match location {
        CommentLocation::Before => {
            let lead = span.lead();
            let start = lead
                .clone()
                .find(|position| is_comment(&children[*position]));
            Ok(start.map(|start| start..lead.end))
        }
        CommentLocation::Inline => {
            let trail = span.trail();
            let Some(comment) = trail
                .clone()
                .find(|position| is_comment(&children[*position]))
            else {
                return Ok(None);
            };

            let mut start = comment;
            while start > trail.start && is_whitespace(&children[start - 1]) {
                start -= 1;
            }

            // 行末の改行は残す（置き換えで次の行を巻き込まないため）
            let mut end = comment + 1;
            while end < trail.end
                && (is_comment(&children[end])
                    || (is_whitespace(&children[end])
                        && end + 1 < trail.end
                        && is_comment(&children[end + 1])))
            {
                end += 1;
            }
            Ok(Some(start..end))
        }
    }
}

fn is_comment(element: &rowan::NodeOrToken<RedNode, crate::RedToken>) -> bool {
    match element {
        rowan::NodeOrToken::Node(node) => node.kind() == SyntaxKind::Comment,
        rowan::NodeOrToken::Token(token) => token.kind().is_comment(),
    }
}

fn is_whitespace(element: &rowan::NodeOrToken<RedNode, crate::RedToken>) -> bool {
    matches!(element, rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace)
}

fn is_comma(element: &rowan::NodeOrToken<RedNode, crate::RedToken>) -> bool {
    matches!(element, rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma)
}

fn extract_comment_bodies(trivia: &[Trivia]) -> Vec<String> {
    trivia
        .iter()
        .filter_map(|elem| match elem {
            Trivia::Comment(comment) => Some(comment.body.clone()),
            _ => None,
        })
        .collect()
}

fn join_comment_bodies(bodies: Vec<String>) -> Option<String> {
    if bodies.is_empty() {
        None
    } else {
        Some(bodies.join("\n"))
    }
}

fn make_comment_node(style: CommentKind, text: &str) -> rowan::GreenNode {
    rowan::GreenNode::new(
        SyntaxKind::Comment.into(),
        vec![kind_from_style(style)
            .create_token(render_comment_text(style, text))
            .into()],
    )
}

fn make_comment_nodes(style: CommentKind, body: &str, newline: &str) -> Vec<GreenElement> {
    if style == CommentKind::Block || !body.contains('\n') {
        return vec![make_comment_node(style, body).into()];
    }
    let mut nodes = Vec::new();
    for (i, line) in body.split('\n').enumerate() {
        if i > 0 {
            nodes.push(SyntaxKind::Newline.create_token(newline).into());
        }
        nodes.push(make_comment_node(style, line).into());
    }
    nodes
}

fn trivia_comment_style(trivia: &[Trivia]) -> Option<CommentKind> {
    trivia.iter().find_map(|elem| match elem {
        Trivia::Comment(comment) => Some(comment.kind),
        _ => None,
    })
}

fn update_comment(
    doc: &HjsonDocument,
    path: HjsonCommentPath,
    comment: Option<String>,
) -> Result<GreenNode, HjsonEditError> {
    let root = doc.syntax_root().clone();
    let preferred = doc.current_comment_style();
    let newline = doc.current_newline();

    match resolve_comment_target(&path) {
        CommentTarget::Document { header } => {
            build_document_comment(&root, header, comment, preferred, newline)
        }
        CommentTarget::Slot { path, location } => {
            build_slot_comment(root, path, location, comment, preferred, newline)
        }
        CommentTarget::ContainerFooter { path } => {
            build_container_footer_comment(root, path, comment, preferred, newline)
        }
    }
}

fn build_document_comment(
    root: &RedNode,
    header: bool,
    comment: Option<String>,
    preferred: Prefer<CommentKind>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let ast = Document::cast(root.clone())?;

    let existing_style = if header {
        trivia_comment_style(&ast.leading_trivia)
    } else {
        trivia_comment_style(&ast.trailing_trivia)
    };

    let children: Vec<_> = root.children_with_tokens().collect();

    let (mut green, insert_at) = if header {
        let leading = &ast.leading_trivia;
        if let Some(start) = leading.iter().position(|e| matches!(e, Trivia::Comment(_))) {
            let range = start..leading.len();
            let green = root.green().splice_children(range.clone(), vec![]);
            (green, range.start)
        } else {
            (root.green().clone().into_owned(), leading.len())
        }
    } else {
        let trailing = &ast.trailing_trivia;
        if let Some(start) = trailing
            .iter()
            .position(|e| matches!(e, Trivia::Comment(_)))
        {
            let mut end = trailing
                .iter()
                .rposition(|e| matches!(e, Trivia::Comment(_)))
                .expect("rposition succeeds because position() found at least one comment")
                + 1;
            if end < trailing.len() && matches!(trailing[end], Trivia::Newline { .. }) {
                end += 1;
            }
            let root_start = children.len().saturating_sub(trailing.len());
            let range = (root_start + start)..(root_start + end);
            let green = root.green().splice_children(range.clone(), vec![]);
            (green, range.start)
        } else {
            (root.green().clone().into_owned(), children.len())
        }
    };

    if let Some(comment) = comment {
        let resolved_style = resolve_comment_style(preferred, existing_style);
        let comment_elements = make_comment_nodes(resolved_style, &comment, newline);

        let mut inserted: Vec<GreenElement> = comment_elements;
        if header {
            inserted.push(SyntaxKind::Newline.create_token(newline).into());
        }
        green = green.splice_children(insert_at..insert_at, inserted);
    }

    Ok(green)
}

/// スロットの前後のコメントを差し替える。`comment` が `None` なら削除する。
fn build_slot_comment(
    root: RedNode,
    path: &HjsonObjectPath,
    location: CommentLocation,
    comment: Option<String>,
    preferred: Prefer<CommentKind>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let slot = PathResolver::new(root).resolve_slot(path)?;
    let container = slot.container().clone();
    let index = slot.index();
    let span = slot_at(&container, index)?;

    let existing_style = trivia_comment_style(&slot_trivia(&container, index, location)?);
    let existing_range = comment_range(&container, index, location)?;
    let green_after_remove = match &existing_range {
        Some(range) => container.green().splice_children(range.clone(), vec![]),
        None => container.green().clone().into_owned(),
    };

    let Some(comment) = comment else {
        return Ok(container.replace_with(green_after_remove));
    };

    let resolved_style = resolve_comment_style(preferred, existing_style);
    let children = children_of(&container);

    let (insert_at, inserted) = match location {
        CommentLocation::Before => {
            let mut inserted = make_comment_nodes(resolved_style, &comment, newline);
            inserted.push(SyntaxKind::Newline.create_token(newline).into());
            // 要素の字下げを新しい行にも引き継ぐ
            if let Some(indent) = trailing_whitespace(&children, &span) {
                inserted.push(indent);
            }
            let at = existing_range
                .as_ref()
                .map(|range| range.start)
                .unwrap_or_else(|| span.elem().start);
            (at, inserted)
        }
        CommentLocation::Inline => {
            // 引用符なし文字列は行末までを飲み込むため、その直後にコメントを置けない。
            if children
                .get(span.elem().start)
                .is_some_and(ends_with_unquoted_string)
            {
                return Err(HjsonEditError::InvalidTarget {
                    path: path.clone(),
                    reason: "an inline comment cannot follow an unquoted string value; \
                             quote the value or use the :before suffix"
                        .to_string(),
                });
            }

            // 行末でない位置では行コメントが後続を飲み込むため、ブロックコメントにする。
            // 複数行の本文も 1 つのブロックコメントにまとめる（行の構造を変えない）。
            let style = if span.line_break().is_some() && !comment.contains('\n') {
                resolved_style
            } else {
                CommentKind::Block
            };
            let mut inserted = vec![SyntaxKind::Whitespace.create_token(" ").into()];
            inserted.extend(make_comment_nodes(style, &comment, newline));

            let at = match &existing_range {
                Some(range) => range.start,
                None => {
                    // 区切りカンマがある場合はその直後に置く
                    let mut at = span.trail().start;
                    if children.get(at).is_some_and(is_comma) {
                        at += 1;
                    }
                    at
                }
            };
            (at, inserted)
        }
    };

    let updated = green_after_remove.splice_children(insert_at..insert_at, inserted);
    Ok(container.replace_with(updated))
}

/// コンテナの閉じ区切りの直前にあるコメントを差し替える。`None` なら削除する。
fn build_container_footer_comment(
    root: RedNode,
    path: &HjsonObjectPath,
    comment: Option<String>,
    preferred: Prefer<CommentKind>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let container = resolve_container_node(&root, path)?;
    let children = children_of(&container);
    let range = container_footer_range(&container)?.ok_or_else(|| no_container_footer(path))?;

    let trivia: Vec<Trivia> = children[range.clone()]
        .iter()
        .filter_map(trivia_from_element)
        .collect();
    let existing_style = trivia_comment_style(&trivia);

    // 閉じ区切りの手前は、要素の行末より後ろにあるトリビア（コメント・改行・空白）だけ。
    // コメント行の行頭から閉じ区切りまでを一度に置き換え、体裁を作り直す。
    let first_comment = range.clone().position(|index| is_comment(&children[index]));
    let deletion_start = match first_comment {
        Some(index) => {
            let mut start = range.start + index;
            while start > range.start && is_whitespace(&children[start - 1]) {
                start -= 1;
            }
            start
        }
        None => {
            let mut at = range.end;
            while at > range.start && is_whitespace(&children[at - 1]) {
                at -= 1;
            }
            at
        }
    };
    // 置き換える範囲が無く、消すコメントも無ければ何もしない
    if comment.is_none() && deletion_start == range.end {
        return Ok(container.replace_with(container.green().clone().into_owned()));
    }

    let at_line_start = deletion_start > 0 && is_newline(&children[deletion_start - 1]);

    let mut inserted: Vec<GreenElement> = Vec::new();
    if let Some(comment) = comment {
        let resolved_style = resolve_comment_style(preferred, existing_style);
        // コメントは要素の後の行に置くため、直前が行末でなければ改行を足す
        if !at_line_start {
            inserted.push(SyntaxKind::Newline.create_token(newline).into());
        }
        inserted.push(element_indent(&children, &container)?);
        inserted.extend(make_comment_nodes(resolved_style, &comment, newline));
        inserted.push(SyntaxKind::Newline.create_token(newline).into());
    }

    // 閉じ区切りの字下げは元のテキストを保つ。行頭に無ければ開き区切りの行に合わせる。
    let closing_indent = closing_indent_of(&children, &range, &container);
    if !closing_indent.is_empty() {
        inserted.push(
            SyntaxKind::Whitespace
                .create_token(closing_indent.as_str())
                .into(),
        );
    }

    let updated = container
        .green()
        .splice_children(deletion_start..range.end, inserted);
    Ok(container.replace_with(updated))
}

/// 閉じ区切りの直前にある字下げ。
fn closing_indent_of(
    children: &[rowan::NodeOrToken<RedNode, crate::RedToken>],
    range: &Range<usize>,
    container: &RedNode,
) -> String {
    let mut text = String::new();
    let mut at = range.end;
    while at > range.start && is_whitespace(&children[at - 1]) {
        at -= 1;
        text = format!("{}{}", children[at], text);
    }

    // 閉じ区切りが元から行頭にある場合は字下げをそのまま保つ。
    // 同じ行に続いている場合は、コンテナの行の字下げに合わせる。
    let at_line_start = at > 0 && is_newline(&children[at - 1]);
    if at_line_start {
        text
    } else {
        crate::high_level::indent::line_indent(
            &crate::high_level::indent::root_of(container),
            container.text_range().start().into(),
        )
    }
}

/// コンテナ内の要素の字下げ。
///
/// 要素があればその手前の空白を使う。空のコンテナでは、コンテナ自身の行の
/// 字下げに文書のインデント 1 段を足したものを使う。
fn element_indent(
    children: &[rowan::NodeOrToken<RedNode, crate::RedToken>],
    container: &RedNode,
) -> Result<GreenElement, HjsonEditError> {
    let spans = crate::high_level::ops::insert::container_slots(container)?;

    if let Some(last) = spans.last() {
        let lead = last.lead();
        let index = lead
            .clone()
            .rev()
            .find(|index| is_whitespace(&children[*index]));
        if let Some(index) = index {
            return Ok(to_green_element(&children[index]));
        }
    }

    let indent = crate::high_level::indent::empty_container_indent(container);

    Ok(SyntaxKind::Whitespace.create_token(indent.as_str()).into())
}

fn is_newline(element: &rowan::NodeOrToken<RedNode, crate::RedToken>) -> bool {
    matches!(element, rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline)
}

/// スロットの直前にある字下げ（空白）。
fn trailing_whitespace(
    children: &[rowan::NodeOrToken<RedNode, crate::RedToken>],
    span: &SlotSpan,
) -> Option<GreenElement> {
    let lead = span.lead();
    let last = lead
        .end
        .checked_sub(1)
        .filter(|index| *index >= lead.start)?;
    let element = children.get(last)?;
    is_whitespace(element).then(|| to_green_element(element))
}

/// 要素の末尾が引用符なし文字列か。
fn ends_with_unquoted_string(element: &rowan::NodeOrToken<RedNode, crate::RedToken>) -> bool {
    match element {
        rowan::NodeOrToken::Node(node) => node
            .last_token()
            .is_some_and(|token| token.kind() == SyntaxKind::UnquotedString),
        rowan::NodeOrToken::Token(token) => token.kind() == SyntaxKind::UnquotedString,
    }
}

/// 赤ノードの要素を green 要素にする。
fn to_green_element(element: &rowan::NodeOrToken<RedNode, crate::RedToken>) -> GreenElement {
    match element {
        rowan::NodeOrToken::Node(node) => node.green().clone().into(),
        rowan::NodeOrToken::Token(token) => token.kind().create_token(token.text()).into(),
    }
}

fn kind_from_style(style: CommentKind) -> SyntaxKind {
    match style {
        CommentKind::Block => SyntaxKind::BlockComment,
        CommentKind::Line => SyntaxKind::LineComment,
        CommentKind::Hash => SyntaxKind::HashComment,
    }
}

fn resolve_comment_style(
    preferred: Prefer<CommentKind>,
    existing_style: Option<CommentKind>,
) -> CommentKind {
    match preferred {
        Prefer::Preserve => existing_style.unwrap_or(CommentKind::Block),
        Prefer::Explicit(kind) => kind,
    }
}

fn render_comment_text(style: CommentKind, comment: &str) -> String {
    match style {
        CommentKind::Line => {
            if comment.is_empty() {
                String::from("//")
            } else {
                format!("// {}", comment)
            }
        }
        CommentKind::Hash => {
            if comment.is_empty() {
                String::from("#")
            } else {
                format!("# {}", comment)
            }
        }
        CommentKind::Block => format!("/* {} */", comment),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::CommentTrivia;
    use crate::high_level::path::HjsonObjectPath;

    #[test]
    fn test_render_comment_text() {
        assert_eq!(render_comment_text(CommentKind::Line, "hello"), "// hello");
        assert_eq!(render_comment_text(CommentKind::Line, ""), "//");
        assert_eq!(render_comment_text(CommentKind::Hash, "hello"), "# hello");
        assert_eq!(render_comment_text(CommentKind::Hash, ""), "#");
        assert_eq!(
            render_comment_text(CommentKind::Block, "hello"),
            "/* hello */"
        );
    }

    #[test]
    fn test_kind_from_style() {
        assert_eq!(kind_from_style(CommentKind::Line), SyntaxKind::LineComment);
        assert_eq!(kind_from_style(CommentKind::Hash), SyntaxKind::HashComment);
        assert_eq!(
            kind_from_style(CommentKind::Block),
            SyntaxKind::BlockComment
        );
    }

    #[test]
    fn test_resolve_comment_style_explicit_respects_multiline_line() {
        assert_eq!(
            resolve_comment_style(Prefer::Explicit(CommentKind::Line), None),
            CommentKind::Line
        );
    }

    #[test]
    fn test_resolve_comment_style_preserve_falls_back_to_block() {
        assert_eq!(
            resolve_comment_style(Prefer::Preserve, None),
            CommentKind::Block
        );
    }

    #[test]
    fn test_resolve_comment_style_preserve_keeps_existing() {
        assert_eq!(
            resolve_comment_style(Prefer::Preserve, Some(CommentKind::Line)),
            CommentKind::Line
        );
    }

    #[test]
    fn test_resolve_comment_style_explicit_overrides() {
        assert_eq!(
            resolve_comment_style(Prefer::Explicit(CommentKind::Hash), None),
            CommentKind::Hash
        );
    }

    #[test]
    fn test_extract_comment_bodies_mixed_trivia() {
        let trivia = vec![
            Trivia::Whitespace("  ".to_string()),
            Trivia::Comment(CommentTrivia {
                kind: CommentKind::Line,
                text: "// first".to_string(),
                body: "first".to_string(),
            }),
            Trivia::Comment(CommentTrivia {
                kind: CommentKind::Block,
                text: "/* second */".to_string(),
                body: "second".to_string(),
            }),
        ];
        assert_eq!(
            extract_comment_bodies(&trivia),
            vec!["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn test_extract_comment_bodies_no_comments() {
        let trivia = vec![Trivia::Whitespace("  ".to_string())];
        assert!(extract_comment_bodies(&trivia).is_empty());
    }

    #[test]
    fn test_join_comment_bodies_empty_trivia() {
        let trivia = vec![Trivia::Whitespace("  ".to_string())];
        assert_eq!(join_comment_bodies(extract_comment_bodies(&trivia)), None);
    }

    #[test]
    fn test_join_comment_bodies_single_comment() {
        let trivia = vec![Trivia::Comment(CommentTrivia {
            kind: CommentKind::Line,
            text: "// hello".to_string(),
            body: "hello".to_string(),
        })];
        assert_eq!(
            join_comment_bodies(extract_comment_bodies(&trivia)),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_join_comment_bodies_multiple_comments() {
        let trivia = vec![
            Trivia::Comment(CommentTrivia {
                kind: CommentKind::Line,
                text: "// first".to_string(),
                body: "first".to_string(),
            }),
            Trivia::Comment(CommentTrivia {
                kind: CommentKind::Block,
                text: "/* second */".to_string(),
                body: "second".to_string(),
            }),
        ];
        assert_eq!(
            join_comment_bodies(extract_comment_bodies(&trivia)),
            Some("first\nsecond".to_string())
        );
    }

    #[test]
    fn test_trivia_comment_style() {
        let with_comments = vec![
            Trivia::Whitespace("  ".to_string()),
            Trivia::Comment(CommentTrivia {
                kind: CommentKind::Line,
                text: "// hi".to_string(),
                body: "hi".to_string(),
            }),
            Trivia::Comment(CommentTrivia {
                kind: CommentKind::Block,
                text: "/* there */".to_string(),
                body: "there".to_string(),
            }),
        ];
        assert_eq!(
            trivia_comment_style(&with_comments),
            Some(CommentKind::Line)
        );
        let no_comments = vec![Trivia::Whitespace("  ".to_string())];
        assert_eq!(trivia_comment_style(&no_comments), None);
        assert_eq!(trivia_comment_style(&[]), None);
    }

    #[test]
    fn test_resolve_comment_target_header() {
        match resolve_comment_target(&HjsonCommentPath::Header) {
            CommentTarget::Document { header: true } => {}
            _ => panic!("expected Document header"),
        }
    }

    #[test]
    fn test_resolve_comment_target_footer() {
        match resolve_comment_target(&HjsonCommentPath::Footer) {
            CommentTarget::Document { header: false } => {}
            _ => panic!("expected Document footer"),
        }
    }

    #[test]
    fn test_resolve_comment_target_before() {
        let path = HjsonCommentPath::Before(HjsonObjectPath::parse("a").unwrap());
        match resolve_comment_target(&path) {
            CommentTarget::Slot {
                location: CommentLocation::Before,
                ..
            } => {}
            _ => panic!("expected Slot Before"),
        }
    }

    #[test]
    fn test_resolve_comment_target_inline() {
        let path = HjsonCommentPath::Inline(HjsonObjectPath::parse("a").unwrap());
        match resolve_comment_target(&path) {
            CommentTarget::Slot {
                location: CommentLocation::Inline,
                ..
            } => {}
            _ => panic!("expected Slot Inline"),
        }
    }
}
