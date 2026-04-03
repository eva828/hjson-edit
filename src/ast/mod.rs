//! Lossless HJSON abstract tree.
//!
//! This layer is a typed, semantic view over the lossless rowan syntax tree.
//! It keeps the original syntax nodes alive, while exposing comments, newline
//! style, and string quote style through higher-level accessors.

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

    pub fn from_newline_text(text: &str) -> Option<Self> {
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

    pub fn text(&self) -> &str {
        match self {
            Self::Whitespace(text) => text,
            Self::Newline { text, .. } => text,
            Self::Comment(comment) => &comment.text,
            Self::Comma(text) => text,
        }
    }

    pub fn comment(&self) -> Option<&CommentTrivia> {
        match self {
            Self::Comment(comment) => Some(comment),
            _ => None,
        }
    }

    pub fn newline_kind(&self) -> Option<NewlineKind> {
        match self {
            Self::Newline { kind, .. } => Some(*kind),
            _ => None,
        }
    }
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

    fn from_token(token: &RedToken) -> Option<Self> {
        let text = token.text().to_string();
        let kind = token_kind(token);

        match kind {
            SyntaxKind::String => Some(Self {
                body: text
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(&text)
                    .to_string(),
                raw: text,
                quote_style: QuoteStyle::DoubleQuoted,
            }),
            SyntaxKind::BlockString => Some(Self {
                body: text
                    .strip_prefix("'''")
                    .and_then(|s| s.strip_suffix("'''"))
                    .unwrap_or(&text)
                    .to_string(),
                raw: text,
                quote_style: QuoteStyle::Block,
            }),
            SyntaxKind::UnquotedString => Some(Self {
                body: text.clone(),
                raw: text,
                quote_style: QuoteStyle::Unquoted,
            }),
            _ => None,
        }
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
            Self::String(string) => Some(string.raw_text()),
            Self::Integer { raw, .. } | Self::Float { raw, .. } => Some(raw),
            Self::Bool(true) => Some("true"),
            Self::Bool(false) => Some("false"),
            Self::Null => Some("null"),
        }
    }

    pub fn quote_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::String(string) => Some(string.quote_style()),
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
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Array> {
        match self {
            Self::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn as_scalar(&self) -> Option<&ScalarValue> {
        match self {
            Self::Scalar(scalar) => Some(scalar),
            _ => None,
        }
    }

    pub fn raw_text(&self) -> Option<&str> {
        self.as_scalar().and_then(ScalarValue::raw_text)
    }

    pub fn quote_style(&self) -> Option<QuoteStyle> {
        self.as_scalar().and_then(ScalarValue::quote_style)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentItem {
    Object(Object),
    Array(Array),
    Value(Value),
}

impl DocumentItem {
    pub fn syntax(&self) -> &RedNode {
        match self {
            Self::Object(object) => object.syntax(),
            Self::Array(array) => array.syntax(),
            Self::Value(value) => value.syntax(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    syntax: RedNode,
}

impl AstNode for Document {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Root
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { syntax: node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Root,
                found: node.kind(),
            })
        }
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Document {
    pub fn item(&self) -> Option<DocumentItem> {
        let child = self
            .syntax
            .children()
            .find(|child| !child.kind().is_trivia())?;

        match child.kind() {
            SyntaxKind::Object => Some(DocumentItem::Object(Object { syntax: child })),
            SyntaxKind::Array => Some(DocumentItem::Array(Array { syntax: child })),
            SyntaxKind::Value => Some(DocumentItem::Value(Value { syntax: child })),
            _ => None,
        }
    }

    pub fn leading_trivia_elements(
        &self,
    ) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
        let mut trivia = Vec::new();

        for child in self.syntax.children_with_tokens() {
            if child_is_non_trivia(&child) {
                break;
            }
            trivia.push(child);
        }

        Ok(trivia)
    }

    pub fn trailing_trivia_elements(
        &self,
    ) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
        let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
        let mut trivia = Vec::new();

        for child in children.into_iter().rev() {
            if child_is_non_trivia(&child) {
                break;
            }
            trivia.push(child);
        }

        trivia.reverse();

        Ok(trivia)
    }

    pub fn leading_trivia(&self) -> Result<Vec<Trivia>, HjsonEditError> {
        Ok(self
            .leading_trivia_elements()?
            .into_iter()
            .filter_map(|element| trivia_from_element(&element, false))
            .collect::<Vec<_>>())
    }

    pub fn trailing_trivia(&self) -> Result<Vec<Trivia>, HjsonEditError> {
        Ok(self
            .trailing_trivia_elements()?
            .into_iter()
            .filter_map(|element| trivia_from_element(&element, true))
            .collect::<Vec<_>>())
    }

    pub fn content(&self) -> Option<ValueContent> {
        match self.item()? {
            DocumentItem::Object(object) => Some(ValueContent::Object(object)),
            DocumentItem::Array(array) => Some(ValueContent::Array(array)),
            DocumentItem::Value(value) => value.content().ok(),
        }
    }

    pub fn object(&self) -> Option<Object> {
        match self.content()? {
            ValueContent::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn array(&self) -> Option<Array> {
        match self.content()? {
            ValueContent::Array(array) => Some(array),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    syntax: RedNode,
}

impl AstNode for Object {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Object
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { syntax: node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: node.kind(),
            })
        }
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
            .map(|child| Member { syntax: child })
            .collect()
    }

    pub fn pairs(&self) -> Vec<Member> {
        self.members()
    }

    pub fn member_by_key(&self, key: &str) -> Option<Member> {
        self.members()
            .into_iter()
            .find(|member| member.pair_key().as_deref() == Some(key))
    }

    pub fn is_braced(&self) -> bool {
        let mut first = None;
        let mut last = None;

        for child in self.syntax.children_with_tokens() {
            if let Some(kind) = token_or_node_kind(&child) {
                if kind.is_trivia() {
                    continue;
                }
                first = Some(kind);
                break;
            }
        }

        let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
        for child in children.into_iter().rev() {
            if let Some(kind) = token_or_node_kind(&child) {
                if kind.is_trivia() {
                    continue;
                }
                last = Some(kind);
                break;
            }
        }

        first == Some(SyntaxKind::LBrace) && last == Some(SyntaxKind::RBrace)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Array {
    syntax: RedNode,
}

impl AstNode for Array {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Array
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { syntax: node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: node.kind(),
            })
        }
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
            .map(|child| Value { syntax: child })
            .collect()
    }

    pub fn values(&self) -> Vec<Value> {
        self.items()
    }

    pub fn is_bracketed(&self) -> bool {
        let mut first = None;
        let mut last = None;

        for child in self.syntax.children_with_tokens() {
            if let Some(kind) = token_or_node_kind(&child) {
                if kind.is_trivia() {
                    continue;
                }
                first = Some(kind);
                break;
            }
        }

        let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
        for child in children.into_iter().rev() {
            if let Some(kind) = token_or_node_kind(&child) {
                if kind.is_trivia() {
                    continue;
                }
                last = Some(kind);
                break;
            }
        }

        first == Some(SyntaxKind::LBracket) && last == Some(SyntaxKind::RBracket)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    syntax: RedNode,
}

pub type Pair = Member;

impl AstNode for Member {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Pair
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { syntax: node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Pair,
                found: node.kind(),
            })
        }
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
            .map(|syntax| Key { syntax })
    }

    pub fn value(&self) -> Option<Value> {
        self.syntax
            .children()
            .find(|child| child.kind() == SyntaxKind::Value)
            .map(|syntax| Value { syntax })
    }

    pub fn pair_key(&self) -> Option<String> {
        self.key().and_then(|key| key.body_text())
    }

    pub fn leading_trivia_elements(
        &self,
    ) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
        collect_leading_trivia_elements(&self.syntax)
    }

    pub fn trailing_trivia_elements(
        &self,
    ) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
        collect_trailing_trivia_elements(&self.syntax)
    }

    pub fn leading_trivia(&self) -> Result<Vec<Trivia>, HjsonEditError> {
        Ok(self
            .leading_trivia_elements()?
            .into_iter()
            .filter_map(|element| trivia_from_element(&element, false))
            .collect::<Vec<_>>())
    }

    pub fn trailing_trivia(&self) -> Result<Vec<Trivia>, HjsonEditError> {
        Ok(self
            .trailing_trivia_elements()?
            .into_iter()
            .filter_map(|element| trivia_from_element(&element, true))
            .collect::<Vec<_>>())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    syntax: RedNode,
}

