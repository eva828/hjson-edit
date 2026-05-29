use rowan::{GreenNode, NodeOrToken};

use crate::ast::{AstNode, Key, QuoteStyle};
use crate::high_level::error::HjsonEditError;
use crate::number::HjsonNumber;
use crate::syntax::{GreenElement, SyntaxKind};
use crate::{RedNode, RedToken};

#[derive(Debug, Clone, PartialEq)]
pub enum HjsonValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
    Array(Vec<HjsonValue>),
    Object(Vec<(String, HjsonValue)>),
}

#[derive(Debug, Clone, Copy)]
pub struct RenderConfig<'a> {
    pub string_style: Option<QuoteStyle>,
    pub pretty: bool,
    pub depth: usize,
    pub newline: &'a str,
}

impl<'a> RenderConfig<'a> {
    pub fn with_depth(self, depth: usize) -> Self {
        Self { depth, ..self }
    }

    pub fn wrap_container_elements(
        &self,
        rendered: GreenElement,
        container_is_empty: bool,
    ) -> Vec<GreenElement> {
        let mut result = Vec::new();
        if container_is_empty {
            let indent = "  ".repeat(self.depth);
            if !indent.is_empty() {
                result.push(SyntaxKind::Whitespace.create_token(&indent).into());
            }
            result.push(rendered);
            result.push(SyntaxKind::Newline.create_token(self.newline).into());
            if self.depth > 1 {
                result.push(
                    SyntaxKind::Whitespace
                        .create_token("  ".repeat(self.depth - 1))
                        .into(),
                );
            }
        } else if self.pretty {
            let indent = "  ".repeat(self.depth);
            result.push(SyntaxKind::Newline.create_token(self.newline).into());
            result.push(SyntaxKind::Whitespace.create_token(&indent).into());
            result.push(rendered);
            result.push(SyntaxKind::Newline.create_token(self.newline).into());
            result.push(SyntaxKind::Whitespace.create_token(&indent).into());
        } else {
            result.push(rendered);
        }
        result
    }
}

impl Default for RenderConfig<'_> {
    fn default() -> Self {
        Self {
            string_style: None,
            pretty: false,
            depth: 0,
            newline: "\n",
        }
    }
}

impl From<&str> for HjsonValue {
    fn from(s: &str) -> Self {
        HjsonValue::String(s.to_string())
    }
}

impl From<String> for HjsonValue {
    fn from(s: String) -> Self {
        HjsonValue::String(s)
    }
}

impl From<i64> for HjsonValue {
    fn from(n: i64) -> Self {
        HjsonValue::Integer(n)
    }
}

impl From<i32> for HjsonValue {
    fn from(n: i32) -> Self {
        HjsonValue::Integer(n as i64)
    }
}

impl From<f64> for HjsonValue {
    fn from(n: f64) -> Self {
        HjsonValue::Float(n)
    }
}

impl From<f32> for HjsonValue {
    fn from(n: f32) -> Self {
        HjsonValue::Float(n as f64)
    }
}

impl From<bool> for HjsonValue {
    fn from(b: bool) -> Self {
        HjsonValue::Bool(b)
    }
}

impl From<Vec<HjsonValue>> for HjsonValue {
    fn from(v: Vec<HjsonValue>) -> Self {
        HjsonValue::Array(v)
    }
}

impl From<Vec<(String, HjsonValue)>> for HjsonValue {
    fn from(v: Vec<(String, HjsonValue)>) -> Self {
        HjsonValue::Object(v)
    }
}

impl HjsonValue {
    pub(crate) fn render_value_node(&self, config: &RenderConfig<'_>) -> GreenNode {
        GreenNode::new(SyntaxKind::Value.into(), self.render_body(config))
    }

    pub(crate) fn render_member_node(
        &self,
        key: &str,
        key_style: Option<QuoteStyle>,
        config: &RenderConfig<'_>,
    ) -> GreenNode {
        let children: Vec<GreenElement> = vec![
            render_key_node(key, key_style).into(),
            SyntaxKind::Colon.create_token(":").into(),
            SyntaxKind::Whitespace.create_token(" ").into(),
            self.render_value_node(&config.with_depth(config.depth + 1))
                .into(),
        ];
        GreenNode::new(SyntaxKind::Member.into(), children)
    }

    /// Array の子要素1つを描画（前後の改行・インデント含む）
    pub(crate) fn render_array_item_elements(
        &self,
        config: &RenderConfig<'_>,
        container_is_empty: bool,
    ) -> Vec<GreenElement> {
        let rendered = self.render_value_node(config).into();
        config.wrap_container_elements(rendered, container_is_empty)
    }

