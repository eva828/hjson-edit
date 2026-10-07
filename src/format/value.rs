use rowan::GreenNode;

use super::{Format, FormatConfig};
use crate::ast::{self, AstNode, Value as AstValue};
use crate::syntax::SyntaxKind;

impl Format for AstValue {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        match self.content() {
            Ok(content) => match content {
                ast::ValueContent::Object(obj) => {
                    let formatted = obj.format_at(config, depth + 1);
                    GreenNode::new(SyntaxKind::Value.into(), vec![formatted.into()])
                }
                ast::ValueContent::Array(arr) => {
                    let formatted = arr.format_at(config, depth + 1);
                    GreenNode::new(SyntaxKind::Value.into(), vec![formatted.into()])
                }
                ast::ValueContent::Scalar(_) => self.syntax().green().clone().into(),
            },
            Err(_) => self.syntax().green().clone().into(),
        }
    }
}
