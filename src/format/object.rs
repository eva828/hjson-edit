use rowan::GreenNode;

use super::{leading_segments, make_comment_node, push_leading, Format, FormatConfig};
use crate::ast::AstNode;
use crate::ast::{DelimiterStyle, Object as AstObject, Trivia};
use crate::syntax::{GreenElement, SyntaxKind};

/// 最後のメンバーの後ろにある改行の数（ブレースレスの本文末）。
fn trailing_newlines(node: &crate::syntax::RedNode) -> usize {
    let children: Vec<_> = node.children_with_tokens().collect();
    let start = children
        .iter()
        .rposition(|child| matches!(child, rowan::NodeOrToken::Node(node) if node.kind() == SyntaxKind::Member))
        .map_or(children.len(), |index| index + 1);

    children[start..]
        .iter()
        .filter(|child| matches!(child, rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline))
        .count()
}

impl Format for AstObject {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let members = self.members();
        let indent_str = config.indent.render(depth);
        let parent_indent = config.indent.render(depth.saturating_sub(1));
        let nl = &config.newline;
        let mut children: Vec<GreenElement> = Vec::new();

        let is_braced = self.delimiter_style == DelimiterStyle::Braced;
        // 要素もコメントも無いコンテナでも、元が複数行なら 1 行に畳まない
        let keep_empty_lines = is_braced
            && members.is_empty()
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
            children.push(SyntaxKind::LBrace.create_token("{").into());
        }

        if is_braced && (!members.is_empty() || has_leading_comments || keep_empty_lines) {
            children.push(SyntaxKind::Newline.create_token(nl).into());
        }

        // 要素が無い（コメントだけの）コンテナは、先頭のトリビアをそのまま出す
        if members.is_empty() {
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
                    if !super::is_unterminated_block_comment(&comment) {
                        children.push(SyntaxKind::Newline.create_token(nl).into());
                    }
                }
            }
        }

        for (i, member) in members.iter().enumerate() {
            // 先頭の要素の前のトリビアはコンテナ側が持つ（要素側と重複する）
            let (lead, base_newline) = if i == 0 {
                (&self.leading_trivia, is_braced)
            } else {
                // 直前の要素の行末の改行はその要素の trailing にあり、ここには含まれない
                (&member.leading_trivia, false)
            };
            push_leading(&mut children, lead, indent_str.as_str(), nl, base_newline);

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

            // 閉じていないブロックコメントの後ろに改行を足すとコメントの一部になる
            let ends_unterminated = member_trailing_comments
                .last()
                .is_some_and(super::is_unterminated_block_comment);
            if !ends_unterminated
                && (i + 1 < members.len() || is_braced || !member.trailing_trivia.is_empty())
            {
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        // ブレースレスの本文末の空行は、コンテナ直下のトークンとして現れる
        if !is_braced && self.trailing_trivia.is_empty() {
            let tail_newlines = trailing_newlines(self.syntax());
            for _ in 1..tail_newlines {
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
                    if !super::is_unterminated_block_comment(&comment) {
                        children.push(SyntaxKind::Newline.create_token(nl).into());
                    }
                }
            }
        }

        if is_braced {
            let needs_closing_indent =
                !members.is_empty() || has_leading_comments || keep_empty_lines;
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
