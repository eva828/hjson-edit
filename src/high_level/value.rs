use rowan::{GreenNode, GreenNodeBuilder, NodeOrToken};

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
    pub fn value_to_green(&self) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Value.into());
        match self {
            HjsonValue::String(s) => {
                let text = format!("\"{}\"", s);
                builder.token(SyntaxKind::String.into(), text.as_str())
            }
            HjsonValue::Integer(n) => {
                let text = n.to_string();
                builder.token(SyntaxKind::Integer.into(), text.as_str())
            }
            HjsonValue::Float(n) => {
                let text = n.to_string();
                builder.token(SyntaxKind::Float.into(), text.as_str())
            }
            HjsonValue::Bool(true) => builder.token(SyntaxKind::True.into(), "true"),
            HjsonValue::Bool(false) => builder.token(SyntaxKind::False.into(), "false"),
            HjsonValue::Null => builder.token(SyntaxKind::Null.into(), "null"),
            HjsonValue::Array(items) => {
                builder.start_node(SyntaxKind::Array.into());
                builder.token(SyntaxKind::LBracket.into(), "[");
                for (idx, item) in items.iter().enumerate() {
                    push_node(&mut builder, &item.value_to_green());
                    if idx + 1 != items.len() {
                        builder.token(SyntaxKind::Comma.into(), ",");
                    }
                }
                builder.token(SyntaxKind::RBracket.into(), "]");
                builder.finish_node();
            }
            HjsonValue::Object(entries) => {
                builder.start_node(SyntaxKind::Object.into());
                builder.token(SyntaxKind::LBrace.into(), "{");
                for (idx, (key, item)) in entries.iter().enumerate() {
                    push_node(&mut builder, &item.pair_to_green(key));
                    if idx + 1 != entries.len() {
                        builder.token(SyntaxKind::Comma.into(), ",");
                    }
                }
                builder.token(SyntaxKind::RBrace.into(), "}");
                builder.finish_node();
            }
        }
        builder.finish_node();
        builder.finish()
    }

    pub fn to_green(&self) -> GreenNode {
        self.value_to_green()
    }

    pub fn pair_to_green(&self, key: &str) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Pair.into());
        builder.start_node(SyntaxKind::Key.into());
        builder.token(SyntaxKind::UnquotedString.into(), key);
        builder.finish_node();
        builder.token(SyntaxKind::Colon.into(), ":");
        push_node(&mut builder, &self.value_to_green());
        builder.finish_node();
        builder.finish()
    }
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