impl AstNode for Key {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Key
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { syntax: node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Key,
                found: node.kind(),
            })
        }
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Key {
    pub fn string_value(&self) -> Option<StringValue> {
        self.syntax
            .children_with_tokens()
            .find_map(|element| match element {
                NodeOrToken::Token(token) => StringValue::from_token(&token),
                _ => None,
            })
    }

    pub fn raw_text(&self) -> Option<String> {
        self.string_value()
            .map(|value| value.raw_text().to_string())
    }

    pub fn body_text(&self) -> Option<String> {
        self.string_value()
            .map(|value| value.body_text().to_string())
    }

    pub fn quote_style(&self) -> Option<QuoteStyle> {
        self.string_value().map(|value| value.quote_style())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    syntax: RedNode,
}

impl AstNode for Value {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Value
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { syntax: node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            })
        }
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
            .find(|child| !child_is_trivia(child))
            .ok_or(HjsonEditError::ParentNotFound)?;

        match child {
            NodeOrToken::Node(node) => match node.kind() {
                SyntaxKind::Object => Ok(ValueContent::Object(Object { syntax: node })),
                SyntaxKind::Array => Ok(ValueContent::Array(Array { syntax: node })),
                _ => Err(HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Value,
                    found: node.kind(),
                }),
            },
            NodeOrToken::Token(token) => Ok(ValueContent::Scalar(scalar_from_token(&token)?)),
        }
    }

    pub fn quote_style(&self) -> Result<Option<QuoteStyle>, HjsonEditError> {
        Ok(self.content()?.quote_style())
    }

    pub fn raw_text(&self) -> Result<Option<String>, HjsonEditError> {
        Ok(self.content()?.raw_text().map(ToString::to_string))
    }
}

