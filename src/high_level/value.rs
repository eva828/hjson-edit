use rowan::{GreenNode, Language, NodeOrToken};

use crate::high_level::ops::object::StringStyle;
use crate::{syntax::HjsonLanguage, RedNode, SyntaxKind};

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
    pub fn to_hjson_string(&self) -> String {
        match self {
            HjsonValue::String(s) => render_hjson_string(s),
            HjsonValue::Integer(n) => n.to_string(),
            HjsonValue::Float(f) => f.to_string(),
            HjsonValue::Bool(b) => b.to_string(),
            HjsonValue::Null => "null".to_string(),
            HjsonValue::Array(items) => {
                let items = items
                    .iter()
                    .map(|v| v.to_hjson_string())
                    .collect::<Vec<_>>();
                format!("[{}]", items.join(", "))
            }
            HjsonValue::Object(entries) => {
                let entries = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", render_hjson_key(k), v.to_hjson_string()))
                    .collect::<Vec<_>>();
                format!("{{{}}}", entries.join(", "))
            }
        }
    }

    pub fn to_pretty_hjson_string(&self, indent_unit: &str, newline: &str, depth: usize) -> String {
        match self {
            HjsonValue::String(s) => render_hjson_string(s),
            HjsonValue::Integer(n) => n.to_string(),
            HjsonValue::Float(f) => f.to_string(),
            HjsonValue::Bool(b) => b.to_string(),
            HjsonValue::Null => "null".to_string(),
            HjsonValue::Array(items) => {
                if items.is_empty() {
                    return "[]".to_string();
                }

                let child_indent = indent_unit.repeat(depth + 1);
                let end_indent = indent_unit.repeat(depth);
                let mut out = String::from("[");
                out.push_str(newline);
                for item in items {
                    out.push_str(&child_indent);
                    out.push_str(&item.to_pretty_hjson_string(indent_unit, newline, depth + 1));
                    out.push_str(newline);
                }
                out.push_str(&end_indent);
                out.push(']');
                out
            }
            HjsonValue::Object(entries) => {
                if entries.is_empty() {
                    return "{}".to_string();
                }

                let child_indent = indent_unit.repeat(depth + 1);
                let end_indent = indent_unit.repeat(depth);
                let mut out = String::from("{");
                out.push_str(newline);
                for (key, value) in entries {
                    out.push_str(&child_indent);
                    out.push_str(&render_hjson_key(key));
                    out.push_str(": ");
                    out.push_str(&value.to_pretty_hjson_string(indent_unit, newline, depth + 1));
                    out.push_str(newline);
                }
                out.push_str(&end_indent);
                out.push('}');
                out
            }
        }
    }

    pub fn to_pretty_hjson_string_with_style(
        &self,
        indent_unit: &str,
        newline: &str,
        depth: usize,
        string_style: StringStyle,
    ) -> String {
        match self {
            HjsonValue::String(s) => render_hjson_string_with_style(s, string_style, None),
            HjsonValue::Integer(n) => n.to_string(),
            HjsonValue::Float(f) => f.to_string(),
            HjsonValue::Bool(b) => b.to_string(),
            HjsonValue::Null => "null".to_string(),
            HjsonValue::Array(items) => {
                if items.is_empty() {
                    return "[]".to_string();
                }

                let child_indent = indent_unit.repeat(depth + 1);
                let end_indent = indent_unit.repeat(depth);
                let mut out = String::from("[");
                out.push_str(newline);
                for item in items {
                    out.push_str(&child_indent);
                    out.push_str(&item.to_pretty_hjson_string_with_style(
                        indent_unit,
                        newline,
                        depth + 1,
                        string_style,
                    ));
                    out.push_str(newline);
                }
                out.push_str(&end_indent);
                out.push(']');
                out
            }
            HjsonValue::Object(entries) => {
                if entries.is_empty() {
                    return "{}".to_string();
                }

                let child_indent = indent_unit.repeat(depth + 1);
                let end_indent = indent_unit.repeat(depth);
                let mut out = String::from("{");
                out.push_str(newline);
                for (key, value) in entries {
                    out.push_str(&child_indent);
                    out.push_str(&render_hjson_key(key));
                    out.push_str(": ");
                    out.push_str(&value.to_pretty_hjson_string_with_style(
                        indent_unit,
                        newline,
                        depth + 1,
                        string_style,
                    ));
                    out.push_str(newline);
                }
                out.push_str(&end_indent);
                out.push('}');
                out
            }
        }
    }

    pub fn to_pretty_hjson_string_with_style_from_existing(
        &self,
        indent_unit: &str,
        newline: &str,
        depth: usize,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> String {
        match self {
            HjsonValue::String(s) => {
                render_hjson_string_with_style(s, string_style, existing_style)
            }
            HjsonValue::Integer(n) => n.to_string(),
            HjsonValue::Float(f) => f.to_string(),
            HjsonValue::Bool(b) => b.to_string(),
            HjsonValue::Null => "null".to_string(),
            HjsonValue::Array(items) => {
                if items.is_empty() {
                    return "[]".to_string();
                }

                let child_indent = indent_unit.repeat(depth + 1);
                let end_indent = indent_unit.repeat(depth);
                let mut out = String::from("[");
                out.push_str(newline);
                for item in items {
                    out.push_str(&child_indent);
                    out.push_str(&item.to_pretty_hjson_string_with_style_from_existing(
                        indent_unit,
                        newline,
                        depth + 1,
                        string_style,
                        None,
                    ));
                    out.push_str(newline);
                }
                out.push_str(&end_indent);
                out.push(']');
                out
            }
            HjsonValue::Object(entries) => {
                if entries.is_empty() {
                    return "{}".to_string();
                }

                let child_indent = indent_unit.repeat(depth + 1);
                let end_indent = indent_unit.repeat(depth);
                let mut out = String::from("{");
                out.push_str(newline);
                for (key, value) in entries {
                    out.push_str(&child_indent);
                    out.push_str(&render_hjson_key(key));
                    out.push_str(": ");
                    out.push_str(&value.to_pretty_hjson_string_with_style_from_existing(
                        indent_unit,
                        newline,
                        depth + 1,
                        string_style,
                        None,
                    ));
                    out.push_str(newline);
                }
                out.push_str(&end_indent);
                out.push('}');
                out
            }
        }
    }

    pub fn to_hjson_string_with_style(
        &self,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> String {
        match self {
            HjsonValue::String(s) => {
                render_hjson_string_with_style(s, string_style, existing_style)
            }
            _ => self.to_hjson_string(),
        }
    }

    pub fn from_rednode(v: RedNode) -> Self {
        match v.kind() {
            SyntaxKind::Value => v
                .children_with_tokens()
                .next()
                .map(|child| match child {
                    NodeOrToken::Node(node) => Self::from_rednode(node),
                    NodeOrToken::Token(token) => {
                        Self::from_token(token.kind(), token.text().to_string())
                    }
                })
                .unwrap_or(HjsonValue::Null),
            SyntaxKind::Integer => {
                let text = v.text().to_string();
                let value = text.parse::<i64>().unwrap_or_else(|e| {
                    panic!("Failed to parse integer literal `{}` as i64: {}", text, e);
                });
                HjsonValue::from(value)
            }
            SyntaxKind::Float => {
                let text = v.text().to_string();
                let value = text.parse::<f64>().unwrap_or_else(|e| {
                    panic!("Failed to parse float literal `{}` as f64: {}", text, e);
                });
                HjsonValue::from(value)
            }
            SyntaxKind::True => HjsonValue::Bool(true),
            SyntaxKind::False => HjsonValue::Bool(false),
            SyntaxKind::String => {
                let text = v.text().to_string();
                HjsonValue::String(text.trim_matches('"').to_string())
            }
            SyntaxKind::UnquotedString => HjsonValue::String(v.text().to_string()),
            SyntaxKind::BlockString => HjsonValue::String(v.text().to_string()),
            SyntaxKind::Object => {
                let pairs = v
                    .children()
                    .filter(|child| {
                        HjsonLanguage::kind_from_raw(child.kind().into()) == SyntaxKind::Pair
                    })
                    .map(|child| {
                        let key = child
                            .children()
                            .find(|child| {
                                HjsonLanguage::kind_from_raw(child.kind().into()) == SyntaxKind::Key
                            })
                            .map(|child| {
                                let text = child.text().to_string();
                                text.trim_matches('"').to_string()
                            })
                            .unwrap_or_default();
                        let value = child
                            .children()
                            .find(|child| {
                                HjsonLanguage::kind_from_raw(child.kind().into())
                                    == SyntaxKind::Value
                            })
                            .map(|child| Self::from_rednode(child))
                            .unwrap_or(HjsonValue::Null);
                        (key, value)
                    });
                HjsonValue::Object(pairs.collect())
            }
            SyntaxKind::Array => HjsonValue::Array(
                v.children()
                    .filter(|child| {
                        HjsonLanguage::kind_from_raw(child.kind().into()) == SyntaxKind::Value
                    })
                    .map(|child| Self::from_rednode(child))
                    .collect(),
            ),
            _ => {
                unimplemented!("Unsupported syntax kind: {:?}", v.kind())
            }
        }
    }

    fn from_token(kind: SyntaxKind, text: String) -> Self {
        match kind {
            SyntaxKind::Integer => HjsonValue::from(text.parse::<i64>().unwrap_or_default()),
            SyntaxKind::Float => HjsonValue::from(text.parse::<f64>().unwrap_or_default()),
            SyntaxKind::True => HjsonValue::Bool(true),
            SyntaxKind::False => HjsonValue::Bool(false),
            SyntaxKind::String => HjsonValue::String(text.trim_matches('"').to_string()),
            SyntaxKind::UnquotedString => HjsonValue::String(text),
            SyntaxKind::BlockString => HjsonValue::String(text),
            SyntaxKind::Null => HjsonValue::Null,
            _ => HjsonValue::Null,
        }
    }

    pub fn into_greennode(self) -> GreenNode {
        let text = self.to_hjson_string();
        let doc =
            crate::HjsonDocument::parse(&text).expect("generated HJSON value should always parse");

        doc.root()
            .children()
            .next()
            .expect("parsed generated HJSON should have a value node")
            .green()
            .clone()
            .into()
    }

    pub fn into_greennode_with_style(
        self,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> GreenNode {
        let text = self.to_hjson_string_with_style(string_style, existing_style);
        let doc =
            crate::HjsonDocument::parse(&text).expect("generated HJSON value should always parse");

        doc.root()
            .children()
            .next()
            .expect("parsed generated HJSON should have a value node")
            .green()
            .clone()
            .into()
    }
}

fn render_hjson_key(s: &str) -> String {
    if is_unquoted_ident(s) {
        s.to_string()
    } else {
        format!("\"{}\"", escape_string(s))
    }
}

fn render_hjson_string(s: &str) -> String {
    if is_unquoted_ident(s) {
        s.to_string()
    } else {
        format!("\"{}\"", escape_string(s))
    }
}

fn render_hjson_string_with_style(
    s: &str,
    style: StringStyle,
    existing_style: Option<StringStyle>,
) -> String {
    match style {
        StringStyle::Preserve => match existing_style.unwrap_or(StringStyle::Unquoted) {
            StringStyle::DoubleQuoted => render_double_quoted_string(s),
            StringStyle::Block => render_block_string(s),
            StringStyle::Unquoted | StringStyle::Preserve => render_unquoted_string(s),
        },
        StringStyle::DoubleQuoted => render_double_quoted_string(s),
        StringStyle::Block => render_block_string(s),
        StringStyle::Unquoted => render_unquoted_string(s),
    }
}

fn render_double_quoted_string(s: &str) -> String {
    format!("\"{}\"", escape_string(s))
}

fn render_block_string(s: &str) -> String {
    if s.contains("'''") {
        render_double_quoted_string(s)
    } else {
        format!("'''{}'''", s)
    }
}

fn render_unquoted_string(s: &str) -> String {
    if is_unquoted_ident(s) {
        s.to_string()
    } else if s.contains('\n') || s.contains('\r') {
        render_block_string(s)
    } else {
        render_double_quoted_string(s)
    }
}

fn is_unquoted_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn escape_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
