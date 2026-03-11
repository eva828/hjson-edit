/// 差分検出モジュール
///
/// 編集前後の Green Tree を比較して変更された行番号を特定する。
/// 文字列の再構築を避け、主にテキスト長と改行数だけを走査して
/// オフセット計算と行計算を行う。
use rowan::{GreenNode, GreenNodeData, GreenTokenData, NodeOrToken};

/// 変更情報
#[derive(Debug, Clone)]
pub struct ChangeInfo {
    /// 変更された開始行（1始まり）
    pub start_line: usize,
    /// 変更された終了行（1始まり）
    pub end_line: usize,
    /// 変更の種類
    pub change_type: ChangeType,
}

/// 変更の種類
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// 内容が変更された
    Modified,
    /// 追加された
    Added,
    /// 削除された
    Deleted,
}

/// 2つの GreenNode を比較して変更された行番号を返す
///
/// # Arguments
/// * `before` - 編集前の GreenNode
/// * `after` - 編集後の GreenNode
/// * `source_before` - 編集前のソース文字列（行番号計算用）
///
/// # Returns
/// 変更された行番号のリスト（1始まり、昇順）
pub fn detect_changed_lines(
    before: &GreenNode,
    after: &GreenNode,
    source_before: &str,
) -> Vec<usize> {
    let mut changed_lines = Vec::new();
    let mut visited = std::collections::HashSet::new();

    let diffs = collect_differences(before, after, Vec::new());

    for diff in diffs {
        let lines = get_lines_for_diff(&diff, before, source_before);
        for line in lines {
            if visited.insert(line) {
                changed_lines.push(line);
            }
        }
    }

    changed_lines.sort_unstable();
    changed_lines
}

/// 差分情報（パスのみ保持）
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DiffInfo {
    path: Vec<usize>,
    is_addition: bool,
    is_deletion: bool,
}

/// 要素の種類とテキスト長・改行数だけを比較するための要約情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ElementSummary {
    kind: rowan::SyntaxKind,
    text_len: usize,
    newline_count: usize,
}

fn summarize_element(elem: &NodeOrToken<&GreenNodeData, &GreenTokenData>) -> ElementSummary {
    match elem {
        NodeOrToken::Node(node) => ElementSummary {
            kind: node.kind(),
            text_len: node_text_len_ref(node),
            newline_count: node_newline_count_ref(node),
        },
        NodeOrToken::Token(token) => ElementSummary {
            kind: token.kind(),
            text_len: token.text().len(),
            newline_count: token.text().bytes().filter(|&b| b == b'\n').count(),
        },
    }
}

/// ノード配下のテキスト長を再帰的に取得
fn node_text_len_ref(node: &GreenNodeData) -> usize {
    node.children()
        .map(|child| match child {
            NodeOrToken::Node(n) => node_text_len_ref(n),
            NodeOrToken::Token(t) => t.text().len(),
        })
        .sum()
}

/// ノード配下の改行数を再帰的に取得
fn node_newline_count_ref(node: &GreenNodeData) -> usize {
    node.children()
        .map(|child| match child {
            NodeOrToken::Node(n) => node_newline_count_ref(n),
            NodeOrToken::Token(t) => t.text().bytes().filter(|&b| b == b'\n').count(),
        })
        .sum()
}

/// 再帰的に差分を収集
fn collect_differences(before: &GreenNode, after: &GreenNode, path: Vec<usize>) -> Vec<DiffInfo> {
    let mut diffs = Vec::new();

    let before_children: Vec<_> = before.children().collect();
    let after_children: Vec<_> = after.children().collect();
    let max_len = before_children.len().max(after_children.len());

    for i in 0..max_len {
        let mut child_path = path.clone();
        child_path.push(i);

        match (before_children.get(i), after_children.get(i)) {
            (Some(b), Some(a)) => {
                let b_summary = summarize_element(b);
                let a_summary = summarize_element(a);

                if b_summary != a_summary {
                    diffs.push(DiffInfo {
                        path: child_path.clone(),
                        is_addition: false,
                        is_deletion: false,
                    });

                    if let (NodeOrToken::Node(bn), NodeOrToken::Node(an)) = (b, a) {
                        let before_node: GreenNode =
                            GreenNode::from(std::borrow::Cow::Borrowed(*bn));
                        let after_node: GreenNode =
                            GreenNode::from(std::borrow::Cow::Borrowed(*an));
                        diffs.extend(collect_differences(&before_node, &after_node, child_path));
                    }
                }
            }
            (Some(_), None) => {
                diffs.push(DiffInfo {
                    path: child_path,
                    is_addition: false,
                    is_deletion: true,
                });
            }
            (None, Some(_)) => {
                diffs.push(DiffInfo {
                    path: child_path,
                    is_addition: true,
                    is_deletion: false,
                });
            }
            (None, None) => unreachable!(),
        }
    }

    diffs
}

/// 差分に対応する行番号を取得
fn get_lines_for_diff(diff: &DiffInfo, root: &GreenNode, source: &str) -> Vec<usize> {
    let offset = calc_offset_at_path(root, &diff.path);
    let start_line = offset_to_line(source, offset);
    let text_metrics = get_metrics_at_path(root, &diff.path);

    let mut lines = Vec::with_capacity(text_metrics.newline_count.saturating_add(1));
    lines.push(start_line);
    for i in 1..=text_metrics.newline_count {
        lines.push(start_line + i);
    }
    lines
}

