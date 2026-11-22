/// インデントの書き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    /// 空白 `n` 個。
    Spaces(u8),
    /// タブ。
    Tabs,
}

impl Default for IndentStyle {
    fn default() -> Self {
        Self::Spaces(2)
    }
}

impl IndentStyle {
    /// 1 段分の字下げ。
    pub(crate) fn unit(&self) -> String {
        match self {
            IndentStyle::Spaces(n) => " ".repeat(*n as usize),
            IndentStyle::Tabs => "\t".to_string(),
        }
    }

    pub(crate) fn render(&self, depth: usize) -> String {
        match self {
            IndentStyle::Spaces(n) => " ".repeat(*n as usize * depth),
            IndentStyle::Tabs => "\t".repeat(depth),
        }
    }
}

/// 整形の設定。
///
/// 未指定の項目は文書から推定する
/// （[`HjsonDocument::detected_format_config`](crate::HjsonDocument::detected_format_config)）。CLI で
/// `--indent-style` / `--indent-width` / `--newline` を指定したときも `edit` は変更した
/// コンテナを整形し、指定は整形の対象にだけ作用する。文書全体を整形するときは `format`
/// サブコマンドを使う。
#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub(crate) newline: String,
    pub(crate) indent: IndentStyle,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            newline: "\n".to_string(),
            indent: IndentStyle::default(),
        }
    }
}

impl FormatConfig {
    /// 現在のインデント。
    pub fn indent(&self) -> IndentStyle {
        self.indent
    }

    /// 現在の改行コード。
    pub fn newline(&self) -> &str {
        &self.newline
    }

    /// 改行コードを指定する。
    pub fn with_newline(mut self, newline: impl Into<String>) -> Self {
        self.newline = newline.into();
        self
    }

    /// インデントを指定する。
    pub fn with_indent(mut self, indent: IndentStyle) -> Self {
        self.indent = indent;
        self
    }
}
