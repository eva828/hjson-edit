use rowan::{GreenNode, NodeOrToken};

use crate::ast::{
    self, Array as AstArray, AstNode, CommentKind, CommentTrivia,
    DelimiterStyle, Document, DocumentItem, Member, Object as AstObject, Trivia,
    Value as AstValue,
};
use crate::syntax::{GreenElement, SyntaxKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(u8),
    Tabs,
}

impl Default for IndentStyle {
    fn default() -> Self {
        Self::Spaces(2)
    }
}

impl IndentStyle {
    fn render(&self, depth: usize) -> String {
        match self {
            IndentStyle::Spaces(n) => " ".repeat(*n as usize * depth),
            IndentStyle::Tabs => "\t".repeat(depth),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub(crate) newline: String,
    pub(crate) indent: IndentStyle,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            newline: "\n".to_string(),
            indent: IndentStyle::default(),
        }
    }
}

impl FormatConfig {
    pub fn with_newline(mut self, newline: impl Into<String>) -> Self {
        self.newline = newline.into();
        self
    }

    pub fn with_indent(mut self, indent: IndentStyle) -> Self {
        self.indent = indent;
        self
    }
}

pub trait Format {
    fn format(&self, config: &FormatConfig) -> GreenNode {
        self.format_at(config, 0)
    }
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode;
}

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
                children.push(SyntaxKind::Whitespace.create_token(indent_str.as_str()).into());
                children.push(make_comment_node(ct));
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        for (i, member) in members.iter().enumerate() {
            children.push(SyntaxKind::Whitespace.create_token(indent_str.as_str()).into());

            if i > 0 {
                for triv in &member.leading_trivia {
                    if let Trivia::Comment(ct) = triv {
                        children.push(make_comment_node(ct));
                        children.push(SyntaxKind::Newline.create_token(nl).into());
                        children.push(
                            SyntaxKind::Whitespace.create_token(indent_str.as_str()).into(),
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
                        SyntaxKind::Whitespace.create_token(indent_str.as_str()).into(),
                    );
                    children.push(make_comment_node(ct));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        } else if has_trailing_comments {
            for triv in &self.trailing_trivia {
                if let Trivia::Comment(ct) = triv {
                    children.push(
                        SyntaxKind::Whitespace.create_token(indent_str.as_str()).into(),
                    );
                    children.push(make_comment_node(ct));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        }

        if is_braced {
            let needs_closing_indent = !members.is_empty() || has_leading_comments;
            if needs_closing_indent && depth > 0 {
                children.push(SyntaxKind::Whitespace.create_token(parent_indent.as_str()).into());
            }
            children.push(SyntaxKind::RBrace.create_token("}").into());
        }

        GreenNode::new(SyntaxKind::Object.into(), children)
    }
}

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
                children.push(SyntaxKind::Whitespace.create_token(indent_str.as_str()).into());
                children.push(make_comment_node(ct));
                children.push(SyntaxKind::Newline.create_token(nl).into());
            }
        }

        for (i, item) in items.iter().enumerate() {
            children.push(SyntaxKind::Whitespace.create_token(indent_str.as_str()).into());

            if i > 0 {
                for triv in &item.leading_trivia {
                    if let Trivia::Comment(ct) = triv {
                        children.push(make_comment_node(ct));
                        children.push(SyntaxKind::Newline.create_token(nl).into());
                        children.push(
                            SyntaxKind::Whitespace.create_token(indent_str.as_str()).into(),
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
                    children.push(SyntaxKind::Whitespace.create_token(indent_str.as_str()).into());
                    children.push(make_comment_node(ct));
                    children.push(SyntaxKind::Newline.create_token(nl).into());
                }
            }
        }

        if is_braced {
            let needs_closing_indent = !items.is_empty() || has_leading_comments;
            if needs_closing_indent && depth > 0 {
                children.push(SyntaxKind::Whitespace.create_token(parent_indent.as_str()).into());
            }
            children.push(SyntaxKind::RBracket.create_token("]").into());
        }

        GreenNode::new(SyntaxKind::Array.into(), children)
    }
}

impl Format for Member {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        let Some(val) = self.value() else {
            return self.syntax().green().clone().into();
        };

        let member_green: GreenNode = self.syntax().green().clone().into();
        let formatted_value = val.format_at(config, depth);

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
            let mut replacements: Vec<GreenElement> = Vec::new();
            replacements.push(SyntaxKind::Whitespace.create_token(" ").into());
            replacements.push(formatted_value.into());

            return member_green.splice_children(
                replace_start..value_index + 1,
                replacements,
            );
        }

        let value_index = self
            .syntax()
            .children_with_tokens()
            .position(|child| {
                matches!(&child, rowan::NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value)
            })
            .unwrap_or(0);

        member_green.splice_children(
            value_index..value_index + 1,
            vec![formatted_value.into()],
        )
    }
}

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

