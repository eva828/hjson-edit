use rowan::NodeOrToken;
use std::ops::Range;

pub mod edit;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TriviaBoundary {
    pub leading_protected_prefix_len: usize,
    pub trailing_protected_suffix_len: usize,
}

impl TriviaBoundary {
    pub fn owned_leading_len(self, leading_len: usize) -> usize {
        leading_len.saturating_sub(self.leading_protected_prefix_len)
    }

    pub fn owned_trailing_len(self, trailing_len: usize) -> usize {
        trailing_len.saturating_sub(self.trailing_protected_suffix_len)
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
    pub body_insert_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub braced: bool,
    pub layout: Layout,
    pub empty_body_insert_index: usize,
    pub body_end_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub braced: bool,
    pub layout: Layout,
    pub empty_body_insert_index: usize,
    pub body_end_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub before_colon_trivia: Vec<Trivia>,
    pub after_colon_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
    pub trivia_boundary: TriviaBoundary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    syntax: RedNode,
    pub trivia: CommentTrivia,
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
    pub trivia_boundary: TriviaBoundary,
}

pub type Pair = Member;

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentItem {
    Object(Object),
    Array(Array),
    Value(Value),
}

impl DocumentItem {
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::Object(object) => object.detected_string_style(),
            Self::Array(array) => array.detected_string_style(),
            Self::Value(value) => value.detected_string_style(),
        }
    }
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
        let leading_trivia = collect_root_leading_trivia(&node);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: leading_trivia.clone(),
            trailing_trivia: collect_root_trailing_trivia(&node),
            layout,
            body_insert_index: leading_trivia.len(),
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
                    .children_with_tokens()
                    .nth(self.body_insert_index)?;
                match child {
                    NodeOrToken::Node(node) => match node.kind() {
                        SyntaxKind::Object => Some(DocumentItem::Object(Object::cast(node).ok()?)),
                        SyntaxKind::Array => Some(DocumentItem::Array(Array::cast(node).ok()?)),
                        SyntaxKind::Value => Some(DocumentItem::Value(Value::cast(node).ok()?)),
                        _ => None,
                    },
                    NodeOrToken::Token(_) => None,
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

    pub fn body_insert_index(&self) -> usize {
        self.body_insert_index
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

    pub fn comments(&self) -> Vec<Comment> {
        collect_comments(&self.syntax)
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.item().and_then(|item| item.detected_string_style())
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
        let is_root_child = matches!(node.parent().map(|parent| parent.kind()), Some(SyntaxKind::Root));
        let layout = collect_layout(&node, owns_trivia);
        let braced = matches!(
            node.children().next().map(|child| child.kind()),
            Some(SyntaxKind::LBrace)
        );
        let leading_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_leading_trivia(&node, SyntaxKind::Object, Some(SyntaxKind::LBrace))
        };
        let trailing_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_trailing_trivia(&node, SyntaxKind::Object, Some(SyntaxKind::RBrace))
        };
        let empty_body_insert_index = node
            .children_with_tokens()
            .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBrace))
            .unwrap_or_else(|| node.children_with_tokens().count());
        let body_end_index = empty_body_insert_index;

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            trailing_trivia,
            braced,
            layout,
            empty_body_insert_index,
            body_end_index,
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

    pub fn comments(&self) -> Vec<Comment> {
        collect_comments(&self.syntax)
    }

    pub fn empty_body_insert_index(&self) -> usize {
        self.empty_body_insert_index
    }

    pub fn append_member_index(&self) -> usize {
        self.body_end_index
    }

    pub fn body_end_index(&self) -> usize {
        self.body_end_index
    }

    pub fn member_by_key(&self, key: &str) -> Option<Member> {
        self.members()
            .into_iter()
            .find(|member| member.pair_key().as_deref() == Some(key))
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.members()
            .into_iter()
            .find_map(|member| member.detected_string_style())
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
        let is_root_child = matches!(node.parent().map(|parent| parent.kind()), Some(SyntaxKind::Root));
        let layout = collect_layout(&node, owns_trivia);
        let braced = matches!(
            node.children().next().map(|child| child.kind()),
            Some(SyntaxKind::LBracket)
        );
        let leading_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_leading_trivia(&node, SyntaxKind::Array, Some(SyntaxKind::LBracket))
        };
        let trailing_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_trailing_trivia(&node, SyntaxKind::Array, Some(SyntaxKind::RBracket))
        };
        let empty_body_insert_index = node
            .children_with_tokens()
            .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBracket))
            .unwrap_or_else(|| node.children_with_tokens().count());
        let body_end_index = empty_body_insert_index;

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            trailing_trivia,
            braced,
            layout,
            empty_body_insert_index,
            body_end_index,
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

    pub fn comments(&self) -> Vec<Comment> {
        collect_comments(&self.syntax)
    }

    pub fn empty_body_insert_index(&self) -> usize {
        self.empty_body_insert_index
    }

    pub fn append_value_index(&self) -> usize {
        self.body_end_index
    }

    pub fn body_end_index(&self) -> usize {
        self.body_end_index
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.items()
            .into_iter()
            .find_map(|item| item.detected_string_style())
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
        let (before_colon_trivia, after_colon_trivia) = collect_member_middle_trivia(&node);
        let leading_trivia = collect_leading_trivia(&node, layout.owns_trivia);
        let trailing_trivia = collect_trailing_trivia(&node, layout.owns_trivia);
        let trivia_boundary = TriviaBoundary {
            leading_protected_prefix_len: 0,
            trailing_protected_suffix_len: collect_trivia_boundary(&trailing_trivia)
                .trailing_protected_suffix_len,
        };

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            before_colon_trivia,
            after_colon_trivia,
            trailing_trivia,
            layout,
            trivia_boundary,
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

    pub fn comments(&self) -> Vec<Comment> {
        collect_comments(&self.syntax)
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.value().and_then(|value| value.detected_string_style())
    }

    pub fn removal_range(&self) -> Range<usize> {
        let owned_leading_len = self
            .trivia_boundary
            .owned_leading_len(self.leading_trivia.len());
        let owned_trailing_len = self
            .trivia_boundary
            .owned_trailing_len(self.trailing_trivia.len());
        let start = self.syntax.index().saturating_sub(owned_leading_len);
        let end = self.syntax.index() + 1 + owned_trailing_len;
        start..end
    }

    pub fn insert_before_index(&self) -> usize {
        self.removal_range().start
    }

    pub fn insert_after_index(&self) -> usize {
        self.removal_range().end
    }

    pub fn post_node_index(&self) -> usize {
        self.syntax.index() + 1
    }
}

