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
    pub fn render_value(&self, config: &RenderConfig<'_>) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Value.into());
        self.render_body(&mut builder, config);
        builder.finish_node();
        builder.finish()
    }

    pub fn render_member(
        &self,
        key: &str,
        key_style: Option<QuoteStyle>,
        config: &RenderConfig<'_>,
    ) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Member.into());
        render_key(&mut builder, key, key_style);
        builder.token(SyntaxKind::Colon.into(), ":");
        builder.token(SyntaxKind::Whitespace.into(), " ");
        push_node(
            &mut builder,
            &self.render_value(&config.with_depth(config.depth + 1)),
        );
        builder.finish_node();
        builder.finish()
    }

    fn render_body(&self, builder: &mut GreenNodeBuilder<'static>, config: &RenderConfig<'_>) {
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
                builder.token(kind.into(), text.as_str());
            }
            HjsonValue::Integer(n) => {
                let text = n.to_string();
                builder.token(SyntaxKind::Integer.into(), text.as_str());
            }
            HjsonValue::Float(n) => {
                let text = n.to_string();
                builder.token(SyntaxKind::Float.into(), text.as_str());
            }
            HjsonValue::Bool(true) => {
                builder.token(SyntaxKind::True.into(), "true");
            }
            HjsonValue::Bool(false) => {
                builder.token(SyntaxKind::False.into(), "false");
            }
            HjsonValue::Null => {
                builder.token(SyntaxKind::Null.into(), "null");
            }
            HjsonValue::Array(items) => {
                builder.start_node(SyntaxKind::Array.into());
                builder.token(SyntaxKind::LBracket.into(), "[");
                if config.pretty && !items.is_empty() {
                    builder.token(SyntaxKind::Newline.into(), config.newline);
                    for (idx, item) in items.iter().enumerate() {
                        builder.token(
                            SyntaxKind::Whitespace.into(),
                            indent(config.depth + 1).as_str(),
                        );
                        builder.start_node(SyntaxKind::Value.into());
                        item.render_body(builder, &config.with_depth(config.depth + 1));
                        builder.finish_node();
                        if idx + 1 != items.len() {
                            builder.token(SyntaxKind::Newline.into(), config.newline);
                        }
                    }
                    builder.token(SyntaxKind::Newline.into(), config.newline);
                    builder.token(
                        SyntaxKind::Whitespace.into(),
                        indent(config.depth).as_str(),
                    );
                    builder.token(SyntaxKind::RBracket.into(), "]");
                } else {
                    for (idx, item) in items.iter().enumerate() {
                        builder.start_node(SyntaxKind::Value.into());
                        item.render_body(builder, config);
                        builder.finish_node();
                        if idx + 1 != items.len() {
                            builder.token(SyntaxKind::Comma.into(), ",");
                        }
                    }
                    builder.token(SyntaxKind::RBracket.into(), "]");
                }
                builder.finish_node();
            }
            HjsonValue::Object(entries) => {
                builder.start_node(SyntaxKind::Object.into());
                builder.token(SyntaxKind::LBrace.into(), "{");
                if config.pretty && !entries.is_empty() {
                    builder.token(SyntaxKind::Newline.into(), config.newline);
                    for (idx, (key, value)) in entries.iter().enumerate() {
                        builder.token(
                            SyntaxKind::Whitespace.into(),
                            indent(config.depth + 1).as_str(),
                        );
                        builder.start_node(SyntaxKind::Member.into());
                        render_key(builder, key, None);
                        builder.token(SyntaxKind::Colon.into(), ":");
                        builder.token(SyntaxKind::Whitespace.into(), " ");
                        push_node(
                            builder,
                            &value.render_value(&config.with_depth(config.depth + 2)),
                        );
                        builder.finish_node();
                        if idx + 1 != entries.len() {
                            builder.token(SyntaxKind::Newline.into(), config.newline);
                        }
                    }
                    builder.token(SyntaxKind::Newline.into(), config.newline);
                    builder.token(
                        SyntaxKind::Whitespace.into(),
                        indent(config.depth).as_str(),
                    );
                    builder.token(SyntaxKind::RBrace.into(), "}");
                } else {
                    for (idx, (key, value)) in entries.iter().enumerate() {
                        push_node(
                            builder,
                            &value.render_member(key, None, config),
                        );
                        if idx + 1 != entries.len() {
                            builder.token(SyntaxKind::Comma.into(), ",");
                        }
                    }
                    builder.token(SyntaxKind::RBrace.into(), "}");
                }
                builder.finish_node();
            }
        }
    }
}

fn render_key(
    builder: &mut GreenNodeBuilder<'static>,
    key: &str,
    key_style: Option<QuoteStyle>,
) {
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
