/// 差分検出モジュール
///
/// 編集前後のGreen Treeを比較して変更された行番号を特定（1始まり）


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

/// 2つのGreenNodeを比較して変更された行番号を返す
///
/// # Arguments
/// * `before` - 編集前のGreenNode
/// * `after` - 編集後のGreenNode
/// * `source_before` - 編集前のソース文字列（行番号計算用）
///
/// # Returns
/// 変更された行番号のリスト（1始まり、昇順）
pub fn detect_changed_lines(
    before: &rowan::GreenNode,
    after: &rowan::GreenNode,
    source_before: &str,
) -> Vec<usize> {
    let mut changed_lines = Vec::new();
    let mut visited = std::collections::HashSet::new();
    
    // 差分ノードを検出
    let diffs = collect_differences(before, after, vec![]);
    
    for diff in diffs {
        // 変更に関与する行番号を収集
        let lines = get_lines_for_diff(&diff,
            before,
            source_before,
        );
        
        for line in lines {
            if visited.insert(line) {
                changed_lines.push(line);
            }
        }
    }
    
    // ソートして返す
    changed_lines.sort_unstable();
    changed_lines
}

/// 差分情報（パスのみ保持）
#[derive(Debug, Clone)]
struct DiffInfo {
    path: Vec<usize>,
    is_addition: bool,
    is_deletion: bool,
}

/// 要素のテキストを取得（参照版）
fn elem_text(elem: &rowan::NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>) -> String {
    match elem {
        rowan::NodeOrToken::Node(n) => {
            let mut result = String::new();
            for child in n.children() {
                result.push_str(&elem_text(&child));
            }
            result
        }
        rowan::NodeOrToken::Token(t) => t.text().to_string(),
    }
}

/// ノードのテキストを再帰的に取得（参照版）
fn node_text_ref(node: &rowan::GreenNodeData) -> String {
    let mut result = String::new();
    for child in node.children() {
        match child {
            rowan::NodeOrToken::Node(n) => result.push_str(&node_text_ref(n)),
            rowan::NodeOrToken::Token(t) => result.push_str(t.text()),
        }
    }
    result
}

/// 再帰的に差分を収集
fn collect_differences(
    before: &rowan::GreenNode,
    after: &rowan::GreenNode,
    path: Vec<usize>,
) -> Vec<DiffInfo> {
    let mut diffs = Vec::new();
    
    let before_children: Vec<_> = before.children().collect();
    let after_children: Vec<_> = after.children().collect();
    
    let max_len = before_children.len().max(after_children.len());
    
    for i in 0..max_len {
        let mut child_path = path.clone();
        child_path.push(i);
        
        match (before_children.get(i), after_children.get(i)) {
            (Some(b), Some(a)) => {
                // 両方存在 - テキストで比較
                let b_text = elem_text(b);
                let a_text = elem_text(a);
                if b.kind() != a.kind() || b_text != a_text {
                    diffs.push(DiffInfo {
                        path: child_path.clone(),
                        is_addition: false,
                        is_deletion: false,
                    });
                    
                    // 両方ともノードなら再帰
                    if let (rowan::NodeOrToken::Node(bn), rowan::NodeOrToken::Node(an)) = (b, a) {
                        // Arc::cloneを使ってGreenNodeを取得
                        let before_node: rowan::GreenNode = rowan::GreenNode::from(std::borrow::Cow::Borrowed(*bn));
                        let after_node: rowan::GreenNode = rowan::GreenNode::from(std::borrow::Cow::Borrowed(*an));
                        let child_diffs = collect_differences(&before_node, &after_node, child_path);
                        diffs.extend(child_diffs);
                    }
                }
            }
            (Some(_), None) => {
                // beforeにのみ存在（削除）
                diffs.push(DiffInfo {
                    path: child_path,
                    is_addition: false,
                    is_deletion: true,
                });
            }
            (None, Some(_)) => {
                // afterにのみ存在（追加）
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
fn get_lines_for_diff(
    diff: &DiffInfo,
    root: &rowan::GreenNode,
    source: &str,
) -> Vec<usize> {
    let mut lines = Vec::new();
    
    // パスに基づいてオフセットを計算
    let offset = calc_offset_at_path(root, &diff.path);
    let line = offset_to_line(source, offset);
    lines.push(line);
    
    // パスの先にある要素のテキスト長を取得
    let elem_text = get_text_at_path(root, &diff.path);
    let newline_count = elem_text.chars().filter(|&c| c == '\n').count();
    
    for i in 1..=newline_count {
        lines.push(line + i);
    }
    
    lines
}

/// パス位置のオフセットを計算
fn calc_offset_at_path(root: &rowan::GreenNode, path: &[usize]) -> usize {
    fn calc_offset_recursive(node: &rowan::GreenNode, path: &[usize]) -> usize {
        if path.is_empty() {
            return 0;
        }
        
        let children: Vec<_> = node.children().collect();
        let child_idx = path[0];
        
        if child_idx >= children.len() {
            return 0;
        }
        
        let mut offset = 0;
        // 目標の子までのテキスト長を加算
        for j in 0..child_idx {
            match &children[j] {
                rowan::NodeOrToken::Node(n) => {
                    offset += node_text_ref(n).len();
                }
                rowan::NodeOrToken::Token(t) => {
                    offset += t.text().len();
                }
            }
        }
        
        // 次のレベルへ
        if child_idx < children.len() {
            match &children[child_idx] {
                rowan::NodeOrToken::Node(n) => {
                    let green_node: rowan::GreenNode = rowan::GreenNode::from(std::borrow::Cow::Borrowed(*n));
                    offset += calc_offset_recursive(&green_node, &path[1..]);
                }
                rowan::NodeOrToken::Token(_) => {
                    // トークンならここで終了
                }
            }
        }
        
        offset
    }
    
    calc_offset_recursive(root, path)
}

/// パス位置のテキストを取得
fn get_text_at_path(root: &rowan::GreenNode, path: &[usize]) -> String {
    fn get_text_recursive(node: &rowan::GreenNode, path: &[usize]) -> String {
        if path.is_empty() {
            // ノードの場合は全体のテキスト
            let mut result = String::new();
            for child in node.children() {
                result.push_str(&elem_text(&child));
            }
            return result;
        }
        
        let children: Vec<_> = node.children().collect();
        let child_idx = path[0];
        
        if child_idx >= children.len() {
            return String::new();
        }
        
        match &children[child_idx] {
            rowan::NodeOrToken::Node(n) => {
                let green_node: rowan::GreenNode = rowan::GreenNode::from(std::borrow::Cow::Borrowed(*n));
                get_text_recursive(&green_node, &path[1..])
            }
            rowan::NodeOrToken::Token(t) => {
                t.text().to_string()
            }
        }
    }
    
    get_text_recursive(root, path)
}



/// オフセットを行番号に変換（1始まり）
fn offset_to_line(source: &str, offset: usize) -> usize {
    let text_before = &source[..offset.min(source.len())];
    text_before.chars().filter(|&c| c == '\n').count() + 1
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
            changed.push(i + 1); // 1始まり
        }
    }
    
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::syntax::green::GreenTreeBuilder;
    use super::super::syntax::kind::SyntaxKind;

    fn create_simple_object(text: &str) -> rowan::GreenNode {
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
            builder.token(kind, &c.to_string());
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
        
        // 変更後のノード（valueを変更）
        let after_source = "{\n  \"key\": \"new_value\"\n}";
        let after = create_simple_object(after_source);
        
        let changes = detect_changed_lines(&before, &after, source);
        
        // 2行目が変更されたはず
        assert!(changes.contains(&2));
    }
}
