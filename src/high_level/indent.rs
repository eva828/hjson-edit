//! 挿入する要素の字下げ。
//!
//! 編集は最小限のテキスト変更に留めるが、新しい要素は既存の要素と桁を揃える。
//! 字下げの算出をここに集め、要素の挿入とコメントの挿入で同じ規則を使う。

use rowan::{Language, NodeOrToken};

use crate::format::IndentStyle;
use crate::syntax::{GreenElement, RedNode, SyntaxKind};

/// ノードを含む文書のルート。
pub(crate) fn root_of(node: &RedNode) -> RedNode {
    node.ancestors().last().unwrap_or_else(|| node.clone())
}

/// 指定位置の行の字下げ（直前の改行から位置までの先頭の空白）。
pub(crate) fn line_indent(root: &RedNode, offset: usize) -> String {
    let text = root.text().to_string();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);

    text[line_start..offset]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

/// 字下げ 1 段分。文書から推定し、推定できなければ 2 スペース。
pub(crate) fn indent_unit(root: &RedNode) -> String {
    match crate::high_level::style::detect_indent_from_ast(root) {
        IndentStyle::Spaces(width) => " ".repeat(width as usize),
        IndentStyle::Tabs => "\t".to_string(),
    }
}

/// 指定位置の直前にある字下げ。行頭でなければ空。
pub(crate) fn indent_before(root: &RedNode, offset: usize) -> String {
    let text = root.text().to_string();
    let head = &text[..offset];
    let trimmed = head.trim_end_matches([' ', '\t']);
    if !trimmed.ends_with('\n') {
        return String::new();
    }

    head[trimmed.len()..].to_string()
}

/// コンテナの要素を置く字下げ。コンテナの行の字下げに 1 段を足す。
pub(crate) fn container_indent(container: &RedNode, base: &str) -> String {
    format!("{base}{}", indent_unit(&root_of(container)))
}

/// 空のコンテナへ最初の要素を入れるときの字下げ。
///
/// コンテナが文書に繋がっている場合は、その行の字下げに 1 段を足す。
pub(crate) fn empty_container_indent(container: &RedNode) -> String {
    let root = root_of(container);
    let base = line_indent(&root, container.text_range().start().into());

    container_indent(container, &base)
}

/// 要素列の各行頭に字下げを入れる。
///
/// 先頭と、要素列の中の改行の直後にだけ入れる（末尾の改行の後ろは、続く要素が
/// 自分の字下げを持っているため入れない）。
pub(crate) fn indent_lines(elements: Vec<GreenElement>, indent: &str) -> Vec<GreenElement> {
    let mut indented: Vec<GreenElement> = Vec::with_capacity(elements.len() + 1);
    let mut needs_indent = true;

    for element in elements {
        if needs_indent && !indent.is_empty() {
            indented.push(whitespace_element(indent));
        }
        needs_indent = is_newline_element(&element);
        indented.push(element);
    }

    indented
}

/// 空白の green 要素。
pub(crate) fn whitespace_element(text: &str) -> GreenElement {
    SyntaxKind::Whitespace.create_token(text).into()
}

fn is_newline_element(element: &GreenElement) -> bool {
    match element {
        NodeOrToken::Token(token) => {
            crate::syntax::HjsonLanguage::kind_from_raw(token.kind()) == SyntaxKind::Newline
        }
        NodeOrToken::Node(_) => false,
    }
}