impl AstNode for Comment {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Comment
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Comment,
                found: node.kind(),
            });
        }

        let trivia = collect_comment_trivia(&node).ok_or(HjsonEditError::ParentNotFound)?;

        Ok(Self {
            syntax: node,
            trivia,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Comment {
    pub fn kind(&self) -> CommentKind {
        self.trivia.kind
    }

    pub fn raw_text(&self) -> &str {
        &self.trivia.text
    }

    pub fn body_text(&self) -> &str {
        &self.trivia.body
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
        let parent_kind = node.parent().map(|parent| parent.kind());
        let owns_value_trivia = matches!(parent_kind, Some(SyntaxKind::Array));
        let leading_trivia = if owns_value_trivia {
            collect_leading_trivia(&node, true)
        } else {
            Vec::new()
        };
        let trailing_trivia = if owns_value_trivia {
            collect_trailing_trivia(&node, true)
        } else {
            Vec::new()
        };
        let trivia_boundary = TriviaBoundary {
            leading_protected_prefix_len: 0,
            trailing_protected_suffix_len: collect_trivia_boundary(&trailing_trivia)
                .trailing_protected_suffix_len,
        };

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            trailing_trivia,
            layout,
            trivia_boundary,
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

    pub fn quote_style(&self) -> Option<QuoteStyle> {
        self.content().ok()?.quote_style()
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        match self.content().ok()? {
            ValueContent::Scalar(ScalarValue::String(string)) => Some(string.quote_style()),
            ValueContent::Object(object) => object.detected_string_style(),
            ValueContent::Array(array) => array.detected_string_style(),
            ValueContent::Scalar(_) => None,
        }
    }

    pub fn removal_range(&self) -> Range<usize> {
        let owned_leading_len = self
            .trivia_boundary
            .owned_leading_len(self.leading_trivia.len());
        let owned_trailing_len = self
            .trivia_boundary
            .owned_trailing_len(self.trailing_trivia.len());
        let start = self.syntax.index().saturating_sub(owned_leading_len);
        let end = self.syntax.index() + 1 + owned_trailing_len;
        start..end
    }

    pub fn insert_before_index(&self) -> usize {
        self.removal_range().start
    }

    pub fn insert_after_index(&self) -> usize {
        self.removal_range().end
    }

    pub fn post_node_index(&self) -> usize {
        self.syntax.index() + 1
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

fn collect_comments(node: &RedNode) -> Vec<Comment> {
    node.children()
        .filter(|child| child.kind() == SyntaxKind::Comment)
        .filter_map(|child| Comment::cast(child).ok())
        .collect()
}

fn collect_comment_trivia(node: &RedNode) -> Option<CommentTrivia> {
    node.children_with_tokens().find_map(|elem| match elem {
        NodeOrToken::Token(token) => match token.kind() {
            SyntaxKind::LineComment => Some(CommentTrivia {
                kind: CommentKind::Line,
                text: token.text().to_string(),
                body: strip_line_comment(token.text()),
            }),
            SyntaxKind::HashComment => Some(CommentTrivia {
                kind: CommentKind::Hash,
                text: token.text().to_string(),
                body: strip_hash_comment(token.text()),
            }),
            SyntaxKind::BlockComment => Some(CommentTrivia {
                kind: CommentKind::Block,
                text: token.text().to_string(),
                body: strip_block_comment(token.text()),
            }),
            _ => None,
        },
        NodeOrToken::Node(_) => None,
    })
}

fn collect_container_leading_trivia(
    node: &RedNode,
    structural_kind: SyntaxKind,
    opening: Option<SyntaxKind>,
) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let start = opening
        .and_then(|kind| {
            children
                .iter()
                .position(|elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == kind))
        })
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let mut trivia = Vec::new();
    for elem in children.iter().skip(start) {
        match elem {
            NodeOrToken::Node(node) if node.kind() == structural_kind => break,
            _ => {
                if let Some(item) = trivia_from_element(elem) {
                    trivia.push(item);
                }
            }
        }
    }

    trivia
}

fn collect_container_trailing_trivia(
    node: &RedNode,
    structural_kind: SyntaxKind,
    closing: Option<SyntaxKind>,
) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let end = closing
        .and_then(|kind| {
            children
                .iter()
                .rposition(|elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == kind))
        })
        .unwrap_or(children.len());

    let start = children
        .iter()
        .rposition(|elem| matches!(elem, NodeOrToken::Node(node) if node.kind() == structural_kind))
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let mut trivia = Vec::new();
    for elem in children.iter().skip(start).take(end.saturating_sub(start)) {
        if let Some(item) = trivia_from_element(elem) {
            trivia.push(item);
        }
    }

    trivia
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
    let mut saw_boundary_newline = false;
    let mut prev_has_trailing = false;

    for elem in children.iter().take(index) {
        let Some(item) = trivia_from_element(elem) else {
            trivia.clear();
            saw_boundary_newline = false;
            prev_has_trailing = matches!(elem.kind(), SyntaxKind::Pair | SyntaxKind::Value);
            continue;
        };

        match item {
            Trivia::Comma(_) => {
                trivia.clear();
                saw_boundary_newline = false;
                prev_has_trailing = false;
            }
            Trivia::Newline { .. } if prev_has_trailing && !saw_boundary_newline => {
                trivia.clear();
                saw_boundary_newline = true;
                prev_has_trailing = false;
            }
            Trivia::Newline { .. } if !saw_boundary_newline => {
                trivia.clear();
                saw_boundary_newline = true;
                prev_has_trailing = false;
            }
            other => {
                trivia.push(other);
                prev_has_trailing = false;
            }
        }
    }

    trivia
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

fn collect_member_middle_trivia(node: &RedNode) -> (Vec<Trivia>, Vec<Trivia>) {
    let mut before_colon = Vec::new();
    let mut after_colon = Vec::new();
    let mut seen_colon = false;

    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Key => continue,
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value => break,
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Colon => {
                seen_colon = true;
                continue;
            }
            other => {
                if let Some(item) = trivia_from_element(&other) {
                    if seen_colon {
                        after_colon.push(item);
                    } else {
                        before_colon.push(item);
                    }
                }
            }
        }
    }

    (before_colon, after_colon)
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
            Trivia::Whitespace(text) => trivia.push(Trivia::Whitespace(text)),
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

fn collect_trivia_boundary(_trivia: &[Trivia]) -> TriviaBoundary {
    TriviaBoundary {
        leading_protected_prefix_len: 0,
        trailing_protected_suffix_len: 1,
    }
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
        NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => {
            collect_comment_trivia(node).map(Trivia::Comment)
        }
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
