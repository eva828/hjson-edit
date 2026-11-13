//! 編集結果の検証。
//!
//! 編集は「最小限のテキスト変更」で行うため、結果が妥当かどうかを必ず検査する。
//!
//! - 編集後のテキストが再パースできること
//! - 値が「元の値に同じ操作を適用した結果」と一致すること（値の層の操作を基準にする）
//! - コメント操作では、対象以外のコメントが変化していないこと
//!
//! 検証に失敗した場合は編集結果を返さず [`HjsonEditError::InvalidEdit`] を返す。

use rowan::{NodeOrToken, TextRange};

use crate::high_level::error::{format_text, HjsonEditError};
use crate::high_level::ops::comment::{all_comment_bodies, comment_slots, CommentSlot};
use crate::high_level::ops::insert::slot_at;
use crate::high_level::path::{ArrayPathSegment, HjsonCommentPath, HjsonObjectPath, PathSegment};
use crate::high_level::resolve::{array_slot_index, member_index, resolve_body, PathResolver};
use crate::high_level::value::HjsonValue;
use crate::high_level::value_ops::values_equal;
use crate::high_level::HjsonDocument;
use crate::syntax::{RedNode, SyntaxKind};

/// コメントの変化を許す範囲。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditedRegion {
    /// `path` が指すノードの部分木（値の置き換え・追加・マージ）。
    /// 要素の前後のコメントは変わってはならない。
    Node,
    /// 要素本体とその前後のトリビア（削除ではトリビアも一緒に消える）。
    Slot,
}

/// 値の編集を検証する。
///
/// `expected` は、編集前の値に同じ操作を適用した結果を返す関数（オラクル）。
/// 併せて、編集対象の外側のコメントが変化していないことも確認する。
pub(crate) fn value_edit(
    previous: &RedNode,
    updated: &RedNode,
    path: &HjsonObjectPath,
    expected: impl FnOnce(&HjsonValue) -> Result<HjsonValue, HjsonEditError>,
    region: EditedRegion,
) -> Result<(), HjsonEditError> {
    let expected = expected(&value_of(previous, path)?)?;
    let actual = reparsed_value(updated, path)?;

    if !values_equal(&expected, &actual) {
        return Err(HjsonEditError::InvalidEdit {
            target: path.to_string(),
            detail: format!("expected {expected:?}, found {actual:?}"),
        });
    }

    if !comments_outside_unchanged(
        previous,
        updated,
        edited_range(previous, path, region),
        edited_range(updated, path, region),
    ) {
        return Err(HjsonEditError::InvalidEdit {
            target: path.to_string(),
            detail: "comments outside the edited region changed".to_string(),
        });
    }

    Ok(())
}

/// 値を指定した操作（バッチ検証用）。
#[derive(Debug, Clone)]
pub enum ValueEdit {
    /// 値を設定した。
    Set(HjsonObjectPath, HjsonValue),
    /// 値を削除した。
    Remove(HjsonObjectPath),
}

impl ValueEdit {
    fn path(&self) -> &HjsonObjectPath {
        match self {
            ValueEdit::Set(path, _) | ValueEdit::Remove(path) => path,
        }
    }

    fn region(&self) -> EditedRegion {
        match self {
            ValueEdit::Set(_, _) => EditedRegion::Node,
            ValueEdit::Remove(_) => EditedRegion::Slot,
        }
    }
}