fn collect_leading_trivia_elements(
    node: &RedNode,
) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
    let parent = node.parent().ok_or(HjsonEditError::ParentNotFound)?;
    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let index = node.index();
    let mut trivia = Vec::new();

    let mut prev_is_newline = false;
    for i in (0..index).rev() {
        let sibling = children
            .get(i)
            .ok_or(HjsonEditError::ParentNotFound)?
            .clone();

        let Some(item) = trivia_from_element(&sibling, false) else {
            break;
        };

        if matches!(item, Trivia::Newline { .. }) {
            if prev_is_newline {
                break;
            }
            prev_is_newline = true;
        }

        trivia.push(sibling);
    }

    Ok(trivia)
}

fn collect_trailing_trivia_elements(
    node: &RedNode,
) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
    let parent = node.parent().ok_or(HjsonEditError::ParentNotFound)?;
    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let index = node.index();
    let mut trivia = Vec::new();

    for i in index + 1..children.len() {
        let sibling = children
            .get(i)
            .ok_or(HjsonEditError::ParentNotFound)?
            .clone();

        let Some(item) = trivia_from_element(&sibling, true) else {
            break;
        };

        trivia.push(sibling);

        if matches!(item, Trivia::Newline { .. }) {
            break;
        }
    }

    Ok(trivia)
}

fn child_is_non_trivia(element: &NodeOrToken<RedNode, RedToken>) -> bool {
    match element {
        NodeOrToken::Node(node) => !node.kind().is_trivia(),
        NodeOrToken::Token(token) => !token_kind(token).is_trivia(),
    }
}

fn child_is_trivia(element: &NodeOrToken<RedNode, RedToken>) -> bool {
    match element {
        NodeOrToken::Node(node) => node.kind().is_trivia(),
        NodeOrToken::Token(token) => token_kind(token).is_trivia(),
    }
}

fn token_kind(token: &RedToken) -> SyntaxKind {
    token.kind()
}

fn token_or_node_kind(element: &NodeOrToken<RedNode, RedToken>) -> Option<SyntaxKind> {
    match element {
        NodeOrToken::Node(node) => Some(node.kind()),
        NodeOrToken::Token(token) => Some(token_kind(token)),
    }
}

