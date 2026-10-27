use rowan::GreenNode;

use super::value::multiline_block_node;
use super::{Format, FormatConfig};
use crate::ast::{AstNode, Member, Trivia};
use crate::high_level::indent::{line_indent, root_of};
use crate::syntax::{GreenElement, SyntaxKind};

impl Format for Member {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let Some(val) = self.value() else {
            return self.syntax().green().clone().into();
        };

        let member_green: GreenNode = self.syntax().green().clone().into();
        let indent = config.indent.render(depth);
        let key_width = self
            .key()
            .and_then(|key| key.raw_text())
            .map_or(0, |key| key.chars().count());

        // `:` の後ろで改行している値は、元の行に置いたままにする
        let value_on_new_line = self
            .after_colon_trivia
            .iter()
            .any(|trivia| matches!(trivia, Trivia::Newline { .. }));
        let value_indent = if value_on_new_line {
            line_indent(
                &root_of(val.syntax()),
                val.syntax().text_range().start().into(),
            )
        } else {
            indent.clone()
        };

        // 複数行の Block 文字列は、`'''` の桁に合わせて書き直す
        let (body_indent, delimiter_column, closing_indent) = if value_on_new_line {
            // 値が独立した行にある場合は、その行の字下げをそのまま使う
            let column = value_indent.chars().count();
            (value_indent.clone(), column, value_indent.clone())
        } else {
            (
                format!("{indent}{}", config.indent.unit()),
                indent.chars().count() + key_width + 2,
                indent.clone(),
            )
        };
        // 値が独立した行にある場合は、元の字下げ（タブ等）を本文にも使う
        let column_indent_text = if value_on_new_line {
            value_indent.as_str()
        } else {
            ""
        };
        let formatted_value = multiline_block_node(
            &val,
            &body_indent,
            delimiter_column,
            column_indent_text,
            &closing_indent,
            &config.newline,
        )
        .unwrap_or_else(|| val.format_at(config, depth));

        let colon_index = self
            .syntax()
            .children_with_tokens()
            .position(|child| {
                matches!(&child, rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Colon)
            });

        if let Some(col_idx) = colon_index {
            let value_index = self
                .syntax()
                .children_with_tokens()
                .position(|child| {
                    matches!(&child, rowan::NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value)
                })
                .unwrap_or(0);

            let replace_start = col_idx + 1;
            // `:` と値の間のコメントは落とさない
            let colon_comments: Vec<_> = self
                .after_colon_trivia
                .iter()
                .filter_map(|trivia| match trivia {
                    Trivia::Comment(comment) => Some(comment),
                    _ => None,
                })
                .collect();

            let mut replacements: Vec<GreenElement> = Vec::new();
            if !value_on_new_line || !colon_comments.is_empty() {
                replacements.push(SyntaxKind::Whitespace.create_token(" ").into());
            }
            for (index, comment) in colon_comments.iter().enumerate() {
                if index > 0 {
                    replacements.push(SyntaxKind::Whitespace.create_token(" ").into());
                }
                replacements.push(super::make_comment_node(comment));
            }
            if !value_on_new_line && !colon_comments.is_empty() {
                replacements.push(SyntaxKind::Whitespace.create_token(" ").into());
            }
            if value_on_new_line {
                replacements.push(SyntaxKind::Newline.create_token(&config.newline).into());
                if !value_indent.is_empty() {
                    replacements.push(
                        SyntaxKind::Whitespace
                            .create_token(value_indent.as_str())
                            .into(),
                    );
                }
            }
            replacements.push(formatted_value.into());

            return member_green.splice_children(replace_start..value_index + 1, replacements);
        }

        let value_index = self
            .syntax()
            .children_with_tokens()
            .position(|child| {
                matches!(&child, rowan::NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value)
            })
            .unwrap_or(0);

        member_green.splice_children(value_index..value_index + 1, vec![formatted_value.into()])
    }
}
