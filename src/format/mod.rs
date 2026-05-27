use rowan::GreenNode;

use crate::ast::{AstNode, Document};

#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub(crate) newline: String,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            newline: "\n".to_string(),
        }
    }
}

impl FormatConfig {
    pub fn with_newline(mut self, newline: impl Into<String>) -> Self {
        self.newline = newline.into();
        self
    }
}

pub struct Formatter;

impl Formatter {
    pub fn build(document: &Document, _config: FormatConfig) -> GreenNode {
        document.syntax().green().clone().into()
    }
}
