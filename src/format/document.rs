use rowan::GreenNode;

use super::{Format, FormatConfig};
use crate::ast::{DelimiterStyle, Document, DocumentItem, Trivia};
use crate::syntax::{GreenElement, SyntaxKind};

impl Format for Document {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let mut children: Vec<GreenElement> = Vec::new();

        for triv in &self.leading_trivia {
            children.push(trivia_element(triv, config));
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
            children.push(trivia_element(triv, config));
        }

        GreenNode::new(SyntaxKind::Root.into(), children)
    }
}

/// 文書の前後にあるトリビア。改行は整形後の改行コードで出力する。
fn trivia_element(trivia: &Trivia, config: &FormatConfig) -> GreenElement {
    match trivia {
        Trivia::Newline { .. } => SyntaxKind::Newline
            .create_token(config.newline.as_str())
            .into(),
        other => other.to_green_element(),
    }
}
