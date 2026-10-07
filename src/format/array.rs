use rowan::GreenNode;

use super::{make_comment_node, Format, FormatConfig};
use crate::ast::{Array as AstArray, DelimiterStyle, Trivia};
use crate::syntax::{GreenElement, SyntaxKind};

impl Format for AstArray {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let items = self.items();
        let indent_str = config.indent.render(depth);
        let parent_indent = config.indent.render(depth.saturating_sub(1));
        let nl = &config.newline;
        let mut children: Vec<GreenElement> = Vec::new();

        let is_braced = self.delimiter_style == DelimiterStyle::Braced;
        let has_leading_comments = self
            .leading_trivia
            .iter()
            .any(|t| matches!(t, Trivia::Comment(_)));
        let has_trailing_comments = self
            .trailing_trivia
            .iter()
            .any(|t| matches!(t, Trivia::Comment(_)));

        if is_braced {
            children.push(SyntaxKind::LBracket.create_token("[").into());
        }

        if !items.is_empty() || has_leading_comments {
            children.push(SyntaxKind::Newline.create_token(nl).into());
        }

        for triv in &self.leading_trivia {
            if let Trivia::Comment(ct) = triv {
                children.push(
                    SyntaxKind::Whitespace
                        .create_token(indent_str.as_str())
                        .into(),
                );
                children.push(make_comment_node(ct));
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        for (i, item) in items.iter().enumerate() {
            children.push(
                SyntaxKind::Whitespace
                    .create_token(indent_str.as_str())
                    .into(),
            );

            if i > 0 {
                for triv in &item.leading_trivia {
                    if let Trivia::Comment(ct) = triv {
                        children.push(make_comment_node(ct));
                        children.push(SyntaxKind::Newline.create_token(nl).into());
                        children.push(
                            SyntaxKind::Whitespace
                                .create_token(indent_str.as_str())
                                .into(),
                        );
                    }
                }
            }

            if let Ok(val) = item.value() {
                children.push(val.format_at(config, depth).into());
            }

            for triv in &item.trailing_trivia {
                if let Trivia::Comment(ct) = triv {
                    children.push(SyntaxKind::Whitespace.create_token(" ").into());
                    children.push(make_comment_node(ct));
                }
            }

            let has_more = i + 1 < items.len() || has_trailing_comments || is_braced;
            if has_more {
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        if has_trailing_comments {
            for triv in &self.trailing_trivia {
                if let Trivia::Comment(ct) = triv {
                    children.push(
                        SyntaxKind::Whitespace
                            .create_token(indent_str.as_str())
                            .into(),
                    );
                    children.push(make_comment_node(ct));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        }

        if is_braced {
            let needs_closing_indent = !items.is_empty() || has_leading_comments;
            if needs_closing_indent && depth > 0 {
                children.push(
                    SyntaxKind::Whitespace
                        .create_token(parent_indent.as_str())
                        .into(),
                );
            }
            children.push(SyntaxKind::RBracket.create_token("]").into());
        }

        GreenNode::new(SyntaxKind::Array.into(), children)
    }
}