#[derive(Debug, Clone, Copy, Default)]
struct TextMetrics {
    newline_count: usize,
}

/// パス位置のオフセットを計算
fn calc_offset_at_path(root: &GreenNode, path: &[usize]) -> usize {
    fn calc_offset_recursive(node: &GreenNode, path: &[usize]) -> usize {
        if path.is_empty() {
            return 0;
        }

        let child_idx = path[0];
        let mut iter = node.children();

        let mut offset = 0usize;
        for child in iter.by_ref().take(child_idx) {
            offset += match child {
                NodeOrToken::Node(n) => node_text_len_ref(n),
                NodeOrToken::Token(t) => t.text().len(),
            };
        }

        let Some(target_child) = iter.next() else {
            return 0;
        };

        match target_child {
            NodeOrToken::Node(n) => {
                let green_node: GreenNode = GreenNode::from(std::borrow::Cow::Borrowed(n));
                offset + calc_offset_recursive(&green_node, &path[1..])
            }
            NodeOrToken::Token(_) => offset,
        }
    }

    calc_offset_recursive(root, path)
}

/// パス位置のテキスト長と改行数を取得
fn get_metrics_at_path(root: &GreenNode, path: &[usize]) -> TextMetrics {
    fn get_metrics_recursive(node: &GreenNode, path: &[usize]) -> TextMetrics {
        if path.is_empty() {
            return TextMetrics {
                newline_count: node
                    .children()
                    .map(|child| match child {
                        NodeOrToken::Node(n) => node_newline_count_ref(n),
                        NodeOrToken::Token(t) => t.text().bytes().filter(|&b| b == b'\n').count(),
                    })
                    .sum(),
            };
        }

        let child_idx = path[0];
        let Some(child) = node.children().nth(child_idx) else {
            return TextMetrics::default();
        };

        match child {
            NodeOrToken::Node(n) => {
                let green_node: GreenNode = GreenNode::from(std::borrow::Cow::Borrowed(n));
                get_metrics_recursive(&green_node, &path[1..])
            }
            NodeOrToken::Token(t) => TextMetrics {
                newline_count: t.text().bytes().filter(|&b| b == b'\n').count(),
            },
        }
    }

    get_metrics_recursive(root, path)
}

/// オフセットを行番号に変換（1始まり）
///
/// `offset` はバイトオフセットを想定する。
fn offset_to_line(source: &str, offset: usize) -> usize {
    let safe_offset = nearest_char_boundary_at_or_before(source, offset.min(source.len()));
    source[..safe_offset]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

fn nearest_char_boundary_at_or_before(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// シンプルな行ベース差分（テスト用）
pub fn simple_line_diff(before: &str, after: &str) -> Vec<usize> {
    let before_lines: Vec<_> = before.lines().collect();
    let after_lines: Vec<_> = after.lines().collect();

    let max_len = before_lines.len().max(after_lines.len());
    let mut changed = Vec::new();

    for i in 0..max_len {
        let b = before_lines.get(i);
        let a = after_lines.get(i);

        if b != a {
            changed.push(i + 1);
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use crate::syntax::green::GreenTreeBuilder;
    use crate::syntax::kind::SyntaxKind;
    use super::*;

    fn create_simple_object(text: &str) -> GreenNode {
        let mut builder = GreenTreeBuilder::new();
        builder.start_node(SyntaxKind::Object);
        for c in text.chars() {
            let kind = match c {
                '{' => SyntaxKind::LBrace,
                '}' => SyntaxKind::RBrace,
                ':' => SyntaxKind::Colon,
                ',' => SyntaxKind::Comma,
                '"' => SyntaxKind::String,
                ' ' | '\t' => SyntaxKind::Whitespace,
                '\n' => SyntaxKind::Newline,
                _ => SyntaxKind::Ident,
            };
            let text = c.to_string();
            builder.token(kind, &text);
        }
        builder.finish_node();
        builder.finish()
    }

    #[test]
    fn test_simple_line_diff() {
        let before = "line1\nline2\nline3";
        let after = "line1\nmodified\nline3";

        let changes = simple_line_diff(before, after);
        assert_eq!(changes, vec![2]);
    }

    #[test]
    fn test_detect_changed_lines() {
        let source = "{\n  \"key\": \"value\"\n}";
        let before = create_simple_object(source);

        let after_source = "{\n  \"key\": \"new_value\"\n}";
        let after = create_simple_object(after_source);

        let changes = detect_changed_lines(&before, &after, source);
        assert!(changes.contains(&2));
    }

    #[test]
    fn test_offset_to_line_with_multibyte_chars() {
        let source = "{\n  key: \"あいう\"\n}\n";
        let offset = source.find("あ").unwrap();
        assert_eq!(offset_to_line(source, offset), 2);
    }

    #[test]
    fn test_nearest_char_boundary_at_or_before() {
        let s = "é";
        assert_eq!(nearest_char_boundary_at_or_before(s, 1), 0);
        assert_eq!(nearest_char_boundary_at_or_before(s, 2), 2);
    }
}