/// 一連の値の編集を、文書全体の値とコメントで検証する。
///
/// 編集ごとの検証（`set` / `remove` の既定）は操作のたびに文書全体を再パースするため、
/// 操作が多いと文書の大きさ × 操作数の費用がかかる。この関数は再パースを 1 回だけ行う。
pub(crate) fn value_edits_unchanged(
    original: &RedNode,
    updated: &RedNode,
    edits: &[ValueEdit],
) -> Result<(), HjsonEditError> {
    let empty = HjsonObjectPath::from_segments(Vec::new());
    let mut expected = value_of(original, &empty)?;
    for edit in edits {
        match edit {
            ValueEdit::Set(path, value) => {
                crate::high_level::value_ops::set_path_in(&mut expected, path, value.clone())?
            }
            ValueEdit::Remove(path) => {
                crate::high_level::value_ops::remove_path_in(&mut expected, path)?
            }
        }
    }

    // 編集後のテキストが再パースでき、値が期待どおりであること
    let actual = reparsed_value(updated, &empty)?;
    if !values_equal(&expected, &actual) {
        return Err(value_mismatch(&expected, &actual, edits));
    }

    // 編集した範囲の外側でコメントが変化していないこと
    let previous_allowed = edited_ranges(original, edits);
    let updated_allowed = edited_ranges(updated, edits);
    let previous_comments = comments_outside(original, &previous_allowed);
    let updated_comments = comments_outside(updated, &updated_allowed);
    if previous_comments != updated_comments {
        return Err(HjsonEditError::InvalidEdit {
            target: "(document)".to_string(),
            detail: comment_difference(&previous_comments, &updated_comments),
        });
    }

    Ok(())
}

/// 値の不一致を、編集のパスで説明する。
///
/// 編集は適用順なので、後ろから見て最初に食い違ったパスを返す（同じパスを後で上書き
/// していれば、そのパスの期待値は最終値なので一致する）。
fn value_mismatch(
    expected: &HjsonValue,
    actual: &HjsonValue,
    edits: &[ValueEdit],
) -> HjsonEditError {
    for edit in edits.iter().rev() {
        let path = edit.path();
        match (
            crate::high_level::value_ops::get_in(expected, path),
            crate::high_level::value_ops::get_in(actual, path),
        ) {
            (Some(want), Some(found)) if !values_equal(want, found) => {
                return HjsonEditError::InvalidEdit {
                    target: path.to_string(),
                    detail: format!("expected {want:?}, found {found:?}"),
                }
            }
            (None, Some(found)) => {
                return HjsonEditError::InvalidEdit {
                    target: path.to_string(),
                    detail: format!("expected no value at this path, found {found:?}"),
                }
            }
            _ => {}
        }
    }

    HjsonEditError::InvalidEdit {
        target: "(document)".to_string(),
        detail: "values changed".to_string(),
    }
}

/// コメントの並びの最初の差分を説明する。
fn comment_difference(previous: &[String], updated: &[String]) -> String {
    let common = previous
        .iter()
        .zip(updated)
        .take_while(|(left, right)| left == right)
        .count();
    let detail = match (previous.get(common), updated.get(common)) {
        (Some(before), Some(after)) => {
            format!(
                "expected {}, found {}",
                format_text(before),
                format_text(after)
            )
        }
        (Some(before), None) => format!("{} is missing", format_text(before)),
        (None, Some(after)) => format!("unexpected {}", format_text(after)),
        (None, None) => "changed".to_string(),
    };

    format!("comments outside the edited regions changed: {detail}")
}

/// 2 つの木の文書全体の値が同じか。整形は値を変えてはならない。
pub(crate) fn values_unchanged(
    previous: &RedNode,
    updated: &RedNode,
) -> Result<(), HjsonEditError> {
    let empty = HjsonObjectPath::from_segments(Vec::new());
    let expected = value_of(previous, &empty)?;
    let actual = reparsed_value(updated, &empty)?;

    if !crate::high_level::value_ops::values_equal(&expected, &actual) {
        return Err(HjsonEditError::InvalidEdit {
            target: "(document)".to_string(),
            detail: "values changed".to_string(),
        });
    }

    Ok(())
}

/// 整形の前後で、値とコメントが変わっていないこと。
///
/// 整形はレイアウトだけを変える操作であり、コメントを落としたり増やしたりしてはならない。
pub(crate) fn formatting_unchanged(
    previous: &RedNode,
    updated: &RedNode,
) -> Result<(), HjsonEditError> {
    values_unchanged(previous, updated)?;

    if all_comment_bodies(previous) != all_comment_bodies(updated) {
        return Err(HjsonEditError::InvalidEdit {
            target: "(document)".to_string(),
            detail: "comments changed".to_string(),
        });
    }

    Ok(())
}

