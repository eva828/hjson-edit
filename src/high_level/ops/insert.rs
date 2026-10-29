//! コンテナへの要素挿入を 1 か所に集約する。
//!
//! HJSON では行末でしか終端できない値（引用符なし文字列など）があり、その直後に
//! カンマや閉じ区切りを置くとそれらが値に吸収されて文書が壊れる。この性質を扱うのは
//! このモジュールだけとし、`set` / `merge` / コメント操作はここが返す挿入位置と
//! 要素列をそのまま使う。
//!
//! 挿入位置の決定は次の規則に従う。
//!
//! 1. 直前の要素の行が改行で終わっているときは、その改行の直後（次の行頭）へ入れる。
//!    既存の改行が挿入要素の行を終端するため、足す改行は要素の後ろだけ。
//! 2. 直前の要素と同じ行に入れる場合、要素が行末でしか終端できないなら前後を改行で囲む。
//!    そうでなければ区切り（カンマ）を補う。
//! 3. 本文が空のときは閉じ区切りの直前（ブレースレスは本文末尾）へ入れ、
//!    閉じ区切りが独立した行にあるなら要素の後ろに改行を足す。
//! 4. 閉じ区切りの直前にあるコメント（footer コメント）は要素の後ろに残す。

use std::ops::Range;

use rowan::NodeOrToken;

use crate::ast::{
    Array as AstArray, AstNode, BodyInsertion, DelimiterStyle, Object as AstObject, SlotSpan,
};
use crate::high_level::error::HjsonEditError;
use crate::high_level::indent::{
    container_indent, indent_before, indent_lines, line_indent, root_of,
};
use crate::high_level::path::HjsonObjectPath;
use crate::syntax::{GreenElement, NewlineKind, RedNode, SyntaxKind};

/// レンダリング済みの要素（メンバーまたは配列要素）。
///
/// コメントを伴う要素（マージでコメントごとコピーする場合）もあるため、
/// 要素は複数の green 要素の並びとして持つ。
#[derive(Clone)]
pub(crate) struct RenderedElement {
    elements: Vec<GreenElement>,
    line_terminated: bool,
    has_leading_comments: bool,
}

impl RenderedElement {
    /// 単一の green 要素から作る。
    pub(crate) fn single(green: GreenElement) -> Self {
        let line_terminated = ends_with_line_token(&green);
        Self {
            elements: vec![green],
            line_terminated,
            has_leading_comments: false,
        }
    }

    /// 並び済みの green 要素から作る（先頭コメントの有無を呼び出し側が指定する）。
    pub(crate) fn from_parts(elements: Vec<GreenElement>, has_leading_comments: bool) -> Self {
        let line_terminated = elements.last().is_some_and(ends_with_line_token);
        Self {
            elements,
            line_terminated,
            has_leading_comments,
        }
    }
}

/// 要素を挿入する位置。
pub(crate) enum InsertPosition {
    /// index の要素の直前。
    Before(usize),
    /// コンテナの末尾（閉じ区切りの直前にあるコメントの手前）。
    End,
}

/// 要素を挿入する位置と要素列を返す。
///
/// `newline` はコンテナに改行が 1 つも無い場合に使う既定の改行コード。
///
/// `base_indent` はコンテナの行の字下げ。文書に繋がっていないコンテナ
/// （マージの途中など）では呼び出し側が渡す。
pub(crate) fn plan_insert(
    container: &RedNode,
    position: InsertPosition,
    element: RenderedElement,
    newline: NewlineKind,
    base_indent: Option<&str>,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let slots = container_slots(container)?;
    let braceless = is_braceless(container)?;
    let base_indent = base_indent.map_or_else(
        || line_indent(&root_of(container), container.text_range().start().into()),
        str::to_string,
    );
    // 混在した改行コードの文書でも、コンテナ内で使われている改行に合わせる
    let newline = container_newline(container).unwrap_or(newline);

    match position {
        InsertPosition::End => {
            plan_append(container, &slots, element, newline, braceless, &base_indent)
        }
        InsertPosition::Before(index) => plan_before(
            container,
            &slots,
            index,
            element,
            newline,
            braceless,
            &base_indent,
        ),
    }
}

