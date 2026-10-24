use rowan::GreenNode;

use super::value::multiline_block_node;
use super::{Format, FormatConfig};
use crate::ast::{AstNode, Member};
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

        // 複数行の Block 文字列は、キーの後ろの `'''` の桁に合わせて書き直す
        let formatted_value = multiline_block_node(
            &val,
            &format!("{indent}{}", config.indent.unit()),
            indent.chars().count() + key_width + 2,
            &indent,
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
            let replacements: Vec<GreenElement> = vec![
                SyntaxKind::Whitespace.create_token(" ").into(),
                formatted_value.into(),
            ];

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
