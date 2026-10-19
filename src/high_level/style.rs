use rowan::NodeOrToken;

use crate::format::IndentStyle;
use crate::syntax::{NewlineKind, RedNode, SyntaxKind};

/// ASTのNewlineトークンから改行スタイルを検出する
pub fn detect_newline_from_ast(root: &RedNode) -> NewlineKind {
    let mut lf = 0usize;
    let mut crlf = 0usize;
    for token in root.descendants_with_tokens() {
        if let NodeOrToken::Token(tok) = token {
            if tok.kind() == SyntaxKind::Newline {
                match NewlineKind::from_text(tok.text()) {
                    Some(NewlineKind::CrLf) => crlf += 1,
                    _ => lf += 1,
                }
            }
        }
    }
    if crlf > lf {
        NewlineKind::CrLf
    } else {
        NewlineKind::Lf
    }
}

/// ノードが属する文書全体から改行コードを検出する。
///
/// 単一行コンテナのように、その部分木だけでは改行を判定できない場合に使う。
pub fn detect_document_newline(node: &RedNode) -> NewlineKind {
    match node.ancestors().last() {
        Some(root) => detect_newline_from_ast(&root),
        None => detect_newline_from_ast(node),
    }
}

/// ASTからインデントスタイルを検出する。
///
/// 行頭（改行の直後）に現れる空白のうち、最も小さい正の幅を1段分とみなす。
/// タブが現れればタブと判定する。インデントが見つからない場合は
/// `IndentStyle::default()`（2スペース）を返す。
pub fn detect_indent_from_ast(root: &RedNode) -> IndentStyle {
    let mut widths: Vec<usize> = Vec::new();
    let mut saw_tab = false;
    // 行頭の空白。次に改行が来るなら空行なので数えない。
    let mut pending: Option<String> = None;
    let mut after_newline = false;

    for element in root.descendants_with_tokens() {
        match element {
            NodeOrToken::Node(_) => {
                if let Some(text) = pending.take() {
                    record_indent_sample(&text, &mut widths, &mut saw_tab);
                }
                after_newline = false;
            }
            NodeOrToken::Token(token) => match token.kind() {
                SyntaxKind::Newline => {
                    pending = None;
                    after_newline = true;
                }
                SyntaxKind::Whitespace if after_newline => {
                    pending = Some(token.text().to_string());
                    after_newline = false;
                }
                _ => {
                    if let Some(text) = pending.take() {
                        record_indent_sample(&text, &mut widths, &mut saw_tab);
                    }
                    after_newline = false;
                }
            },
        }
    }

    if saw_tab {
        return IndentStyle::Tabs;
    }

    let width = widths
        .into_iter()
        .filter(|width| *width > 0)
        .min()
        .unwrap_or(2);
    IndentStyle::Spaces(u8::try_from(width).unwrap_or(2))
}

/// 行頭インデントのサンプルを集める。タブ始まりならタブと判定する。
fn record_indent_sample(text: &str, widths: &mut Vec<usize>, saw_tab: &mut bool) {
    if text.starts_with('\t') {
        *saw_tab = true;
    } else {
        widths.push(text.chars().count());
    }
}

/// 改行コードを検出可能な対象
///
/// テキスト、構文要素、ドキュメントなどから改行コードを検出するためのtrait。
/// 検出できない場合はデフォルト値を返す。
pub trait DetectNewline {
    /// 改行コードを検出する。検出できない場合はデフォルト値を返す。
    fn detect_newline(&self) -> NewlineKind;
}

impl DetectNewline for &str {
    fn detect_newline(&self) -> NewlineKind {
        let crlf = self.matches("\r\n").count();
        let lf = self.matches('\n').count() - crlf;
        if crlf > lf {
            NewlineKind::CrLf
        } else if lf > 0 {
            NewlineKind::Lf
        } else {
            NewlineKind::default()
        }
    }
}

impl DetectNewline for String {
    fn detect_newline(&self) -> NewlineKind {
        self.as_str().detect_newline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_newline_lf() {
        assert_eq!("foo\nbar".detect_newline(), NewlineKind::Lf);
        assert_eq!("\n".detect_newline(), NewlineKind::Lf);
    }

    #[test]
    fn test_detect_newline_crlf() {
        assert_eq!("foo\r\nbar".detect_newline(), NewlineKind::CrLf);
        assert_eq!("\r\n".detect_newline(), NewlineKind::CrLf);
    }

    #[test]
    fn test_detect_newline_mixed_uses_majority() {
        assert_eq!("foo\nbar\nbaz\r\n".detect_newline(), NewlineKind::Lf);
        assert_eq!("foo\r\nbar\r\nbaz\n".detect_newline(), NewlineKind::CrLf);
    }

    #[test]
    fn test_detect_newline_cr_only_or_none() {
        // CR-only defaults to Lf
        assert_eq!("foo\rbar".detect_newline(), NewlineKind::Lf);
        assert_eq!("foo bar".detect_newline(), NewlineKind::Lf);
        assert_eq!("".detect_newline(), NewlineKind::Lf);
    }

    #[test]
    fn test_detect_newline_string() {
        let s = String::from("foo\r\nbar");
        assert_eq!(s.detect_newline(), NewlineKind::CrLf);
    }
}
