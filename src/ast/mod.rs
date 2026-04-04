use rowan::NodeOrToken;

use crate::{HjsonEditError, RedNode, RedToken, SyntaxKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NewlineKind {
    #[default]
    Lf,
    CrLf,
}

impl NewlineKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "\n" => Some(Self::Lf),
            "\r\n" => Some(Self::CrLf),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuoteStyle {
    DoubleQuoted,
    Block,
    Unquoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    Line,
    Hash,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentTrivia {
    pub kind: CommentKind,
    pub text: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trivia {
    Whitespace(String),
    Newline { kind: NewlineKind, text: String },
    Comment(CommentTrivia),
    Comma(String),
}

impl Trivia {
    pub fn is_comment(&self) -> bool {
        matches!(self, Self::Comment(_))
    }

    pub fn newline_kind(&self) -> Option<NewlineKind> {
        match self {
            Self::Newline { kind, .. } => Some(*kind),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Layout {
    pub depth: usize,
    pub is_last: bool,
    pub owns_trivia: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringValue {
    raw: String,
    body: String,
    quote_style: QuoteStyle,
}

impl StringValue {
    pub fn raw_text(&self) -> &str {
        &self.raw
    }

    pub fn body_text(&self) -> &str {
        &self.body
    }

    pub fn quote_style(&self) -> QuoteStyle {
        self.quote_style
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValue {
    String(StringValue),
    Integer { raw: String, parsed: Option<i64> },
    Float { raw: String, parsed: Option<f64> },
    Bool(bool),
    Null,
}

impl ScalarValue {
    pub fn raw_text(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.raw_text()),
            Self::Integer { raw, .. } | Self::Float { raw, .. } => Some(raw),
            Self::Bool(true) => Some("true"),
            Self::Bool(false) => Some("false"),
            Self::Null => Some("null"),
        }
    }

    pub fn quote_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::String(s) => Some(s.quote_style()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueContent {
    Object(Object),
    Array(Array),
    Scalar(ScalarValue),
}

impl ValueContent {
    pub fn raw_text(&self) -> Option<&str> {
        match self {
            Self::Scalar(s) => s.raw_text(),
            _ => None,
        }
    }

    pub fn quote_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::Scalar(s) => s.quote_style(),
            _ => None,
        }
    }
}

pub trait AstNode: Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(node: RedNode) -> Result<Self, HjsonEditError>;
    fn syntax(&self) -> &RedNode;

    fn node(&self) -> &RedNode {
        self.syntax()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    syntax: RedNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
}

pub type Pair = Member;

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentItem {
    Object(Object),
    Array(Array),
    Value(Value),
}

impl AstNode for Document {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::Root | SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value
        )
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Root,
                found: node.kind(),
            });
        }

        let layout = collect_layout(&node, true);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: collect_root_leading_trivia(&node),
            trailing_trivia: collect_root_trailing_trivia(&node),
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Document {
    pub fn item(&self) -> Option<DocumentItem> {
        match self.syntax.kind() {
            SyntaxKind::Root => {
                let child = self
                    .syntax
                    .children()
                    .find(|child| !child.kind().is_trivia())?;
                match child.kind() {
                    SyntaxKind::Object => Some(DocumentItem::Object(Object::cast(child).ok()?)),
                    SyntaxKind::Array => Some(DocumentItem::Array(Array::cast(child).ok()?)),
                    SyntaxKind::Value => Some(DocumentItem::Value(Value::cast(child).ok()?)),
                    _ => None,
                }
            }
            SyntaxKind::Object => Some(DocumentItem::Object(
                Object::cast(self.syntax.clone()).ok()?,
            )),
            SyntaxKind::Array => Some(DocumentItem::Array(Array::cast(self.syntax.clone()).ok()?)),
            SyntaxKind::Value => Some(DocumentItem::Value(Value::cast(self.syntax.clone()).ok()?)),
            _ => None,
        }
    }

    pub fn object(&self) -> Option<Object> {
        match self.item()? {
            DocumentItem::Object(object) => Some(object),
            DocumentItem::Value(value) => match value.content().ok()? {
                ValueContent::Object(object) => Some(object),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn array(&self) -> Option<Array> {
        match self.item()? {
            DocumentItem::Array(array) => Some(array),
            DocumentItem::Value(value) => match value.content().ok()? {
                ValueContent::Array(array) => Some(array),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn value(&self) -> Option<Value> {
        match self.item()? {
            DocumentItem::Value(value) => Some(value),
            _ => None,
        }
    }
}

impl AstNode for Object {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Object
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: node.kind(),
            });
        }

        let owns_trivia = !matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: collect_leading_trivia(&node, layout.owns_trivia),
            trailing_trivia: collect_trailing_trivia(&node, layout.owns_trivia),
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Object {
    pub fn members(&self) -> Vec<Member> {
        self.syntax
            .children()
            .filter(|child| child.kind() == SyntaxKind::Pair)
            .filter_map(|child| Member::cast(child).ok())
            .collect()
    }

    pub fn member_by_key(&self, key: &str) -> Option<Member> {
        self.members()
            .into_iter()
            .find(|member| member.pair_key().as_deref() == Some(key))
    }
}

impl AstNode for Array {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Array
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: node.kind(),
            });
        }

        let owns_trivia = !matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: collect_leading_trivia(&node, layout.owns_trivia),
            trailing_trivia: collect_trailing_trivia(&node, layout.owns_trivia),
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Array {
    pub fn items(&self) -> Vec<Value> {
        self.syntax
            .children()
            .filter(|child| child.kind() == SyntaxKind::Value)
            .filter_map(|child| Value::cast(child).ok())
            .collect()
    }
}

impl AstNode for Member {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Pair
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Pair,
                found: node.kind(),
            });
        }

        let layout = collect_layout(&node, true);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: collect_leading_trivia(&node, layout.owns_trivia),
            trailing_trivia: collect_trailing_trivia(&node, layout.owns_trivia),
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Member {
    pub fn key(&self) -> Option<Key> {
        self.syntax
            .children()
            .find(|child| child.kind() == SyntaxKind::Key)
            .and_then(|child| Key::cast(child).ok())
    }

    pub fn value(&self) -> Option<Value> {
        self.syntax
            .children()
            .find(|child| child.kind() == SyntaxKind::Value)
            .and_then(|child| Value::cast(child).ok())
    }

    pub fn pair_key(&self) -> Option<String> {
        self.key().and_then(|key| key.body_text())
    }
}

impl AstNode for Key {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Key
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Key,
                found: node.kind(),
            });
        }

        Ok(Self { syntax: node })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Key {
    pub fn token_text_and_kind(&self) -> Option<(SyntaxKind, String)> {
        self.syntax
            .children_with_tokens()
            .find_map(|elem| match elem {
                NodeOrToken::Token(token)
                    if matches!(
                        token.kind(),
                        SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString
                    ) =>
                {
                    Some((token.kind(), token.text().to_string()))
                }
                _ => None,
            })
    }

    pub fn raw_text(&self) -> Option<String> {
        self.token_text_and_kind().map(|(_, text)| text)
    }

    pub fn body_text(&self) -> Option<String> {
        self.syntax
            .children_with_tokens()
            .find_map(|elem| match elem {
                NodeOrToken::Token(token) => {
                    let text = token.text().to_string();
                    match token.kind() {
                        SyntaxKind::String => Some(strip_quotes(&text)),
                        SyntaxKind::BlockString => Some(strip_block(&text)),
                        SyntaxKind::UnquotedString => Some(text),
                        _ => None,
                    }
                }
                _ => None,
            })
    }
}

impl AstNode for Value {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Value
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            });
        }

        let owns_trivia = !matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: collect_leading_trivia(&node, layout.owns_trivia),
            trailing_trivia: collect_trailing_trivia(&node, layout.owns_trivia),
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Value {
    pub fn content(&self) -> Result<ValueContent, HjsonEditError> {
        let child = self
            .syntax
            .children_with_tokens()
            .find(|child| match child {
                NodeOrToken::Node(node) => !node.kind().is_trivia(),
                NodeOrToken::Token(token) => !token.kind().is_trivia(),
            })
            .ok_or(HjsonEditError::ParentNotFound)?;

        Ok(match child {
            NodeOrToken::Node(node) => match node.kind() {
                SyntaxKind::Object => ValueContent::Object(Object::cast(node)?),
                SyntaxKind::Array => ValueContent::Array(Array::cast(node)?),
                other => {
                    return Err(HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Value,
                        found: other,
                    });
                }
            },
            NodeOrToken::Token(token) => match token.kind() {
                SyntaxKind::String => ValueContent::Scalar(ScalarValue::String(StringValue {
                    raw: token.text().to_string(),
                    body: strip_quotes(&token.text().to_string()),
                    quote_style: QuoteStyle::DoubleQuoted,
                })),
                SyntaxKind::BlockString => ValueContent::Scalar(ScalarValue::String(StringValue {
                    raw: token.text().to_string(),
                    body: strip_block(&token.text().to_string()),
                    quote_style: QuoteStyle::Block,
                })),
                SyntaxKind::UnquotedString => {
                    ValueContent::Scalar(ScalarValue::String(StringValue {
                        raw: token.text().to_string(),
                        body: token.text().to_string(),
                        quote_style: QuoteStyle::Unquoted,
                    }))
                }
                SyntaxKind::Integer => ValueContent::Scalar(ScalarValue::Integer {
                    raw: token.text().to_string(),
                    parsed: token.text().to_string().parse::<i64>().ok(),
                }),
                SyntaxKind::Float => ValueContent::Scalar(ScalarValue::Float {
                    raw: token.text().to_string(),
                    parsed: token.text().to_string().parse::<f64>().ok(),
                }),
                SyntaxKind::True => ValueContent::Scalar(ScalarValue::Bool(true)),
                SyntaxKind::False => ValueContent::Scalar(ScalarValue::Bool(false)),
                SyntaxKind::Null => ValueContent::Scalar(ScalarValue::Null),
                other => {
                    return Err(HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Value,
                        found: other,
                    });
                }
            },
        })
    }
}