/// コンテナ直下で最初に現れる改行の種類。
fn container_newline(container: &RedNode) -> Option<NewlineKind> {
    container
        .children_with_tokens()
        .find_map(|child| match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                NewlineKind::from_text(token.text())
            }
            _ => None,
        })
}

/// 指定 index の子要素の改行の種類。
fn newline_at(container: &RedNode, index: usize) -> Option<NewlineKind> {
    container
        .children_with_tokens()
        .nth(index)
        .and_then(|child| match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                NewlineKind::from_text(token.text())
            }
            _ => None,
        })
}

/// 要素本体だけを差し替える位置と要素を返す。
pub(crate) fn plan_replace(
    container: &RedNode,
    index: usize,
    element: RenderedElement,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let slot = slot_at(container, index)?;
    Ok((slot.elem().start, element.elements))
}

/// 要素をトリビアごと削除する範囲を返す。
pub(crate) fn plan_remove(
    container: &RedNode,
    index: usize,
) -> Result<Range<usize>, HjsonEditError> {
    Ok(slot_at(container, index)?.span())
}

/// コンテナの要素位置を返す。
pub(crate) fn container_slots(container: &RedNode) -> Result<Vec<SlotSpan>, HjsonEditError> {
    match container.kind() {
        SyntaxKind::Object => Ok(AstObject::cast(container.clone())?.slots()),
        SyntaxKind::Array => Ok(AstArray::cast(container.clone())?.slots()),
        found => Err(HjsonEditError::UnexpectedStructure {
            expected: SyntaxKind::Object,
            found,
        }),
    }
}

/// コンテナ内の要素位置を 1 つ返す。
pub(crate) fn slot_at(container: &RedNode, index: usize) -> Result<SlotSpan, HjsonEditError> {
    let slots = container_slots(container)?;
    let len = slots.len();
    slots
        .get(index)
        .cloned()
        .ok_or_else(|| HjsonEditError::InvalidTarget {
            path: HjsonObjectPath::from_segments(Vec::new()),
            reason: format!("element index {index} is out of bounds (length {len})"),
        })
}

/// 末尾へ要素を追加する。
fn plan_append(
    container: &RedNode,
    slots: &[SlotSpan],
    element: RenderedElement,
    newline: NewlineKind,
    braceless: bool,
    base_indent: &str,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let RenderedElement {
        elements,
        line_terminated,
        has_leading_comments,
    } = element;

    let Some(last) = slots.last() else {
        return plan_append_to_empty_body(
            container,
            RenderedElement {
                elements,
                line_terminated,
                has_leading_comments,
            },
            newline,
            braceless,
            base_indent,
        );
    };

    // 直前の要素の行が改行で終わっているなら、その次の行頭へ入れる。
    // 既存の改行が挿入要素の行を終端するため、足すのは要素の後ろの改行だけ。
    if let Some(line_break) = last.line_break() {
        let kind = newline_at(container, line_break).unwrap_or(newline);
        let indent = indent_before(
            &root_of(container),
            child_start(container, last.elem().start),
        );
        let mut inserted = elements;
        inserted.push(newline_element(kind));
        return Ok((line_break + 1, indent_lines(inserted, &indent)));
    }

    let insert_index = last.trail().end;

    // 先頭にコメント行を持つ要素は行の途中に置けないため、前後を改行で囲む。
    if has_leading_comments {
        let indent = inline_insertion_indent(container, insert_index, base_indent);
        return Ok((insert_index, start_line(elements, newline, &indent)));
    }

    if braceless {
        // ブレースレスの本文は行単位なので、前に改行を入れて独立した行にする。
        let mut inserted = vec![newline_element(newline)];
        inserted.extend(elements);
        return Ok((insert_index, inserted));
    }

    // 直前の要素と同じ行に入れる。
    let mut inserted = Vec::new();
    if !has_separator_between(container, last.elem().end, insert_index) {
        inserted.extend(separator_elements_of(&separator_gap(container)));
    }
    inserted.extend(elements);
    if line_terminated {
        inserted.push(newline_element(newline));
    }

    Ok((insert_index, inserted))
}

