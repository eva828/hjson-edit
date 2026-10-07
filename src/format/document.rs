use rowan::GreenNode;

use super::{Format, FormatConfig};
use crate::ast::{DelimiterStyle, Document, DocumentItem};
use crate::syntax::{GreenElement, SyntaxKind};

impl Format for Document {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let mut children: Vec<GreenElement> = Vec::new();

        for triv in &self.leading_trivia {
            children.push(triv.to_green_element());
        }

        if let Some(item) = self.item() {
            match item {
                DocumentItem::Object(obj) => {
                    let child_depth = if obj.delimiter_style == DelimiterStyle::Braced {
                        depth + 1
                    } else {
                        depth
                    };
                    children.push(obj.format_at(config, child_depth).into());
                }
                DocumentItem::Array(arr) => {
                    children.push(arr.format_at(config, depth + 1).into());
                }
                DocumentItem::Value(val) => {
                    children.push(val.format_at(config, depth).into());
                }
            }
        }

        for triv in &self.trailing_trivia {
            children.push(triv.to_green_element());
        }

        GreenNode::new(SyntaxKind::Root.into(), children)
    }
}
