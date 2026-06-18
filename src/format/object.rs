use rowan::GreenNode;

use super::{Format, FormatConfig, make_comment_node};
use crate::ast::{DelimiterStyle, Object as AstObject, Trivia};
use crate::syntax::{GreenElement, SyntaxKind};

impl Format for AstObject {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let members = self.members();
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
            children.push(SyntaxKind::LBrace.create_token("{").into());
        }

        if is_braced && (!members.is_empty() || has_leading_comments) {
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

        for (i, member) in members.iter().enumerate() {
            children.push(
                SyntaxKind::Whitespace
                    .create_token(indent_str.as_str())
                    .into(),
            );

            if i > 0 {
                for triv in &member.leading_trivia {
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

            children.push(member.format_at(config, depth).into());

            let member_trailing_comments: Vec<_> = member
                .trailing_trivia
                .iter()
                .filter_map(|t| {
                    if let Trivia::Comment(ct) = t {
                        Some(ct.clone())
                    } else {
                        None
                    }
                })
                .collect();

            for ct in &member_trailing_comments {
                children.push(SyntaxKind::Whitespace.create_token(" ").into());
                children.push(make_comment_node(ct));
            }

            if i + 1 < members.len() || is_braced || !member.trailing_trivia.is_empty() {
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        if has_trailing_comments && !members.is_empty() {
            let last_member = &members[members.len() - 1];
            let last_member_inline_count = last_member
                .trailing_trivia
                .iter()
                .filter(|t| matches!(t, Trivia::Comment(_)))
                .count();

            let mut skipped = 0;
            for triv in &self.trailing_trivia {
                if let Trivia::Comment(ct) = triv {
                    if skipped < last_member_inline_count {
                        skipped += 1;
                        continue;
                    }
                    children.push(
                        SyntaxKind::Whitespace
                            .create_token(indent_str.as_str())
                            .into(),
                    );
                    children.push(make_comment_node(ct));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        } else if has_trailing_comments {
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
            let needs_closing_indent = !members.is_empty() || has_leading_comments;
            if needs_closing_indent && depth > 0 {
                children.push(
                    SyntaxKind::Whitespace
                        .create_token(parent_indent.as_str())
                        .into(),
                );
            }
            children.push(SyntaxKind::RBrace.create_token("}").into());
        }

        GreenNode::new(SyntaxKind::Object.into(), children)
    }
}