fn strip_quotes(text: &str) -> String {
    text.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(text)
        .to_string()
}

fn strip_block(text: &str) -> String {
    text.strip_prefix("'''")
        .and_then(|s| s.strip_suffix("'''"))
        .unwrap_or(text)
        .to_string()
}

fn collect_leading_trivia(node: &RedNode, enabled: bool) -> Vec<Trivia> {
    if !enabled {
        return Vec::new();
    }

    let children = if let Some(parent) = node.parent() {
        parent.children_with_tokens().collect::<Vec<_>>()
    } else {
        node.children_with_tokens().collect::<Vec<_>>()
    };

    let index = node.index();
    if index == 0 {
        return Vec::new();
    }

    let mut trivia = Vec::new();
    let mut saw_comment = false;
    let mut saw_newline = false;

    for elem in children.iter().take(index).rev() {
        let Some(item) = trivia_from_element(elem) else {
            break;
        };

        match item {
            Trivia::Whitespace(_) => continue,
            Trivia::Comma(_) => break,
            Trivia::Comment(comment) => {
                saw_comment = true;
                trivia.push(Trivia::Comment(comment));
            }
            Trivia::Newline { kind, text } => {
                if saw_newline && !saw_comment {
                    break;
                }
                saw_newline = true;
                trivia.push(Trivia::Newline { kind, text });
            }
        }
    }

    trivia.reverse();
    if saw_comment {
        trivia
    } else {
        Vec::new()
    }
}