/// 編集で変化してよい範囲。解決できない場合は `None`（コメントは一切変わってはならない）。
fn edited_range(root: &RedNode, path: &HjsonObjectPath, region: EditedRegion) -> Option<TextRange> {
    match region {
        EditedRegion::Node => PathResolver::new(root.clone())
            .resolve_node(path)
            .ok()
            .map(|node| node.text_range()),
        EditedRegion::Slot => slot_range(root, path),
    }
}

/// 要素本体と前後のトリビアを含むテキスト範囲。
fn slot_range(root: &RedNode, path: &HjsonObjectPath) -> Option<TextRange> {
    let (parent_path, last) = path.split_last()?;
    let parent = PathResolver::new(root.clone())
        .resolve_node(&parent_path)
        .ok()?;
    let container = resolve_body(&parent).ok()?;

    let index = match (container.kind(), &last) {
        (SyntaxKind::Object, PathSegment::Key(key)) => member_index(&container, key)?,
        (SyntaxKind::Array, PathSegment::Array(segment)) => array_slot_index(&container, segment)?,
        _ => return None,
    };
    let span = slot_at(&container, index).ok()?;
    let children: Vec<_> = container.children_with_tokens().collect();

    slot_text_range(&children, &span)
}

/// 削除で変化してよいテキスト範囲（要素と、一緒に消えるトリビア）。
fn slot_text_range(
    children: &[NodeOrToken<RedNode, crate::RedToken>],
    span: &crate::ast::SlotSpan,
) -> Option<TextRange> {
    let range = crate::high_level::ops::insert::removal_range(children, span);
    let start = children.get(range.start)?.text_range().start();
    let end = children
        .get(range.end.saturating_sub(1))?
        .text_range()
        .end();

    Some(TextRange::new(start, end))
}

/// 編集で変化してよい範囲をまとめて求める。
///
/// 編集ごとにパスを根から解決すると、編集の数だけ文書全体を走査することになる。
/// 親コンテナごとに 1 回だけ解決し、コンテナごとに 1 回だけ子を並べて求める。
fn edited_ranges(root: &RedNode, edits: &[ValueEdit]) -> Vec<TextRange> {
    // 1 件だけなら、コンテナの子をすべて並べるよりパスを直接たどる方が速い
    if let [edit] = edits {
        return edited_range(root, edit.path(), edit.region())
            .into_iter()
            .collect();
    }

    struct Group {
        parent: HjsonObjectPath,
        items: Vec<(usize, PathSegment, EditedRegion)>,
    }

    let mut groups: Vec<Group> = Vec::new();
    let mut ranges: Vec<Option<TextRange>> = vec![None; edits.len()];
    for (index, edit) in edits.iter().enumerate() {
        let Some((parent, last)) = edit.path().split_last() else {
            // 空のパスは文書全体の差し替えで、本文もその中のコメントも置き換わる
            ranges[index] = Some(root.text_range());
            continue;
        };
        match groups.iter_mut().find(|group| group.parent == parent) {
            Some(group) => group.items.push((index, last, edit.region())),
            None => groups.push(Group {
                parent,
                items: vec![(index, last, edit.region())],
            }),
        }
    }

    for group in groups {
        let Ok(parent) = PathResolver::new(root.clone()).resolve_node(&group.parent) else {
            continue;
        };
        let Ok(container) = resolve_body(&parent) else {
            continue;
        };

        let elements: Vec<RedNode> = container
            .children()
            .filter(|child| matches!(child.kind(), SyntaxKind::Member | SyntaxKind::Value))
            .collect();
        // スロットの範囲は削除のときだけ要る（コンテナごとに 1 回だけ求める）
        let spans = group
            .items
            .iter()
            .any(|(_, _, region)| *region == EditedRegion::Slot)
            .then(|| crate::high_level::ops::insert::container_slots(&container).ok())
            .flatten();
        let children: Vec<_> = container.children_with_tokens().collect();

        for (index, last, region) in group.items {
            let position = match (container.kind(), &last) {
                (SyntaxKind::Object, PathSegment::Key(key)) => elements.iter().position(|member| {
                    crate::high_level::ops::object::member_key_matches(member, key)
                }),
                (SyntaxKind::Array, PathSegment::Array(segment)) => {
                    let len = elements.len();
                    match segment {
                        ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => {
                            (*index < len).then_some(*index)
                        }
                        ArrayPathSegment::Push => len.checked_sub(1),
                    }
                }
                _ => None,
            };
            let Some(position) = position else {
                continue;
            };

            ranges[index] = match region {
                EditedRegion::Node => elements.get(position).map(|node| node.text_range()),
                EditedRegion::Slot => spans
                    .as_ref()
                    .and_then(|spans| spans.get(position))
                    .and_then(|span| slot_text_range(&children, span)),
            };
        }
    }

    ranges.into_iter().flatten().collect()
}