fn trivia_from_element(
    element: &NodeOrToken<RedNode, RedToken>,
    include_comma: bool,
) -> Option<Trivia> {
    match element {
        NodeOrToken::Node(node) if node.kind().is_trivia() => None,
        NodeOrToken::Node(_) => None,
        NodeOrToken::Token(token) => match token_kind(token) {
            SyntaxKind::Whitespace => Some(Trivia::Whitespace(token.text().to_string())),
            SyntaxKind::Newline => Some(Trivia::Newline {
                kind: NewlineKind::from_newline_text(token.text()).unwrap_or_default(),
                text: token.text().to_string(),
            }),
            SyntaxKind::LineComment => Some(Trivia::Comment(comment_trivia(
                CommentKind::Line,
                token.text(),
            ))),
            SyntaxKind::HashComment => Some(Trivia::Comment(comment_trivia(
                CommentKind::Hash,
                token.text(),
            ))),
            SyntaxKind::BlockComment => Some(Trivia::Comment(comment_trivia(
                CommentKind::Block,
                token.text(),
            ))),
            SyntaxKind::Comma if include_comma => Some(Trivia::Comma(token.text().to_string())),
            _ => None,
        },
    }
}

fn comment_trivia(kind: CommentKind, text: &str) -> CommentTrivia {
    CommentTrivia {
        kind,
        text: text.to_string(),
        body: strip_comment_body(kind, text),
    }
}

fn strip_comment_body(kind: CommentKind, text: &str) -> String {
    match kind {
        CommentKind::Line => text
            .strip_prefix("//")
            .unwrap_or(text)
            .trim_start()
            .to_string(),
        CommentKind::Hash => text
            .strip_prefix('#')
            .unwrap_or(text)
            .trim_start()
            .to_string(),
        CommentKind::Block => text
            .strip_prefix("/*")
            .and_then(|s| s.strip_suffix("*/"))
            .unwrap_or(text)
            .trim()
            .to_string(),
    }
}

fn scalar_from_token(token: &RedToken) -> Result<ScalarValue, HjsonEditError> {
    let text = token.text().to_string();

    Ok(match token_kind(token) {
        SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString => {
            ScalarValue::String(StringValue::from_token(token).ok_or(
                HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Value,
                    found: token_kind(token),
                },
            )?)
        }
        SyntaxKind::Integer => ScalarValue::Integer {
            raw: text.clone(),
            parsed: text.parse::<i64>().ok(),
        },
        SyntaxKind::Float => ScalarValue::Float {
            raw: text.clone(),
            parsed: text.parse::<f64>().ok(),
        },
        SyntaxKind::True => ScalarValue::Bool(true),
        SyntaxKind::False => ScalarValue::Bool(false),
        SyntaxKind::Null => ScalarValue::Null,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: token_kind(token),
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstract_tree_exposes_lossless_styles() {
        let doc = crate::high_level::HjsonDocument::parse(
            "{\r\n  // comment\r\n  name: \"value\",\r\n}\r\n",
        )
        .unwrap();

        let ast = doc.ast().unwrap();
        let object = ast.object().expect("expected top-level object");

        assert!(object.is_braced());

        let member = object.member_by_key("name").unwrap();
        let leading = member.leading_trivia().unwrap();
        assert!(leading.iter().any(|trivia| matches!(
            trivia,
            Trivia::Comment(comment) if comment.body == "comment"
        )));
        assert!(leading.iter().any(|trivia| matches!(
            trivia,
            Trivia::Newline {
                kind: NewlineKind::CrLf,
                ..
            }
        )));

        let value = member.value().unwrap();
        match value.content().unwrap() {
            ValueContent::Scalar(ScalarValue::String(string)) => {
                assert_eq!(string.quote_style(), QuoteStyle::DoubleQuoted);
                assert_eq!(string.body_text(), "value");
            }
            other => panic!("unexpected value content: {other:?}"),
        }

        let trailing = member.trailing_trivia().unwrap();
        assert!(trailing
            .iter()
            .any(|trivia| matches!(trivia, Trivia::Comma(_))));
    }
}