fn collect_root_leading_trivia(node: &RedNode) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let mut trivia = Vec::new();

    for elem in children.iter() {
        let Some(item) = trivia_from_element(elem) else {
            break;
        };

        match item {
            Trivia::Whitespace(text) => trivia.push(Trivia::Whitespace(text)),
            Trivia::Newline { kind, text } => trivia.push(Trivia::Newline { kind, text }),
            Trivia::Comment(comment) => trivia.push(Trivia::Comment(comment)),
            Trivia::Comma(_) => break,
        }
    }

    trivia
}

fn collect_trailing_trivia(node: &RedNode, enabled: bool) -> Vec<Trivia> {
    if !enabled {
        return Vec::new();
    }

    let children = if let Some(parent) = node.parent() {
        parent.children_with_tokens().collect::<Vec<_>>()
    } else {
        node.children_with_tokens().collect::<Vec<_>>()
    };

    let index = node.index();
    let mut trivia = Vec::new();
    for elem in children.iter().skip(index + 1) {
        let Some(item) = trivia_from_element(elem) else {
            break;
        };

        match item {
            Trivia::Whitespace(_) => continue,
            Trivia::Comma(text) => trivia.push(Trivia::Comma(text)),
            Trivia::Comment(comment) => {
                trivia.push(Trivia::Comment(comment));
            }
            Trivia::Newline { kind, text } => {
                trivia.push(Trivia::Newline { kind, text });
                break;
            }
        }
    }

    trivia
}

