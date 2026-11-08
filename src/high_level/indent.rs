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

/// ノードの行頭からノードまでの先頭の空白と、その範囲が空白だけかを調べる。
///
/// 文書全体を文字列化すると要素数に対して二次関数的になるため、トークンを後方へ
/// たどって行頭を探す。手前に要素があればその時点で行頭ではないと分かるので、
/// `prefix_is_whitespace` はそこで打ち切る（1 行の巨大な文書でも 1 回あたり定数時間）。
fn line_prefix(node: &RedNode) -> (String, bool) {
    let mut run = String::new();
    let mut only_whitespace = true;

    let mut token = node.first_token().and_then(|token| token.prev_token());
    while let Some(current) = token {
        let text = current.text();
        match text.rfind('\n') {
            Some(position) => {
                let tail = &text[position + 1..];
                let leading: String = tail
                    .chars()
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .collect();
                run.insert_str(0, &leading);
                if leading.len() != tail.len() {
                    only_whitespace = false;
                }
                return (run, only_whitespace);
            }
            None => {
                if text.chars().all(|ch| ch == ' ' || ch == '\t') {
                    run.insert_str(0, text);
                } else {
                    // 行頭はさらに手前にある。行頭直後の空白だけを残す
                    only_whitespace = false;
                    run.clear();
                }
            }
        }
        token = current.prev_token();
    }

    (run, only_whitespace)
}

/// ノードの手前（同じ行）に空白以外があるか。無ければ行頭。
fn prefix_is_whitespace(node: &RedNode) -> bool {
    let mut token = node.first_token().and_then(|token| token.prev_token());
    while let Some(current) = token {
        let text = current.text();
        match text.rfind('\n') {
            Some(position) => {
                return text[position + 1..]
                    .chars()
                    .all(|ch| ch == ' ' || ch == '\t')
            }
            None => {
                if !text.chars().all(|ch| ch == ' ' || ch == '\t') {
                    return false;
                }
            }
        }
        token = current.prev_token();
    }

    true
}

/// ノードの行の字下げ（行頭からノードまでの先頭の空白）。
pub(crate) fn line_indent(node: &RedNode) -> String {
    line_prefix(node).0
}

/// 字下げ 1 段分。文書から推定し、推定できなければ 2 スペース。
pub(crate) fn indent_unit(root: &RedNode) -> String {
    match crate::high_level::style::detect_indent_from_ast(root) {
        IndentStyle::Spaces(width) => " ".repeat(width as usize),
        IndentStyle::Tabs => "\t".to_string(),
    }
}

/// ノードの直前にある字下げ。行頭でなければ空。
pub(crate) fn indent_before(node: &RedNode) -> String {
    if prefix_is_whitespace(node) {
        line_indent(node)
    } else {
        String::new()
    }
}

/// コンテナの要素を置く字下げ。コンテナの行の字下げに 1 段を足す。
pub(crate) fn container_indent(container: &RedNode, base: &str) -> String {
    format!("{base}{}", indent_unit(&root_of(container)))
}

/// ノードが行頭（行の先頭から空白だけ）から始まるか。
pub(crate) fn is_at_line_start(node: &RedNode) -> bool {
    prefix_is_whitespace(node)
}

/// 空のコンテナへ最初の要素を入れるときの字下げ。
///
/// コンテナが文書に繋がっている場合は、その行の字下げに 1 段を足す。
pub(crate) fn empty_container_indent(container: &RedNode) -> String {
    let base = line_indent(container);

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