    /// Object の子 member1つを描画（前後の改行・インデント含む）
    pub(crate) fn render_object_member_elements(
        &self,
        key: &str,
        key_style: Option<QuoteStyle>,
        config: &RenderConfig<'_>,
        container_is_empty: bool,
    ) -> Vec<GreenElement> {
        let rendered = self.render_member_node(key, key_style, config).into();
        config.wrap_container_elements(rendered, container_is_empty)
    }

    fn render_body(&self, config: &RenderConfig<'_>) -> Vec<GreenElement> {
        match self {
            HjsonValue::String(s) => {
                let style = config.string_style.unwrap_or_else(|| {
                    if is_simple_unquoted_text(s) {
                        QuoteStyle::Unquoted
                    } else {
                        QuoteStyle::DoubleQuoted
                    }
                });
                let (kind, text) = render_string(s, style);
                vec![kind.create_token(text.as_str()).into()]
            }
            HjsonValue::Integer(n) => {
                let text = n.to_string();
                vec![SyntaxKind::Integer.create_token(text.as_str()).into()]
            }
            HjsonValue::Float(n) => {
                let text = n.to_string();
                vec![SyntaxKind::Float.create_token(text.as_str()).into()]
            }
            HjsonValue::Bool(true) => {
                vec![SyntaxKind::True.create_token("true").into()]
            }
            HjsonValue::Bool(false) => {
                vec![SyntaxKind::False.create_token("false").into()]
            }
            HjsonValue::Null => {
                vec![SyntaxKind::Null.create_token("null").into()]
            }
            HjsonValue::Array(items) => {
                let mut children = vec![SyntaxKind::LBracket.create_token("[").into()];
                if config.pretty && !items.is_empty() {
                    children.push(SyntaxKind::Newline.create_token(config.newline).into());
                    for (idx, item) in items.iter().enumerate() {
                        children.push(
                            SyntaxKind::Whitespace
                                .create_token(indent(config.depth).as_str())
                                .into(),
                        );
                        children.push(
                            item.render_value_node(&config.with_depth(config.depth + 1))
                                .into(),
                        );
                        if idx + 1 != items.len() {
                            children.push(SyntaxKind::Newline.create_token(config.newline).into());
                        }
                    }
                    children.push(SyntaxKind::Newline.create_token(config.newline).into());
                    children.push(
                        SyntaxKind::Whitespace
                            .create_token(indent(config.depth.saturating_sub(1)).as_str())
                            .into(),
                    );
                    children.push(SyntaxKind::RBracket.create_token("]").into());
                } else {
                    for (idx, item) in items.iter().enumerate() {
                        children.push(item.render_value_node(config).into());
                        if idx + 1 != items.len() {
                            children.push(SyntaxKind::Comma.create_token(",").into());
                        }
                    }
                    children.push(SyntaxKind::RBracket.create_token("]").into());
                }
                children
            }
            HjsonValue::Object(entries) => {
                let mut children = vec![SyntaxKind::LBrace.create_token("{").into()];
                if config.pretty && !entries.is_empty() {
                    children.push(SyntaxKind::Newline.create_token(config.newline).into());
                    for (idx, (key, value)) in entries.iter().enumerate() {
                        children.push(
                            SyntaxKind::Whitespace
                                .create_token(indent(config.depth).as_str())
                                .into(),
                        );
                        children.push(
                            value
                                .render_member_node(key, None, &config.with_depth(config.depth + 1))
                                .into(),
                        );
                        if idx + 1 != entries.len() {
                            children.push(SyntaxKind::Newline.create_token(config.newline).into());
                        }
                    }
                    children.push(SyntaxKind::Newline.create_token(config.newline).into());
                    children.push(
                        SyntaxKind::Whitespace
                            .create_token(indent(config.depth.saturating_sub(1)).as_str())
                            .into(),
                    );
                    children.push(SyntaxKind::RBrace.create_token("}").into());
                } else {
                    for (idx, (key, value)) in entries.iter().enumerate() {
                        children.push(value.render_member_node(key, None, config).into());
                        if idx + 1 != entries.len() {
                            children.push(SyntaxKind::Comma.create_token(",").into());
                        }
                    }
                    children.push(SyntaxKind::RBrace.create_token("}").into());
                }
                children
            }
        }
    }
}