fn collect_root_trailing_trivia(node: &RedNode) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let mut trivia = Vec::new();

    for elem in children.iter().rev() {
        let Some(item) = trivia_from_element(elem) else {
            break;
        };

        match item {
            Trivia::Whitespace(text) => trivia.push(Trivia::Whitespace(text)),
            Trivia::Newline { kind, text } => trivia.push(Trivia::Newline { kind, text }),
            Trivia::Comment(comment) => trivia.push(Trivia::Comment(comment)),
            Trivia::Comma(_) => break,
        }
    }

    trivia.reverse();
    trivia
}

fn collect_layout(node: &RedNode, owns_trivia: bool) -> Layout {
    let mut depth = 0usize;
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), SyntaxKind::Object | SyntaxKind::Array) {
            depth += 1;
        }
        current = parent.parent();
    }

    let is_last = node.parent().is_some_and(|parent| {
        let mut last_child = None;
        for child in parent.children() {
            if !child.kind().is_trivia() {
                last_child = Some(child);
            }
        }
        last_child.is_some_and(|child| child == *node)
    });

    Layout {
        depth,
        is_last,
        owns_trivia,
    }
}

fn trivia_from_element(element: &NodeOrToken<RedNode, RedToken>) -> Option<Trivia> {
    match element {
        NodeOrToken::Token(token) => match token.kind() {
            SyntaxKind::Whitespace => Some(Trivia::Whitespace(token.text().to_string())),
            SyntaxKind::Newline => Some(Trivia::Newline {
                kind: NewlineKind::from_text(token.text()).unwrap_or_default(),
                text: token.text().to_string(),
            }),
            SyntaxKind::LineComment => Some(Trivia::Comment(CommentTrivia {
                kind: CommentKind::Line,
                text: token.text().to_string(),
                body: strip_line_comment(token.text()),
            })),
            SyntaxKind::HashComment => Some(Trivia::Comment(CommentTrivia {
                kind: CommentKind::Hash,
                text: token.text().to_string(),
                body: strip_hash_comment(token.text()),
            })),
            SyntaxKind::BlockComment => Some(Trivia::Comment(CommentTrivia {
                kind: CommentKind::Block,
                text: token.text().to_string(),
                body: strip_block_comment(token.text()),
            })),
            SyntaxKind::Comma => Some(Trivia::Comma(token.text().to_string())),
            _ => None,
        },
        NodeOrToken::Node(_) => None,
    }
}

fn strip_line_comment(text: &str) -> String {
    text.strip_prefix("//")
        .unwrap_or(text)
        .trim_start()
        .to_string()
}

fn strip_hash_comment(text: &str) -> String {
    text.strip_prefix('#')
        .unwrap_or(text)
        .trim_start()
        .to_string()
}

fn strip_block_comment(text: &str) -> String {
    text.strip_prefix("/*")
        .and_then(|s| s.strip_suffix("*/"))
        .unwrap_or(text)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_style_roundtrip() {
        let doc = crate::high_level::HjsonDocument::parse("{ name: \"value\" }\n").unwrap();
        let ast = doc.ast().unwrap();
        let value = ast
            .object()
            .unwrap()
            .member_by_key("name")
            .unwrap()
            .value()
            .unwrap();

        match value.content().unwrap() {
            ValueContent::Scalar(ScalarValue::String(string)) => {
                assert_eq!(string.quote_style(), QuoteStyle::DoubleQuoted);
                assert_eq!(string.body_text(), "value");
            }
            other => panic!("unexpected value: {other:?}"),
        }
    }
}
