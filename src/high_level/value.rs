use rowan::{GreenNode, NodeOrToken};

use crate::ast::{AstNode, Key, QuoteStyle, Trivia};
use crate::high_level::error::HjsonEditError;
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

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderConfig {
    pub string_style: Option<QuoteStyle>,
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
    pub(crate) fn render_value_node(&self, config: &RenderConfig) -> GreenNode {
        GreenNode::new(SyntaxKind::Value.into(), self.render_body(config))
    }

    pub(crate) fn render_member_node(
        &self,
        key: &str,
        key_style: Option<QuoteStyle>,
        config: &RenderConfig,
        before_colon_trivia: &[Trivia],
        after_colon_trivia: &[Trivia],
    ) -> GreenNode {
        let mut children: Vec<GreenElement> =
            Vec::with_capacity(3 + before_colon_trivia.len() + after_colon_trivia.len());
        children.push(render_key_node(key, key_style).into());
        for trivia in before_colon_trivia {
            children.push(trivia.to_green_element());
        }
        children.push(SyntaxKind::Colon.create_token(":").into());
        for trivia in after_colon_trivia {
            children.push(trivia.to_green_element());
        }
        children.push(self.render_value_node(config).into());
        GreenNode::new(SyntaxKind::Member.into(), children)
    }

    fn render_body(&self, config: &RenderConfig) -> Vec<GreenElement> {
        match self {
            HjsonValue::String(s) => {
                let style = config.string_style.unwrap_or_else(|| {
                    if QuoteStyle::Unquoted.is_valid_for_text(s) {
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
                for (idx, item) in items.iter().enumerate() {
                    children.push(item.render_value_node(config).into());
                    if idx + 1 != items.len() {
                        children.push(SyntaxKind::Comma.create_token(",").into());
                    }
                }
                children.push(SyntaxKind::RBracket.create_token("]").into());
                vec![GreenNode::new(SyntaxKind::Array.into(), children).into()]
            }
            HjsonValue::Object(entries) => {
                let mut children = vec![SyntaxKind::LBrace.create_token("{").into()];
                for (idx, (key, value)) in entries.iter().enumerate() {
                    children.push(value.render_member_node(key, None, config, &[], &[]).into());
                    if idx + 1 != entries.len() {
                        children.push(SyntaxKind::Comma.create_token(",").into());
                    }
                }
                children.push(SyntaxKind::RBrace.create_token("}").into());
                vec![GreenNode::new(SyntaxKind::Object.into(), children).into()]
            }
        }
    }
}

impl HjsonValue {
    fn from_token(token: &RedToken) -> Result<Self, HjsonEditError> {
        let text = token.text();
        match token.kind() {
            SyntaxKind::String => {
                let style = if text.starts_with('\'') {
                    QuoteStyle::SingleQuoted
                } else {
                    QuoteStyle::DoubleQuoted
                };
                Ok(HjsonValue::String(style.decode_body(text, 0)))
            }
            SyntaxKind::BlockString => Ok(HjsonValue::String(
                QuoteStyle::Block.decode_body(text, crate::ast::token_column(token)),
            )),
            SyntaxKind::UnquotedString => Ok(HjsonValue::String(
                QuoteStyle::Unquoted.decode_body(text, 0),
            )),
            // 整数として解釈されたテキストでも i64 に収まらない場合がある。
            // 基準実装（hjson-cs の TryParseNumericLiteral）は long で読めなければ
            // double として扱うため、同じ挙動にする。
            SyntaxKind::Integer => match text.parse::<i64>() {
                Ok(value) => Ok(HjsonValue::Integer(value)),
                Err(_) => text.parse::<f64>().map(HjsonValue::Float).map_err(|_| {
                    HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Float,
                        found: token.kind(),
                    }
                }),
            },
            SyntaxKind::Float => text.parse::<f64>().map(HjsonValue::Float).map_err(|_| {
                HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Float,
                    found: token.kind(),
                }
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
                // 重複キーは後勝ち（基準実装の JsonObject と同じ）。位置は最初の出現を保つ。
                let mut entries: Vec<(String, HjsonValue)> = Vec::new();
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
                    let parsed = Self::from_node(&value)?;
                    match entries.iter_mut().find(|(existing, _)| *existing == key) {
                        Some(entry) => entry.1 = parsed,
                        None => entries.push((key, parsed)),
                    }
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
            if QuoteStyle::Unquoted.is_valid_for_text(key) {
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
    (style.syntax_kind(), style.render(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_simple_unquoted_text_accepts_alphanumeric_underscore_dash_dot() {
        assert!(QuoteStyle::Unquoted.is_valid_for_text("hello_world"));
        assert!(QuoteStyle::Unquoted.is_valid_for_text("foo"));
        assert!(QuoteStyle::Unquoted.is_valid_for_text("a-b.c_d-e0"));
    }

    #[test]
    fn test_is_simple_unquoted_text_rejects_space_and_punctuation() {
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("hello world"));
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("foo!"));
        assert!(!QuoteStyle::Unquoted.is_valid_for_text(""));
    }

    #[test]
    fn test_is_simple_unquoted_text_rejects_reserved_words() {
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("true"));
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("false"));
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("null"));
    }

    #[test]
    fn test_is_simple_unquoted_text_rejects_numeric_tokens() {
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("123"));
        assert!(!QuoteStyle::Unquoted.is_valid_for_text("1.23"));
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
        };
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body[0].kind(), SyntaxKind::String.into());
    }

    #[test]
    fn test_render_body_string_block_style() {
        let config = RenderConfig {
            string_style: Some(QuoteStyle::Block),
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
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn test_render_body_object_empty() {
        let config = RenderConfig::default();
        let obj = HjsonValue::Object(vec![]);
        let body = obj.render_body(&config);
        assert_eq!(body.len(), 1);
    }
}