/// 指定した範囲の外側でコメントが変化していないか。
///
/// 本文だけでなく並び順も比較する。同じ本文のコメントが別の位置へ移れば、
/// 外側の並びが変わるため検出できる。
fn comments_outside_unchanged(
    previous: &RedNode,
    updated: &RedNode,
    previous_allowed: Option<TextRange>,
    updated_allowed: Option<TextRange>,
) -> bool {
    let previous_allowed: Vec<TextRange> = previous_allowed.into_iter().collect();
    let updated_allowed: Vec<TextRange> = updated_allowed.into_iter().collect();
    comments_outside(previous, &previous_allowed) == comments_outside(updated, &updated_allowed)
}

/// 許可された範囲の外側にあるコメント。
fn comments_outside(root: &RedNode, allowed: &[TextRange]) -> Vec<String> {
    let mut texts = Vec::new();
    for (range, text) in comment_texts(root) {
        let inside = allowed
            .iter()
            .any(|allowed| allowed.start() <= range.start() && range.end() <= allowed.end());
        if !inside {
            texts.push(text);
        }
    }

    texts
}

/// 文書中のコメントの位置と生のテキスト。
fn comment_texts(root: &RedNode) -> Vec<(TextRange, String)> {
    let mut comments = Vec::new();

    for element in root.descendants_with_tokens() {
        if let NodeOrToken::Token(token) = element {
            if matches!(
                token.kind(),
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
            ) {
                comments.push((token.text_range(), token.text().to_string()));
            }
        }
    }

    comments
}

/// コメント編集を検証する。
///
/// `expected` は編集後のコメント本文（`None` はコメントが無いこと）。
pub(crate) fn comment_edit(
    previous: &HjsonDocument,
    updated: &HjsonDocument,
    path: &HjsonCommentPath,
    expected: Option<&str>,
) -> Result<(), HjsonEditError> {
    let document_path = HjsonObjectPath::from_segments(Vec::new());

    // 編集後のテキストが再パースできること
    let reparsed =
        HjsonDocument::parse(&updated.syntax_root().text().to_string()).map_err(|error| {
            HjsonEditError::InvalidEdit {
                target: path.to_string(),
                detail: format!("edit produced an unparsable document: {error}"),
            }
        })?;

    // コメント操作は値を変えない
    if !values_equal(
        &value_of(previous.syntax_root(), &document_path)?,
        &value_of(reparsed.syntax_root(), &document_path)?,
    ) {
        return Err(HjsonEditError::InvalidEdit {
            target: path.to_string(),
            detail: "comment edit changed the values".to_string(),
        });
    }

    let updated_target = updated.comment_bodies(path)?;

    let actual = if updated_target.is_empty() {
        None
    } else {
        Some(updated_target.join("\n"))
    };
    if actual.as_deref() != expected {
        return Err(HjsonEditError::InvalidEdit {
            target: path.to_string(),
            detail: format!("expected comment {expected:?}, found {actual:?}"),
        });
    }

    // 対象以外のコメントが、本文も付いている位置も変わっていないこと。
    // 位置（スロット）ごとに比べるため、同じ本文のコメントが別の要素へ移っても検出できる。
    let target_key = target_slot_key(previous, path);
    let previous_rest = rest_of_slots(previous, &target_key)?;
    let updated_rest = rest_of_slots(updated, &target_key)?;

    if previous_rest != updated_rest {
        return Err(HjsonEditError::InvalidEdit {
            target: path.to_string(),
            detail: format!(
                "comments outside the target changed: {previous_rest:?} -> {updated_rest:?}"
            ),
        });
    }

    Ok(())
}