/// 末尾へ複数要素をまとめて追加する。
///
/// 複数要素をまとめて扱える形（複数行のコンテナ・ブレースレス・閉じ区切りが独立した行に
/// ある空のコンテナ）では、位置と字下げを 1 回だけ決めて全要素を並べる。1 行のコンテナなど
/// 並べ方に例外がある形では `None` を返し、呼び出し側が 1 件ずつ追加する。
pub(crate) fn plan_append_many(
    container: &RedNode,
    elements: Vec<RenderedElement>,
    newline: NewlineKind,
    base_indent: Option<&str>,
) -> Result<Option<(usize, Vec<GreenElement>)>, HjsonEditError> {
    let slots = container_slots(container)?;
    let braceless = is_braceless(container)?;
    let base_indent = base_indent.map_or_else(
        || line_indent(&root_of(container), container.text_range().start().into()),
        str::to_string,
    );
    let newline = container_newline(container).unwrap_or(newline);

    let Some(last) = slots.last() else {
        let insertion = body_insertion(container)?;
        if braceless || insertion.start_new_line() {
            return Ok(None);
        }
        let (index, at_line_start) = empty_body_insertion(container, insertion.index());
        if !at_line_start {
            return Ok(None);
        }

        let indent = container_indent(container, &base_indent);
        let mut inserted = Vec::new();
        for element in elements {
            let mut block = element.elements;
            block.push(newline_element(newline));
            inserted.extend(indent_lines(block, &indent));
        }
        return Ok(Some((index, inserted)));
    };

    if let Some(line_break) = last.line_break() {
        let kind = newline_at(container, line_break).unwrap_or(newline);
        let indent = indent_before(
            &root_of(container),
            child_start(container, last.elem().start),
        );
        let mut inserted = Vec::new();
        for element in elements {
            let mut block = element.elements;
            block.push(newline_element(kind));
            inserted.extend(indent_lines(block, &indent));
        }
        return Ok(Some((line_break + 1, inserted)));
    }

    if braceless {
        let mut inserted = Vec::new();
        for element in elements {
            inserted.push(newline_element(newline));
            inserted.extend(element.elements);
        }
        return Ok(Some((last.trail().end, inserted)));
    }

    Ok(None)
}

/// 本文が空のコンテナへ要素を追加する。
fn plan_append_to_empty_body(
    container: &RedNode,
    element: RenderedElement,
    newline: NewlineKind,
    braceless: bool,
    base_indent: &str,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let RenderedElement {
        elements,
        line_terminated,
        has_leading_comments,
    } = element;

    let insertion = body_insertion(container)?;
    let (index, at_line_start) = empty_body_insertion(container, insertion.index());

    let mut body = elements;
    if !braceless && (insertion.terminate_line() || line_terminated || has_leading_comments) {
        body.push(newline_element(newline));
    }

    let mut inserted = Vec::new();
    if insertion.start_new_line() && !at_line_start {
        inserted.push(newline_element(newline));
    }
    if at_line_start {
        // 閉じ区切りが独立した行にある場合は、その 1 段上に字下げして置く
        inserted.extend(indent_lines(
            body,
            &container_indent(container, base_indent),
        ));
    } else {
        inserted.extend(body);
    }

    Ok((index, inserted))
}

