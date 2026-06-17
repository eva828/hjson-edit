use crate::syntax::{NewlineKind, RedNode, SyntaxKind};
use rowan::NodeOrToken;

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
    if crlf > lf { NewlineKind::CrLf } else { NewlineKind::Lf }
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
