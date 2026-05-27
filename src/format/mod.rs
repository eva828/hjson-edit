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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::high_level::HjsonDocument;

    #[test]
    fn test_format_is_passthrough() {
        let hjson = "{\n  // comment\n  foo: 123\n}";
        let doc = HjsonDocument::parse(hjson).unwrap();
        
        let config = FormatConfig::default();
        let formatted = Formatter::build(&doc.ast().unwrap(), config);
        
        assert_eq!(crate::syntax::RedNode::new_root(formatted).text().to_string(), hjson);
    }
}