fn make_comment_node(ct: &CommentTrivia) -> GreenElement {
    let kind = match ct.kind {
        CommentKind::Line => SyntaxKind::LineComment,
        CommentKind::Hash => SyntaxKind::HashComment,
        CommentKind::Block => SyntaxKind::BlockComment,
    };
    let token = kind.create_token(ct.text.as_str());
    let node = rowan::GreenNode::new(SyntaxKind::Comment.into(), vec![token.into()]);
    NodeOrToken::Node(node)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::high_level::HjsonDocument;
    use crate::syntax::RedNode;

    fn format(hjson: &str, config: &FormatConfig) -> String {
        let doc = HjsonDocument::parse(hjson).unwrap();
        let formatted = doc.ast().unwrap().format(config);
        RedNode::new_root(formatted).text().to_string()
    }

    fn default_config() -> FormatConfig {
        FormatConfig {
            newline: "\n".to_string(),
            indent: IndentStyle::Spaces(2),
        }
    }

    // --- Basic object formatting ---

    #[test]
    fn test_format_simple_object() {
        let input = "{a:1,b:2}";
        let expected = "{\n  a: 1\n  b: 2\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_nested_object() {
        let input = "{a:{b:3}}";
        let expected = "{\n  a: {\n    b: 3\n  }\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_deeply_nested() {
        let input = "{a:{b:{c:1}}}";
        let expected = "{\n  a: {\n    b: {\n      c: 1\n    }\n  }\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_empty_object() {
        assert_eq!(format("{}", &default_config()), "{}");
    }

    // --- Array formatting ---

    #[test]
    fn test_format_simple_array() {
        let input = "[1,2,3]";
        let expected = "[\n  1\n  2\n  3\n]";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_nested_array() {
        let input = "[[1,2],[3,4]]";
        let expected = "[\n  [\n    1\n    2\n  ]\n  [\n    3\n    4\n  ]\n]";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_empty_array() {
        assert_eq!(format("[]", &default_config()), "[]");
    }

    #[test]
    fn test_format_array_in_object() {
        let input = "{items:[1,2,3]}";
        let expected = "{\n  items: [\n    1\n    2\n    3\n  ]\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    // --- Mixed ---

    #[test]
    fn test_format_object_in_array() {
        let input = "[{a:1},{b:2}]";
        let expected = "[\n  {\n    a: 1\n  }\n  {\n    b: 2\n  }\n]";
        assert_eq!(format(input, &default_config()), expected);
    }

    // --- Braceless root ---

    #[test]
    fn test_format_braceless_root() {
        let input = "a:1\nb:2\n";
        let expected = "a: 1\nb: 2\n";
        assert_eq!(format(input, &default_config()), expected);
    }

    // --- Root array ---

    #[test]
    fn test_format_root_array() {
        let input = "[1,2]";
        let expected = "[\n  1\n  2\n]";
        assert_eq!(format(input, &default_config()), expected);
    }

    // --- Comments ---

    #[test]
    fn test_format_leading_comment_in_object() {
        let input = "{// comment\nfoo:123}";
        let expected = "{\n  // comment\n  foo: 123\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_inline_comment() {
        let input = "{foo:123// inline\n}";
        let expected = "{\n  foo: 123 // inline\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_mixed_comments() {
        let input = "{// before\na:1// inline\n// after\nb:2}";
        let expected = "{\n  // before\n  a: 1 // inline\n  // after\n  b: 2\n}";
        assert_eq!(format(input, &default_config()), expected);
    }

    #[test]
    fn test_format_comment_before_array_item() {
        let input = "[// first\n1,2]";
        let expected = "[\n  // first\n  1\n  2\n]";
        assert_eq!(format(input, &default_config()), expected);
    }

    // --- Config: indent size ---

    #[test]
    fn test_format_indent_4_spaces() {
        let config = FormatConfig {
            newline: "\n".to_string(),
            indent: IndentStyle::Spaces(4),
        };
        let input = "{a:{b:1}}";
        let expected = "{\n    a: {\n        b: 1\n    }\n}";
        assert_eq!(format(input, &config), expected);
    }

    #[test]
    fn test_format_indent_tabs() {
        let config = FormatConfig {
            newline: "\n".to_string(),
            indent: IndentStyle::Tabs,
        };
        let input = "{a:{b:1}}";
        let expected = "{\n\ta: {\n\t\tb: 1\n\t}\n}";
        assert_eq!(format(input, &config), expected);
    }

    // --- Config: newline ---

    #[test]
    fn test_format_crlf() {
        let config = FormatConfig {
            newline: "\r\n".to_string(),
            indent: IndentStyle::Spaces(2),
        };
        let input = "{a:1}";
        let expected = "{\r\n  a: 1\r\n}";
        assert_eq!(format(input, &config), expected);
    }

    // --- Edge cases ---

    #[test]
    fn test_format_whitespace_only_is_preserved() {
        let input = "   \n  \n";
        let doc = HjsonDocument::parse(input).unwrap();
        let formatted = doc.ast().unwrap().format(&default_config());
        let result = RedNode::new_root(formatted).text().to_string();
        assert_eq!(result, input);
    }

    #[test]
    fn test_format_comment_only() {
        let input = "// just a comment\n";
        let doc = HjsonDocument::parse(input).unwrap();
        let formatted = doc.ast().unwrap().format(&default_config());
        let result = RedNode::new_root(formatted).text().to_string();
        assert_eq!(result, input);
    }

    #[test]
    fn test_format_root_scalar() {
        let input = "123\n";
        let doc = HjsonDocument::parse(input).unwrap();
        let formatted = doc.ast().unwrap().format(&default_config());
        let result = RedNode::new_root(formatted).text().to_string();
        assert_eq!(result, input);
    }

    // --- Roundtrip: already-formatted stays unchanged ---

    #[test]
    fn test_format_already_pretty() {
        let input = "{\n  foo: 123\n}\n";
        let result = format(input, &default_config());
        assert_eq!(result, input);
    }

    #[test]
    fn test_format_already_pretty_array() {
        let input = "[\n  1\n  2\n]\n";
        let result = format(input, &default_config());
        assert_eq!(result, input);
    }

    // --- Existing passthrough test replaced ---

    #[test]
    fn test_format_comment_in_object() {
        let hjson = "{\n  // comment\n  foo: 123\n}";
        let doc = HjsonDocument::parse(hjson).unwrap();
        let config = default_config();
        let formatted = doc.ast().unwrap().format(&config);
        assert_eq!(
            RedNode::new_root(formatted).text().to_string(),
            hjson
        );
    }
}
