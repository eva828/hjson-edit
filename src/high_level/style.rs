use crate::syntax::NewlineKind;

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
        if self.contains("\r\n") {
            NewlineKind::CrLf
        } else if self.contains('\n') {
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
    fn test_detect_newline_mixed_lf_crlf_returns_crlf() {
        assert_eq!("foo\nbar\r\nbaz".detect_newline(), NewlineKind::CrLf);
        assert_eq!("foo\r\nbar\nbaz".detect_newline(), NewlineKind::CrLf);
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