/// 既存要素の直前へ挿入する。
fn plan_before(
    container: &RedNode,
    slots: &[SlotSpan],
    index: usize,
    element: RenderedElement,
    newline: NewlineKind,
    braceless: bool,
    base_indent: &str,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let target = slots
        .get(index)
        .ok_or_else(|| HjsonEditError::InvalidTarget {
            path: HjsonObjectPath::from_segments(Vec::new()),
            reason: format!(
                "element index {index} is out of bounds (length {})",
                slots.len()
            ),
        })?
        .clone();

    let RenderedElement {
        elements,
        line_terminated,
        has_leading_comments,
    } = element;

    let insert_index = target.insert_before();

    if braceless || target.at_line_start() {
        // 対象は行頭から始まる。挿入要素をその 1 行前に入れ、対象を次の行に残す。
        let indent = if braceless {
            String::new()
        } else {
            indent_before(
                &root_of(container),
                child_start(container, target.elem().start),
            )
        };
        let mut inserted = elements;
        inserted.push(newline_element(newline));
        return Ok((insert_index, indent_lines(inserted, &indent)));
    }

    if has_leading_comments {
        // 先頭にコメント行を持つ要素は行の途中に置けないため、前後を改行で囲む。
        let indent = inline_insertion_indent(container, insert_index, base_indent);
        return Ok((insert_index, start_line(elements, newline, &indent)));
    }

    // 同一行への挿入。直前の要素との区切り（カンマ）は既にあるものを使い回す。
    if index > 0 {
        let previous = &slots[index - 1];
        if !line_terminated && has_separator_between(container, previous.elem().end, insert_index) {
            // 既存のカンマの直後へ入れて、区切りの並びを崩さない
            let gap = separator_whitespace(container, previous.elem().end, insert_index);
            let mut inserted = separator_elements_of(&gap);
            inserted.extend(elements);
            return Ok((previous.elem().end, inserted));
        }

        if !has_separator_between(container, previous.elem().end, insert_index) {
            let gap = separator_gap(container);
            let mut inserted = separator_elements_of(&gap);
            inserted.extend(elements);
            if line_terminated {
                inserted.push(newline_element(newline));
            } else {
                inserted.extend(separator_elements_of(&gap));
            }
            return Ok((insert_index, inserted));
        }
    }

    let gap = separator_gap(container);
    let mut inserted = Vec::new();
    inserted.extend(elements);
    if line_terminated {
        inserted.push(newline_element(newline));
    } else {
        inserted.extend(separator_elements_of(&gap));
    }

    Ok((insert_index, inserted))
}

/// 子要素の開始テキスト位置。
fn child_start(container: &RedNode, index: usize) -> usize {
    container.children_with_tokens().nth(index).map_or_else(
        || container.text_range().end().into(),
        |child| child.text_range().start().into(),
    )
}

/// 空の本文への挿入位置。閉じ区切りが独立した行にある場合は、行頭（字下げの手前）へ入れる。
fn empty_body_insertion(container: &RedNode, closing_index: usize) -> (usize, bool) {
    let children = container.children_with_tokens().collect::<Vec<_>>();
    let mut at = closing_index;
    while at > 0 && is_whitespace_element(&children[at - 1]) {
        at -= 1;
    }

    let at_line_start = at > 0 && is_newline_element(&children[at - 1]);
    (
        if at_line_start { at } else { closing_index },
        at_line_start,
    )
}

fn is_whitespace_element(element: &NodeOrToken<RedNode, crate::RedToken>) -> bool {
    matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace)
}

fn is_newline_element(element: &NodeOrToken<RedNode, crate::RedToken>) -> bool {
    matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline)
}

/// 行の途中に置けない要素を入れる行の字下げ。
///
/// すでに字下げのある行ならそれを使い、単一行のコンテナなら
/// コンテナの行の字下げに 1 段を足したものを使う。
fn inline_insertion_indent(container: &RedNode, insert_index: usize, base_indent: &str) -> String {
    if !container.text().to_string().contains('\n') {
        return container_indent(container, base_indent);
    }

    line_indent(&root_of(container), child_start(container, insert_index))
}

/// 行の途中に置けない要素を、独立した行（字下げ付き）に置く。
fn start_line(
    mut elements: Vec<GreenElement>,
    newline: NewlineKind,
    indent: &str,
) -> Vec<GreenElement> {
    elements.push(newline_element(newline));
    let mut inserted = vec![newline_element(newline)];
    inserted.extend(indent_lines(elements, indent));
    inserted
}

