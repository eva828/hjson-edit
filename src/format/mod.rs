use rowan::{GreenNode, NodeOrToken};

use crate::ast::{CommentKind, CommentTrivia};
use crate::syntax::{GreenElement, SyntaxKind};

mod array;
mod config;
mod document;
mod member;
mod object;
mod value;

pub use config::{FormatConfig, IndentStyle};

/// 意味ASTを整形して構文木に戻す。
pub trait Format {
    /// 文書全体を整形する。
    fn format(&self, config: &FormatConfig) -> GreenNode {
        self.format_at(config, 0)
    }
    /// 指定した深さを基準に整形する。
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode;
}

pub(crate) fn make_comment_node(ct: &CommentTrivia) -> GreenElement {
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

    #[test]
    fn test_format_array_inline_comments() {
        assert_eq!(
            format("[1// comment\n]", &default_config()),
            "[\n  1 // comment\n]"
        );
        assert_eq!(
            format("[1// a\n,2]", &default_config()),
            "[\n  1 // a\n  2\n]"
        );
        assert_eq!(
            format("[1// a\n,2// b\n]", &default_config()),
            "[\n  1 // a\n  2 // b\n]"
        );
        assert_eq!(
            format("[1// inline\n// trailing\n]", &default_config()),
            "[\n  1 // inline\n  // trailing\n]"
        );
        assert_eq!(
            format("[\n  1 // comment\n]\n", &default_config()),
            "[\n  1 // comment\n]\n"
        );
    }

    #[test]
    fn test_format_object_inline_comments() {
        assert_eq!(
            format("{a:1// comment\n}", &default_config()),
            "{\n  a: 1 // comment\n}"
        );
        assert_eq!(
            format("{a:1// a\n,b:2}", &default_config()),
            "{\n  a: 1 // a\n  b: 2\n}"
        );
        assert_eq!(
            format("{a:1// a\n,b:2// b\n}", &default_config()),
            "{\n  a: 1 // a\n  b: 2 // b\n}"
        );
        assert_eq!(
            format("{a:1// inline\n// trailing\n}", &default_config()),
            "{\n  a: 1 // inline\n  // trailing\n}"
        );
        assert_eq!(
            format("{\n  a: 1 // comment\n}\n", &default_config()),
            "{\n  a: 1 // comment\n}\n"
        );
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

    #[test]
    fn test_format_comment_in_object() {
        let hjson = "{\n  // comment\n  foo: 123\n}";
        let doc = HjsonDocument::parse(hjson).unwrap();
        let config = default_config();
        let formatted = doc.ast().unwrap().format(&config);
        assert_eq!(RedNode::new_root(formatted).text().to_string(), hjson);
    }
}
