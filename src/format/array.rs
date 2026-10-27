use rowan::GreenNode;

use super::value::multiline_block_node;
use super::{leading_segments, make_comment_node, push_leading, Format, FormatConfig};
use crate::ast::{Array as AstArray, AstNode, DelimiterStyle, Trivia};
use crate::syntax::{GreenElement, SyntaxKind};

impl Format for AstArray {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let items = self.items();
        let indent_str = config.indent.render(depth);
        let parent_indent = config.indent.render(depth.saturating_sub(1));
        let nl = &config.newline;
        let mut children: Vec<GreenElement> = Vec::new();

        let is_braced = self.delimiter_style == DelimiterStyle::Braced;
        // 要素もコメントも無いコンテナでも、元が複数行なら 1 行に畳まない
        let keep_empty_lines = is_braced
            && items.is_empty()
            && !self
                .leading_trivia
                .iter()
                .any(|trivia| matches!(trivia, Trivia::Comment(_)))
            && self
                .syntax()
                .children_with_tokens()
                .any(|child| matches!(child, rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline));
        let has_leading_comments = self
            .leading_trivia
            .iter()
            .any(|t| matches!(t, Trivia::Comment(_)));

        if is_braced {
            children.push(SyntaxKind::LBracket.create_token("[").into());
        }

        if !items.is_empty() || has_leading_comments || keep_empty_lines {
            children.push(SyntaxKind::Newline.create_token(nl).into());
        }

        // 要素が無い（コメントだけの）コンテナは、先頭のトリビアをそのまま出す
        if items.is_empty() {
            for (blanks, comment) in leading_segments(&self.leading_trivia, is_braced) {
                for _ in 0..blanks {
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
                if let Some(comment) = comment {
                    children.push(
                        SyntaxKind::Whitespace
                            .create_token(indent_str.as_str())
                            .into(),
                    );
                    children.push(make_comment_node(&comment));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        }

        for (i, item) in items.iter().enumerate() {
            // 先頭の要素の前のトリビアはコンテナ側が持つ（要素側と重複する）
            let (lead, base_newline) = if i == 0 {
                (&self.leading_trivia, true)
            } else {
                // 直前の要素の行末の改行はその要素の trailing にあり、ここには含まれない
                (&item.leading_trivia, false)
            };
            push_leading(&mut children, lead, indent_str.as_str(), nl, base_newline);

            if let Ok(val) = item.value() {
                let formatted = multiline_block_node(
                    &val,
                    &format!("{indent_str}{}", config.indent.unit()),
                    indent_str.chars().count(),
                    &indent_str,
                    &indent_str,
                    nl,
                )
                .unwrap_or_else(|| val.format_at(config, depth));
                children.push(formatted.into());
            }

            for triv in &item.trailing_trivia {
                if let Trivia::Comment(ct) = triv {
                    children.push(SyntaxKind::Whitespace.create_token(" ").into());
                    children.push(make_comment_node(ct));
                }
            }

            let has_more = i + 1 < items.len() || !self.trailing_trivia.is_empty() || is_braced;
            if has_more {
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        {
            // 末尾の空行とコメント（直前の要素の改行は出力済み）
            for (blanks, comment) in leading_segments(&self.trailing_trivia, false) {
                for _ in 0..blanks {
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
                if let Some(comment) = comment {
                    children.push(
                        SyntaxKind::Whitespace
                            .create_token(indent_str.as_str())
                            .into(),
                    );
                    children.push(make_comment_node(&comment));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        }

        if is_braced {
            let needs_closing_indent =
                !items.is_empty() || has_leading_comments || keep_empty_lines;
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