impl HjsonValue {
    fn from_token(token: &RedToken) -> Result<Self, HjsonEditError> {
        let text = token.text();
        match token.kind() {
            SyntaxKind::String => Ok(HjsonValue::String(
                QuoteStyle::DoubleQuoted.decode_body(text),
            )),
            SyntaxKind::BlockString => {
                Ok(HjsonValue::String(QuoteStyle::Block.decode_body(text)))
            }
            SyntaxKind::UnquotedString => {
                Ok(HjsonValue::String(QuoteStyle::Unquoted.decode_body(text)))
            }
            SyntaxKind::Integer => text
                .parse::<i64>()
                .map(HjsonValue::Integer)
                .map_err(|_| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Integer,
                    found: token.kind(),
                }),
            SyntaxKind::Float => text
                .parse::<f64>()
                .map(HjsonValue::Float)
                .map_err(|_| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Float,
                    found: token.kind(),
                }),
            SyntaxKind::True => Ok(HjsonValue::Bool(true)),
            SyntaxKind::False => Ok(HjsonValue::Bool(false)),
            SyntaxKind::Null => Ok(HjsonValue::Null),
            _ => Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: token.kind(),
            }),
        }
    }

    pub(crate) fn from_node(node: &RedNode) -> Result<Self, HjsonEditError> {
        match node.kind() {
            SyntaxKind::Root => {
                let child = node
                    .children()
                    .find(|child| {
                        matches!(
                            child.kind(),
                            SyntaxKind::Value | SyntaxKind::Object | SyntaxKind::Array
                        )
                    })
                    .ok_or(HjsonEditError::ParentNotFound)?;
                Self::from_node(&child)
            }
            SyntaxKind::Value => node
                .children_with_tokens()
                .find(|child| match child {
                    NodeOrToken::Node(child) => !child.kind().is_trivia(),
                    NodeOrToken::Token(token) => !token.kind().is_trivia(),
                })
                .ok_or(HjsonEditError::ParentNotFound)
                .and_then(|child| match child {
                    NodeOrToken::Node(child) => Self::from_node(&child),
                    NodeOrToken::Token(token) => Self::from_token(&token),
                }),
            SyntaxKind::Object => {
                let mut entries = Vec::new();
                for member in node
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Member)
                {
                    let key = member
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Key)
                        .and_then(|key_node| Key::cast(key_node).ok())
                        .and_then(|k| k.body_text())
                        .ok_or(HjsonEditError::ParentNotFound)?;
                    let value = member
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Value)
                        .ok_or(HjsonEditError::ParentNotFound)?;
                    entries.push((key, Self::from_node(&value)?));
                }
                Ok(HjsonValue::Object(entries))
            }
            SyntaxKind::Array => {
                let mut items = Vec::new();
                for value in node
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Value)
                {
                    items.push(Self::from_node(&value)?);
                }
                Ok(HjsonValue::Array(items))
            }
            SyntaxKind::String
            | SyntaxKind::BlockString
            | SyntaxKind::UnquotedString
            | SyntaxKind::Integer
            | SyntaxKind::Float
            | SyntaxKind::True
            | SyntaxKind::False
            | SyntaxKind::Null => node
                .children_with_tokens()
                .find_map(|child| match child {
                    NodeOrToken::Token(token) if !token.kind().is_trivia() => Some(token),
                    _ => None,
                })
                .ok_or(HjsonEditError::ParentNotFound)
                .and_then(|token| Self::from_token(&token)),
            _ => Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            }),
        }
    }
}

fn render_key_node(key: &str, key_style: Option<QuoteStyle>) -> GreenNode {
    let (key_kind, key_text) = render_string(
        key,
        key_style.unwrap_or_else(|| {
            if is_simple_unquoted_text(key) {
                QuoteStyle::Unquoted
            } else {
                QuoteStyle::DoubleQuoted
            }
        }),
    );
    GreenNode::new(
        SyntaxKind::Key.into(),
        vec![key_kind.create_token(key_text.as_str()).into()],
    )
}

