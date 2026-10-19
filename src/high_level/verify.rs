//! 編集結果の検証。
//!
//! 編集は「最小限のテキスト変更」で行うため、結果が妥当かどうかを必ず検査する。
//!
//! - 編集後のテキストが再パースできること
//! - 値が「元の値に同じ操作を適用した結果」と一致すること（[`super::value_ops`] を基準にする）
//! - コメント操作では、対象以外のコメントが変化していないこと
//!
//! 検証に失敗した場合は編集結果を返さず [`HjsonEditError::InvalidEdit`] を返す。

use rowan::{NodeOrToken, TextRange};

use crate::ast::CommentKind;
use crate::high_level::error::HjsonEditError;
use crate::high_level::ops::insert::slot_at;
use crate::high_level::path::PathSegment;
use crate::high_level::path::{HjsonCommentPath, HjsonObjectPath};
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
    let start = children.get(span.lead().start)?.text_range().start();
    let end = children
        .get(span.trail().end.saturating_sub(1))?
        .text_range()
        .end();

    Some(TextRange::new(start, end))
}

/// 指定した範囲の外側でコメントが変化していないか。
fn comments_outside_unchanged(
    previous: &RedNode,
    updated: &RedNode,
    previous_allowed: Option<TextRange>,
    updated_allowed: Option<TextRange>,
) -> bool {
    let mut previous_rest = comments_outside(previous, previous_allowed);
    let mut updated_rest = comments_outside(updated, updated_allowed);
    previous_rest.sort();
    updated_rest.sort();

    previous_rest == updated_rest
}

/// 許可された範囲の外側にあるコメント。
fn comments_outside(root: &RedNode, allowed: Option<TextRange>) -> Vec<String> {
    let mut texts = Vec::new();
    for (range, text) in comment_texts(root) {
        let inside = allowed.is_some_and(|allowed| {
            allowed.start() <= range.start() && range.end() <= allowed.end()
        });
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

    let previous_target = previous.comment_bodies(path)?;
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

    // 対象以外のコメントが変化していないこと（多重集合として比較する）
    let mut previous_rest = all_comment_bodies(previous.syntax_root());
    let mut updated_rest = all_comment_bodies(updated.syntax_root());
    remove_bodies(&mut previous_rest, &previous_target);
    remove_bodies(&mut updated_rest, &updated_target);

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

/// 文書の値。取り出せない場合は編集結果として不正。
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

/// 文書中のコメント本文をすべて集める。
fn all_comment_bodies(root: &RedNode) -> Vec<String> {
    let mut bodies: Vec<String> = Vec::new();

    for element in root.descendants_with_tokens() {
        let NodeOrToken::Token(token) = element else {
            continue;
        };
        let kind = match token.kind() {
            SyntaxKind::LineComment => CommentKind::Line,
            SyntaxKind::HashComment => CommentKind::Hash,
            SyntaxKind::BlockComment => CommentKind::Block,
            _ => continue,
        };
        bodies.push(kind.decode_body(token.text()));
    }

    bodies.sort();
    bodies
}

/// `bodies` から `removed` の要素を 1 つずつ取り除く。
fn remove_bodies(bodies: &mut Vec<String>, removed: &[String]) {
    for body in removed {
        if let Some(position) = bodies.iter().position(|existing| existing == body) {
            bodies.remove(position);
        }
    }
}