/// 対象のスロットを除いた、位置ごとのコメントと、どのスロットにも属さないコメント。
///
/// スロットは要素の前後とコンテナの末尾しか覆わないため、それ以外の場所
/// （ブレースレス文書の末尾やキーと値の間など）のコメントは残余として比較する。
fn rest_of_slots(
    document: &HjsonDocument,
    target_key: &str,
) -> Result<Vec<(String, Vec<String>)>, HjsonEditError> {
    let slots = slots_by_key(document.syntax_root())?;
    let mut rest: Vec<(String, Vec<String>)> = slots
        .iter()
        .filter(|(slot, _)| slot != target_key)
        .cloned()
        .collect();

    // 残ったスロットが説明できないコメント（ブレースレス文書の末尾など）
    let mut unclaimed = all_comment_bodies(document.syntax_root());
    for (_, bodies) in &slots {
        for body in bodies {
            if let Some(index) = unclaimed.iter().position(|other| other == body) {
                unclaimed.remove(index);
            }
        }
    }
    rest.push(("<unclaimed>".to_string(), unclaimed));

    Ok(rest)
}

/// 操作対象のスロットキー。
///
/// `comment_slots` は実位置（`Index`）で列挙するため、`[-]` や `[+n]` のような
/// 解決が必要なセグメントは実位置に直してからキーにする。
fn target_slot_key(document: &HjsonDocument, path: &HjsonCommentPath) -> String {
    match path {
        HjsonCommentPath::Header => CommentSlot::DocumentHeader.key(),
        HjsonCommentPath::Footer => CommentSlot::DocumentFooter.key(),
        HjsonCommentPath::Before(path) => {
            CommentSlot::Before(resolve_indices(document, path)).key()
        }
        HjsonCommentPath::Inline(path) => {
            CommentSlot::Inline(resolve_indices(document, path)).key()
        }
        HjsonCommentPath::ContainerFooter(path) => {
            CommentSlot::ContainerFooter(resolve_indices(document, path)).key()
        }
    }
}

/// 解決が必要な配列セグメント（`[-]` / `[+n]`）を実位置に置き換える。
///
/// 途中のセグメントも含めて解決する（`items[-].x:before` のようなパスがあるため）。
/// 対象が存在しない場合などは、そのままのパスを返す（編集自体が失敗する）。
fn resolve_indices(document: &HjsonDocument, path: &HjsonObjectPath) -> HjsonObjectPath {
    let resolver = PathResolver::new(document.syntax_root().clone());
    let mut segments: Vec<PathSegment> = Vec::with_capacity(path.depth());

    for segment in path.segments() {
        let PathSegment::Array(alias @ (ArrayPathSegment::Push | ArrayPathSegment::InsertAt(_))) =
            segment
        else {
            segments.push(segment.clone());
            continue;
        };

        let prefix = HjsonObjectPath::from_segments(segments.clone());
        let index = resolver
            .resolve_node(&prefix)
            .ok()
            .and_then(|node| resolve_body(&node).ok())
            .and_then(|container| array_slot_index(&container, alias));

        match index {
            Some(index) => segments.push(PathSegment::Array(ArrayPathSegment::Index(index))),
            // 解決できない場合はそのまま（編集側がエラーを返す）
            None => segments.push(segment.clone()),
        }
    }

    HjsonObjectPath::from_segments(segments)
}

/// 位置ごとのコメント（並び順を保つ）。
fn slots_by_key(root: &RedNode) -> Result<Vec<(String, Vec<String>)>, HjsonEditError> {
    Ok(comment_slots(root)?
        .into_iter()
        .map(|(slot, bodies)| (slot.key(), bodies))
        .collect())
}

