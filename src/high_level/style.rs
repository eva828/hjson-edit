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