fn render_string(text: &str, style: QuoteStyle) -> (SyntaxKind, String) {
    match style {
        QuoteStyle::DoubleQuoted => (
            SyntaxKind::String,
            format!("\"{}\"", style.encode_body(text)),
        ),
        QuoteStyle::Block => (
            SyntaxKind::BlockString,
            format!("'''{}'''", style.encode_body(text)),
        ),
        QuoteStyle::Unquoted => (SyntaxKind::UnquotedString, style.encode_body(text)),
    }
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

fn is_simple_unquoted_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !matches!(text, "true" | "false" | "null")
        && !HjsonNumber::is_valid(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_simple_unquoted_text_accepts_alphanumeric_underscore_dash_dot() {
        assert!(is_simple_unquoted_text("hello_world"));
        assert!(is_simple_unquoted_text("foo"));
        assert!(is_simple_unquoted_text("a-b.c_d-e0"));
    }

    #[test]
    fn test_is_simple_unquoted_text_rejects_space_and_punctuation() {
        assert!(!is_simple_unquoted_text("hello world"));
        assert!(!is_simple_unquoted_text("foo!"));
        assert!(!is_simple_unquoted_text(""));
    }

    #[test]
    fn test_is_simple_unquoted_text_rejects_reserved_words() {
        assert!(!is_simple_unquoted_text("true"));
        assert!(!is_simple_unquoted_text("false"));
        assert!(!is_simple_unquoted_text("null"));
    }

    #[test]
    fn test_is_simple_unquoted_text_rejects_numeric_tokens() {
        assert!(!is_simple_unquoted_text("123"));
        assert!(!is_simple_unquoted_text("1.23"));
    }

    #[test]
    fn test_wrap_container_elements_empty_container() {
        let config = RenderConfig {
            string_style: None,
            pretty: true,
            depth: 2,
            newline: "\n",
        };
        let elements: Vec<GreenElement> = vec![
            SyntaxKind::Value.create_token("a").into(),
        ];

        let wrap = config.wrap_container_elements(elements[0].clone(), true);
        assert_eq!(wrap.len(), 4);
        assert_eq!(wrap[0].kind(), SyntaxKind::Whitespace.into());
        assert_eq!(wrap[1].kind(), SyntaxKind::Value.into());
        assert_eq!(wrap[2].kind(), SyntaxKind::Newline.into());
        assert_eq!(wrap[3].kind(), SyntaxKind::Whitespace.into());
    }

    #[test]
    fn test_wrap_container_elements_pretty_not_empty() {
        let config = RenderConfig {
            string_style: None,
            pretty: true,
            depth: 2,
            newline: "\n",
        };
        let elements: Vec<GreenElement> = vec![
            SyntaxKind::Value.create_token("a").into(),
        ];

        let wrap = config.wrap_container_elements(elements[0].clone(), false);
        assert_eq!(wrap.len(), 5);
        assert_eq!(wrap[0].kind(), SyntaxKind::Newline.into());
        assert_eq!(wrap[1].kind(), SyntaxKind::Whitespace.into());
        assert_eq!(wrap[2].kind(), SyntaxKind::Value.into());
        assert_eq!(wrap[3].kind(), SyntaxKind::Newline.into());
        assert_eq!(wrap[4].kind(), SyntaxKind::Whitespace.into());
    }

    #[test]
    fn test_wrap_container_elements_not_pretty_not_empty() {
        let config = RenderConfig {
            string_style: None,
            pretty: false,
            depth: 1,
            newline: "\n",
        };
        let elements: Vec<GreenElement> = vec![
            SyntaxKind::Value.create_token("a").into(),
        ];

        let wrap = config.wrap_container_elements(elements[0].clone(), false);
        assert_eq!(wrap.len(), 1);
        assert_eq!(wrap[0].kind(), SyntaxKind::Value.into());
    }

    #[test]
    fn test_render_body_string_default_style() {
        let config = RenderConfig::default();
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body.len(), 1);
        assert_eq!(body[0].kind(), SyntaxKind::UnquotedString.into());
    }

    #[test]
    fn test_render_body_string_double_quoted_style() {
        let config = RenderConfig {
            string_style: Some(QuoteStyle::DoubleQuoted),
            ..RenderConfig::default()
        };
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body[0].kind(), SyntaxKind::String.into());
    }

    #[test]
    fn test_render_body_string_block_style() {
        let config = RenderConfig {
            string_style: Some(QuoteStyle::Block),
            ..RenderConfig::default()
        };
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body[0].kind(), SyntaxKind::BlockString.into());
    }

    #[test]
    fn test_render_body_array_empty() {
        let config = RenderConfig::default();
        let arr = HjsonValue::Array(vec![]);
        let body = arr.render_body(&config);
        assert_eq!(body.len(), 2);
    }

    #[test]
    fn test_render_body_object_empty() {
        let config = RenderConfig::default();
        let obj = HjsonValue::Object(vec![]);
        let body = obj.render_body(&config);
        assert_eq!(body.len(), 2);
    }
}