/// 文書全体の値。取り出せない場合は編集結果として不正。
///
/// `path` はエラー表示にだけ使う。比較は常に文書全体で行うため、
/// 編集したパス以外の値が変わった場合も [`HjsonEditError::InvalidEdit`] になる。
fn value_of(root: &RedNode, path: &HjsonObjectPath) -> Result<HjsonValue, HjsonEditError> {
    HjsonValue::from_node(root).map_err(|error| HjsonEditError::InvalidEdit {
        target: path.to_string(),
        detail: format!("cannot read the document value: {error}"),
    })
}

/// green ツリーをテキストにして再パースし、値のオラクルと比較できる形にする。
fn reparsed_value(root: &RedNode, path: &HjsonObjectPath) -> Result<HjsonValue, HjsonEditError> {
    let text = root.text().to_string();
    let document = HjsonDocument::parse(&text).map_err(|error| HjsonEditError::InvalidEdit {
        target: path.to_string(),
        detail: format!("edit produced an unparsable document: {error}"),
    })?;

    value_of(document.syntax_root(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> HjsonObjectPath {
        HjsonObjectPath::parse(raw).unwrap()
    }

    fn slots_of(text: &str) -> Vec<(String, Vec<String>)> {
        let document = HjsonDocument::parse(text).unwrap();
        slots_by_key(document.syntax_root()).unwrap()
    }

    #[test]
    fn slots_separate_positions() {
        let slots =
            slots_of("// header\n{\n  // before\n  a: 1 // inline\n  // footer\n}\n// document\n");

        assert!(slots.contains(&(":header".to_string(), vec!["header".to_string()])));
        assert!(slots.contains(&(":footer".to_string(), vec!["document".to_string()])));
        assert!(slots.contains(&("a:before".to_string(), vec!["before".to_string()])));
        assert!(slots.contains(&("a:inline".to_string(), vec!["inline".to_string()])));
        // 文書直下のコンテナの footer は `<root>:footer` で表される
        assert!(slots.contains(&(".:footer".to_string(), vec!["footer".to_string()])));
    }

    #[test]
    fn moving_the_same_comment_changes_the_slots() {
        let before = slots_of("{\n  // note\n  a: 1\n  b: 2\n}\n");
        let after = slots_of("{\n  a: 1\n  // note\n  b: 2\n}\n");

        // 本文の集合は同じでも、位置が違えば列は一致しない
        assert_ne!(before, after);
    }

    #[test]
    fn moving_a_comment_between_document_and_element_is_detected() {
        let header = slots_of("// note\n{\n  a: 1\n}\n");
        let element = slots_of("{\n  // note\n  a: 1\n}\n");

        assert_ne!(header, element);
    }

    fn object_path(raw: &str) -> HjsonObjectPath {
        HjsonObjectPath::parse(raw.to_string()).unwrap()
    }

    #[test]
    fn value_edit_detects_changes_outside_the_edited_path() {
        use crate::high_level::ops::object::HjsonDocumentEdit;
        use crate::high_level::value_ops::set_in;

        let previous = HjsonDocument::parse("{\n  a: 1\n  b: 2\n}\n").unwrap();
        // 編集対象（a）は正しいが、無関係な b も変わる編集を注入する
        let updated = previous
            .clone()
            .set(path("a"), 9)
            .unwrap()
            .set(path("b"), 99)
            .unwrap();

        // 期待値は「a だけを 9 にする」だが、編集結果は b も変わっている
        let error = value_edit(
            previous.syntax_root(),
            updated.syntax_root(),
            &path("a"),
            |document| set_in(document, &[PathSegment::Key("a".to_string())], 9.into()),
            EditedRegion::Slot,
        )
        .unwrap_err();

        assert!(
            matches!(error, HjsonEditError::InvalidEdit { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn comment_edit_accepts_an_unchanged_document() {
        let document = HjsonDocument::parse("{\n  // n\n  a: 1\n}\n").unwrap();
        let path = HjsonCommentPath::Before(object_path("a"));

        comment_edit(&document, &document, &path, Some("n")).unwrap();
    }

    #[test]
    fn reordering_comments_between_slots_is_rejected() {
        // a と c のコメントが入れ替わっている。本文の多重集合は同じだが位置が違う。
        let previous =
            HjsonDocument::parse("{\n  // n\n  a: 1\n  // n\n  b: 2\n  // m\n  c: 3\n}\n").unwrap();
        let updated =
            HjsonDocument::parse("{\n  // m\n  a: 1\n  // n\n  b: 2\n  // n\n  c: 3\n}\n").unwrap();

        // 対象（b の直前）のコメントは変わっていない
        let path = HjsonCommentPath::Before(object_path("b"));
        let error = comment_edit(&previous, &updated, &path, Some("n")).unwrap_err();

        assert!(
            matches!(error, HjsonEditError::InvalidEdit { .. }),
            "reordering was not detected: {error:?}"
        );
    }

    #[test]
    fn unclaimed_comments_are_still_compared() {
        // ブレースレス文書の末尾コメントはどのスロットにも属さない
        let previous = HjsonDocument::parse("a: 1\n// tail\n").unwrap();
        let updated = HjsonDocument::parse("a: 1\n// changed\n").unwrap();
        let path = HjsonCommentPath::Before(object_path("a"));

        let error = comment_edit(&previous, &updated, &path, None).unwrap_err();

        assert!(
            matches!(error, HjsonEditError::InvalidEdit { .. }),
            "a changed unclaimed comment was not detected: {error:?}"
        );
    }

    #[test]
    fn comments_of_a_scalar_document_are_compared() {
        // スカラー文書でも文書全体のコメントは比較できる
        let previous = HjsonDocument::parse("// header\n123\n").unwrap();
        let updated = HjsonDocument::parse("// header\n123\n// tail\n").unwrap();

        let error = comment_edit(
            &previous,
            &updated,
            &HjsonCommentPath::Header,
            Some("header"),
        )
        .unwrap_err();

        assert!(
            matches!(error, HjsonEditError::InvalidEdit { .. }),
            "a changed footer was not detected: {error:?}"
        );
    }

    #[test]
    fn root_container_footer_and_document_footer_are_distinct() {
        // 文書直下のコンテナの footer（閉じ区切りの手前）と、
        // 文書全体の footer（閉じ区切りの後ろ）は別のスロット
        let slots = slots_of("{\n  a: 1\n  // inside\n}\n// outside\n");

        assert!(slots.contains(&(".:footer".to_string(), vec!["inside".to_string()])));
        assert!(slots.contains(&(":footer".to_string(), vec!["outside".to_string()])));
    }

    #[test]
    fn moving_a_comment_to_the_container_footer_is_rejected() {
        let previous = HjsonDocument::parse("{\n  // n\n  a: 1\n  b: 2\n}\n").unwrap();
        let updated = HjsonDocument::parse("{\n  a: 1\n  b: 2\n  // n\n}\n").unwrap();
        let path = HjsonCommentPath::Before(object_path("b"));

        let error = comment_edit(&previous, &updated, &path, None).unwrap_err();

        assert!(
            matches!(error, HjsonEditError::InvalidEdit { .. }),
            "move was not detected: {error:?}"
        );
    }

    #[test]
    fn array_elements_have_their_own_slots() {
        let slots = slots_of("{\n  a: [\n    // first\n    1 // tail\n    // items\n  ]\n}\n");

        assert!(slots.contains(&("a[0]:before".to_string(), vec!["first".to_string()])));
        assert!(slots.contains(&("a[0]:inline".to_string(), vec!["tail".to_string()])));
        assert!(slots.contains(&("a:footer".to_string(), vec!["items".to_string()])));
    }
}

#[cfg(test)]
mod formatting_tests {
    use super::*;

    #[test]
    fn formatting_unchanged_rejects_a_lost_comment() {
        let before = HjsonDocument::parse("{\n  a: 1 // note\n}\n").unwrap();
        let after = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();

        let error = formatting_unchanged(before.syntax_root(), after.syntax_root()).unwrap_err();

        assert!(
            matches!(error, HjsonEditError::InvalidEdit { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn formatting_unchanged_accepts_a_layout_only_change() {
        let before = HjsonDocument::parse("{\n  a: 1\n\n  b: 2 // note\n}\n").unwrap();
        let after = HjsonDocument::parse("{\n\ta: 1\n\n\tb: 2 // note\n}\n").unwrap();

        formatting_unchanged(before.syntax_root(), after.syntax_root()).unwrap();
    }
}

#[cfg(test)]
mod edit_attribution_tests {
    use super::*;
    use crate::high_level::ops::object::HjsonDocumentEdit;
    use crate::high_level::path::HjsonObjectPath;

    fn path(raw: &str) -> HjsonObjectPath {
        HjsonObjectPath::parse(raw).unwrap()
    }

    fn batch_error(
        original: &HjsonDocument,
        updated: &HjsonDocument,
        edits: &[ValueEdit],
    ) -> (String, String) {
        match value_edits_unchanged(original.syntax_root(), updated.syntax_root(), edits) {
            Err(HjsonEditError::InvalidEdit { target, detail }) => (target, detail),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn value_edits_report_the_failing_path() {
        let original = HjsonDocument::parse("{\n  a: 1\n  b: 2\n}\n").unwrap();
        let updated = original
            .clone()
            .set(path("a"), 9)
            .unwrap()
            .set(path("b"), 7)
            .unwrap();

        let (target, detail) = batch_error(
            &original,
            &updated,
            &[
                ValueEdit::Set(path("a"), 9.into()),
                ValueEdit::Set(path("b"), 8.into()),
            ],
        );

        assert_eq!(target, "b");
        assert!(detail.contains("Integer(8)"), "detail: {detail}");
    }

    #[test]
    fn value_edits_report_an_earlier_failing_path() {
        let original = HjsonDocument::parse("{\n  a: 1\n  b: 2\n}\n").unwrap();
        let updated = original
            .clone()
            .set(path("a"), 10)
            .unwrap()
            .set(path("b"), 8)
            .unwrap();

        let (target, detail) = batch_error(
            &original,
            &updated,
            &[
                ValueEdit::Set(path("a"), 9.into()),
                ValueEdit::Set(path("b"), 8.into()),
            ],
        );

        assert_eq!(target, "a");
        assert!(detail.contains("Integer(10)"), "detail: {detail}");
    }

    #[test]
    fn value_edits_report_a_removal_that_did_not_happen() {
        let original = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();

        let (target, detail) = batch_error(&original, &original, &[ValueEdit::Remove(path("a"))]);

        assert_eq!(target, "a");
        assert!(detail.contains("no value"), "detail: {detail}");
    }

    #[test]
    fn comment_changes_name_the_first_difference() {
        let original = HjsonDocument::parse("{\n  // keep\n  a: 1\n}\n").unwrap();
        let updated = HjsonDocument::parse("{\n  a: 9\n}\n").unwrap();

        let (target, detail) =
            batch_error(&original, &updated, &[ValueEdit::Set(path("a"), 9.into())]);

        assert_eq!(target, "(document)");
        assert!(detail.contains("// keep"), "detail: {detail}");
    }

    #[test]
    fn array_removal_index_shift_still_detects_a_lost_comment() {
        // items[0] を消すつもりで、後ろの要素のコメントまで消した文書
        let original = HjsonDocument::parse("items: [\n  1\n  // keep\n  2\n]\n").unwrap();
        let faulty = HjsonDocument::parse("items: [\n  2\n]\n").unwrap();

        let error = value_edits_unchanged(
            original.syntax_root(),
            faulty.syntax_root(),
            &[ValueEdit::Remove(path("items[0]"))],
        )
        .unwrap_err();

        assert!(error.to_string().contains("// keep"), "unexpected: {error}");
    }
}
