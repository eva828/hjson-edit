#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(u8),
    Tabs,
}

impl Default for IndentStyle {
    fn default() -> Self {
        Self::Spaces(2)
    }
}

impl IndentStyle {
    pub(crate) fn render(&self, depth: usize) -> String {
        match self {
            IndentStyle::Spaces(n) => " ".repeat(*n as usize * depth),
            IndentStyle::Tabs => "\t".repeat(depth),
        }
    }
}

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
    pub fn with_newline(mut self, newline: impl Into<String>) -> Self {
        self.newline = newline.into();
        self
    }

    pub fn with_indent(mut self, indent: IndentStyle) -> Self {
        self.indent = indent;
        self
    }
}
