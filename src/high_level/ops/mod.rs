pub mod comment;
pub(crate) mod insert;
pub mod merge;
pub mod object;

use crate::high_level::error::HjsonEditError;
use crate::high_level::value::HjsonValue;
use crate::{RedNode, SyntaxKind};

/// node を囲むコンテナ（オブジェクト・配列）の数を数える。node 自身は数えない。
pub(crate) fn enclosing_container_depth(node: &RedNode) -> usize {
    let mut depth = 0usize;
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), SyntaxKind::Object | SyntaxKind::Array) {
            depth += 1;
        }
        current = parent.parent();
    }

    depth
}

/// 新しい値を受け取るコンテナの深さ。
///
/// `anchor` が Value の場合はその内側のコンテナ、Object / Array の場合は
/// `anchor` 自身が受け取り先になる。
pub(crate) fn receiving_container_depth(anchor: &RedNode) -> usize {
    let base = enclosing_container_depth(anchor);
    match anchor.kind() {
        SyntaxKind::Value | SyntaxKind::Object | SyntaxKind::Array => base + 1,
        _ => base,
    }
}

/// 値そのものが持つコンテナのネスト数（スカラーは 0）。
///
/// 利用者が組み立てた値でも再帰でスタックを使い切らないよう反復で数える。
pub(crate) fn container_depth_of_value(root: &HjsonValue) -> usize {
    let mut max_depth = 0usize;
    let mut stack = vec![(root, 0usize)];

    while let Some((value, depth)) = stack.pop() {
        match value {
            HjsonValue::Object(members) => {
                let next = depth + 1;
                max_depth = max_depth.max(next);
                stack.extend(members.iter().map(|(_, child)| (child, next)));
            }
            HjsonValue::Array(items) => {
                let next = depth + 1;
                max_depth = max_depth.max(next);
                stack.extend(items.iter().map(|child| (child, next)));
            }
            _ => {}
        }
    }

    max_depth
}

/// 編集後の深さが上限を超えないことを確認する。
///
/// パース時と同じ上限を編集側でも守ることで、スタックオーバーフローと
/// 「自分で生成した文書を読み戻せない」状態を防ぐ。
pub(crate) fn ensure_depth_within_limit(depth: usize) -> Result<(), HjsonEditError> {
    if depth > crate::MAX_NESTING_DEPTH {
        return Err(HjsonEditError::NestingTooDeep {
            depth,
            limit: crate::MAX_NESTING_DEPTH,
        });
    }

    Ok(())
}

/// サブツリーが持つコンテナのネスト数（対象 node 自身を含む。スカラーは 0）。
///
/// マージ元のように、まだ文書に組み込まれていないサブツリーの深さを数える。
pub(crate) fn relative_container_depth(root: &RedNode) -> usize {
    let mut max_depth = 0usize;
    let mut stack = vec![(root.clone(), 0usize)];

    while let Some((node, depth)) = stack.pop() {
        let next = depth
            + usize::from(matches!(
                node.kind(),
                SyntaxKind::Object | SyntaxKind::Array
            ));
        max_depth = max_depth.max(next);
        stack.extend(node.children().map(|child| (child, next)));
    }

    max_depth
}