/// コンテナが閉じ区切りを持たない（ブレースレス）か。
fn is_braceless(container: &RedNode) -> Result<bool, HjsonEditError> {
    let style = match container.kind() {
        SyntaxKind::Object => AstObject::cast(container.clone())?.delimiter_style,
        SyntaxKind::Array => AstArray::cast(container.clone())?.delimiter_style,
        found => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found,
            })
        }
    };
    Ok(style == DelimiterStyle::Braceless)
}

/// 本文が空のコンテナへ最初の要素を挿入する位置。
fn body_insertion(container: &RedNode) -> Result<BodyInsertion, HjsonEditError> {
    match container.kind() {
        SyntaxKind::Object => Ok(AstObject::cast(container.clone())?.body_insertion()),
        SyntaxKind::Array => Ok(AstArray::cast(container.clone())?.body_insertion()),
        found => Err(HjsonEditError::UnexpectedStructure {
            expected: SyntaxKind::Object,
            found,
        }),
    }
}

/// `from..to` の子要素に区切り（カンマまたは改行）があるか。
fn has_separator_between(container: &RedNode, from: usize, to: usize) -> bool {
    container
        .children_with_tokens()
        .skip(from)
        .take(to.saturating_sub(from))
        .any(|child| {
            matches!(
                child,
                NodeOrToken::Token(token)
                    if matches!(token.kind(), SyntaxKind::Comma | SyntaxKind::Newline)
            )
        })
}

/// 要素の末尾トークンが行末でしか終端できないか。
///
/// 引用符なし文字列と行コメントは行末までを飲み込むため、直後にトークンを置けない。
fn ends_with_line_token(element: &GreenElement) -> bool {
    let red = match element {
        NodeOrToken::Token(token) => RedNode::new_root(rowan::GreenNode::new(
            SyntaxKind::Value.into(),
            vec![token.clone().into()],
        )),
        NodeOrToken::Node(node) => RedNode::new_root(node.clone()),
    };

    red.last_token().is_some_and(|token| {
        matches!(
            token.kind(),
            SyntaxKind::UnquotedString | SyntaxKind::LineComment | SyntaxKind::HashComment
        )
    })
}

pub(crate) fn newline_element(kind: NewlineKind) -> GreenElement {
    SyntaxKind::Newline.create_token(kind.as_str()).into()
}

/// カンマと、その直後の空白。
///
/// 空白はコンテナで使われている区切りの体裁に合わせる（`[1, 2]` と `[1,2]` の両方）。
fn separator_elements_of(gap: &str) -> Vec<GreenElement> {
    let mut elements = vec![comma_element()];
    if !gap.is_empty() {
        elements.push(crate::high_level::indent::whitespace_element(gap));
    }

    elements
}

/// コンテナの区切りの直後にある空白。最初のカンマを手掛かりにする。
fn separator_gap(container: &RedNode) -> String {
    let children = container.children_with_tokens().collect::<Vec<_>>();
    let Some(position) = children.iter().position(
        |child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma),
    ) else {
        return " ".to_string();
    };

    let mut gap = String::new();
    for child in &children[position + 1..] {
        match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace => {
                gap.push_str(token.text());
            }
            _ => break,
        }
    }

    gap
}

/// 指定した子要素の範囲にあるカンマの直後の空白。
///
/// 空白は挿入位置そのもの（範囲の外）にあるため、範囲の終端では打ち切らない。
fn separator_whitespace(container: &RedNode, from: usize, to: usize) -> String {
    let children = container.children_with_tokens().collect::<Vec<_>>();
    let Some(position) = children[from..to].iter().position(
        |child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma),
    ) else {
        return String::new();
    };

    let mut gap = String::new();
    for child in &children[from + position + 1..] {
        match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace => {
                gap.push_str(token.text());
            }
            _ => break,
        }
    }

    gap
}

fn comma_element() -> GreenElement {
    SyntaxKind::Comma
        .create_token(SyntaxKind::Comma.canonical_text())
        .into()
}
