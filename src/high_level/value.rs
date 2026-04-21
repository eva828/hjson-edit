use rowan::{GreenNode, GreenNodeBuilder, NodeOrToken};

use crate::ast::QuoteStyle;
use crate::syntax::SyntaxKind;

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
    pub fn render_content(&self, string_style: Option<QuoteStyle>) -> crate::GreenElement {
        match self {
            HjsonValue::String(s) => {
                let style = string_style.unwrap_or(QuoteStyle::DoubleQuoted);
                let (kind, text) = render_string(s, style);
                kind.create_token(text).into()
            }
            HjsonValue::Integer(n) => {
                let text = n.to_string();
                SyntaxKind::Integer.create_token(text).into()
            }
            HjsonValue::Float(n) => {
                let text = n.to_string();
                SyntaxKind::Float.create_token(text).into()
            }
            HjsonValue::Bool(true) => SyntaxKind::True.create_token("true").into(),
            HjsonValue::Bool(false) => SyntaxKind::False.create_token("false").into(),
            HjsonValue::Null => SyntaxKind::Null.create_token("null").into(),
            HjsonValue::Array(items) => {
                let mut builder = GreenNodeBuilder::new();
                builder.start_node(SyntaxKind::Array.into());
                builder.token(SyntaxKind::LBracket.into(), "[");
                for (idx, item) in items.iter().enumerate() {
                    push_node(&mut builder, &item.render_value(string_style));
                    if idx + 1 != items.len() {
                        builder.token(SyntaxKind::Comma.into(), ",");
                    }
                }
                builder.token(SyntaxKind::RBracket.into(), "]");
                builder.finish_node();
                builder.finish().into()
            }
            HjsonValue::Object(entries) => {
                let mut builder = GreenNodeBuilder::new();
                builder.start_node(SyntaxKind::Object.into());
                builder.token(SyntaxKind::LBrace.into(), "{");
                for (idx, (key, item)) in entries.iter().enumerate() {
                    push_node(&mut builder, &item.render_pair(key, None, string_style));
                    if idx + 1 != entries.len() {
                        builder.token(SyntaxKind::Comma.into(), ",");
                    }
                }
                builder.token(SyntaxKind::RBrace.into(), "}");
                builder.finish_node();
                builder.finish().into()
            }
        }
    }

    pub fn render_value(&self, string_style: Option<QuoteStyle>) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Value.into());
        push_green_element(&mut builder, &self.render_content(string_style));
        builder.finish_node();
        builder.finish()
    }

    pub fn render_pair(
        &self,
        key: &str,
        key_style: Option<QuoteStyle>,
        string_style: Option<QuoteStyle>,
    ) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Pair.into());
        builder.start_node(SyntaxKind::Key.into());
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
        builder.token(key_kind.into(), key_text.as_str());
        builder.finish_node();
        builder.token(SyntaxKind::Colon.into(), ":");
        builder.token(SyntaxKind::Whitespace.into(), " ");
        push_node(&mut builder, &self.render_value(string_style));
        builder.finish_node();
        builder.finish()
    }

    pub fn render_scalar(&self, string_style: Option<QuoteStyle>) -> GreenNode {
        self.render_value(string_style)
    }
}

fn render_string(text: &str, style: QuoteStyle) -> (SyntaxKind, String) {
    match style {
        QuoteStyle::DoubleQuoted => (
            SyntaxKind::String,
            format!("\"{}\"", escape_double_quoted(text)),
        ),
        QuoteStyle::Block => (SyntaxKind::BlockString, format!("'''{}'''", text)),
        QuoteStyle::Unquoted => (SyntaxKind::UnquotedString, text.to_string()),
    }
}

fn is_simple_unquoted_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn escape_double_quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn push_node(builder: &mut GreenNodeBuilder<'static>, node: &GreenNode) {
    builder.start_node(node.kind());
    for child in node.children() {
        match child {
            NodeOrToken::Node(n) => push_node(builder, &n.to_owned()),
            NodeOrToken::Token(t) => builder.token(t.kind(), t.text()),
        }
    }
    builder.finish_node();
}

fn push_green_element(builder: &mut GreenNodeBuilder<'static>, element: &crate::GreenElement) {
    match element {
        rowan::NodeOrToken::Node(node) => push_node(builder, node),
        rowan::NodeOrToken::Token(token) => builder.token(token.kind(), token.text()),
    }
}
