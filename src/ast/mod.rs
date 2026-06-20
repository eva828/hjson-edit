use rowan::NodeOrToken;
use std::ops::Range;

use crate::syntax::{GreenElement, NewlineKind};
use crate::{HjsonEditError, RedNode, RedToken, SyntaxKind};

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

impl CommentKind {
    pub fn decode_body(&self, raw: &str) -> String {
        match self {
            CommentKind::Line => raw
                .strip_prefix("//")
                .unwrap_or(raw)
                .trim_start()
                .to_string(),
            CommentKind::Hash => raw
                .strip_prefix('#')
                .unwrap_or(raw)
                .trim_start()
                .to_string(),
            CommentKind::Block => raw
                .strip_prefix("/*")
                .and_then(|s| s.strip_suffix("*/"))
                .unwrap_or(raw)
                .trim()
                .to_string(),
        }
    }
}

impl QuoteStyle {
    pub fn decode_body(&self, raw: &str) -> String {
        match self {
            QuoteStyle::DoubleQuoted => {
                let inner = raw
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(raw);
                let mut out = String::new();
                let mut chars = inner.chars().peekable();

                while let Some(c) = chars.next() {
                    if c != '\\' {
                        out.push(c);
                        continue;
                    }

                    match chars.next() {
                        Some('"') => out.push('"'),
                        Some('\\') => out.push('\\'),
                        Some('/') => out.push('/'),
                        Some('b') => out.push('\u{0008}'),
                        Some('f') => out.push('\u{000C}'),
                        Some('n') => out.push('\n'),
                        Some('r') => out.push('\r'),
                        Some('t') => out.push('\t'),
                        Some(other) => {
                            out.push('\\');
                            out.push(other);
                        }
                        None => break,
                    }
                }

                out
            }
            QuoteStyle::Block => {
                let inner = raw
                    .strip_prefix("'''")
                    .and_then(|s| s.strip_suffix("'''"))
                    .unwrap_or(raw)
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");

                let mut lines = inner.split('\n').collect::<Vec<_>>();
                while lines.first().is_some_and(|line| line.trim().is_empty()) {
                    lines.remove(0);
                }
                while lines.last().is_some_and(|line| line.trim().is_empty()) {
                    lines.pop();
                }

                if lines.is_empty() {
                    return String::new();
                }

                let indent = lines
                    .iter()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.chars().take_while(|c| matches!(c, ' ' | '\t')).count())
                    .min()
                    .unwrap_or(0);

                lines
                    .into_iter()
                    .map(|line| {
                        if line.trim().is_empty() {
                            String::new()
                        } else {
                            line.chars().skip(indent).collect::<String>()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            QuoteStyle::Unquoted => raw.to_string(),
        }
    }

    pub fn encode_body(&self, body: &str) -> String {
        match self {
            QuoteStyle::DoubleQuoted => {
                let mut out = String::with_capacity(body.len() + 2);
                for c in body.chars() {
                    match c {
                        '\\' => out.push_str("\\\\"),
                        '"' => out.push_str("\\\""),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        '\u{0008}' => out.push_str("\\b"),
                        '\u{000C}' => out.push_str("\\f"),
                        c if c.is_control() => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out
            }
            QuoteStyle::Block | QuoteStyle::Unquoted => body.to_string(),
        }
    }

    pub fn is_valid_for_text(&self, text: &str) -> bool {
        match self {
            QuoteStyle::Unquoted => {
                !text.is_empty()
                    && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    && !matches!(text, "true" | "false" | "null")
                    && !crate::number::HjsonNumber::is_valid(text)
            }
            QuoteStyle::DoubleQuoted | QuoteStyle::Block => true,
        }
    }

    pub fn render(&self, body: &str) -> String {
        let encoded = self.encode_body(body);
        match self {
            QuoteStyle::DoubleQuoted => format!("\"{}\"", encoded),
            QuoteStyle::Block => format!("'''{}'''", encoded),
            QuoteStyle::Unquoted => encoded,
        }
    }

    pub fn syntax_kind(&self) -> SyntaxKind {
        match self {
            QuoteStyle::DoubleQuoted => SyntaxKind::String,
            QuoteStyle::Block => SyntaxKind::BlockString,
            QuoteStyle::Unquoted => SyntaxKind::UnquotedString,
        }
    }
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

    pub fn to_green_element(&self) -> GreenElement {
        match self {
            Trivia::Whitespace(text) => {
                SyntaxKind::Whitespace.create_token(text.as_str()).into()
            }
            Trivia::Newline { text, .. } => {
                SyntaxKind::Newline.create_token(text.as_str()).into()
            }
            Trivia::Comment(comment) => {
                let kind = match comment.kind {
                    CommentKind::Line => SyntaxKind::LineComment,
                    CommentKind::Hash => SyntaxKind::HashComment,
                    CommentKind::Block => SyntaxKind::BlockComment,
                };
                let token: rowan::GreenToken = kind.create_token(comment.text.as_str());
                let node = rowan::GreenNode::new(
                    SyntaxKind::Comment.into(),
                    vec![token.into()],
                );
                NodeOrToken::Node(node)
            }
            Trivia::Comma(text) => {
                SyntaxKind::Comma.create_token(text.as_str()).into()
            }
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
    pub body_insert_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub delimiter_style: DelimiterStyle,
    pub layout: Layout,
    empty_body_insert_index: usize,
    body_end_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterStyle {
    Braced,
    Braceless,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub delimiter_style: DelimiterStyle,
    pub layout: Layout,
    empty_body_insert_index: usize,
    body_end_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub before_colon_trivia: Vec<Trivia>,
    pub after_colon_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
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
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayItem {
    syntax: RedNode,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
    pub layout: Layout,
}

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
        kind == SyntaxKind::Root
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
        let is_root_child = matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);
        let delimiter_style = detect_delimiter_style(&node, SyntaxKind::LBrace);
        let leading_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_leading_trivia(
                &node,
                SyntaxKind::Member,
                SyntaxKind::LBrace,
                SyntaxKind::RBrace,
            )
        };
        let trailing_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_trailing_trivia(
                &node,
                SyntaxKind::Member,
                SyntaxKind::LBrace,
                SyntaxKind::RBrace,
            )
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
            delimiter_style,
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
            .filter(|child| child.kind() == SyntaxKind::Member)
            .filter_map(|child| Member::cast(child).ok())
            .collect()
    }

    pub fn append_member_index_before_footer(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
            return children
                .iter()
                .rposition(|child| matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Member))
                .map(|idx| idx + 1)
                .unwrap_or(self.empty_body_insert_index);
        }

        self.body_end_index
    }

    pub fn append_member_index_after_comments(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            return self.syntax.children_with_tokens().count();
        }

        self.body_end_index
    }

    pub fn member_by_key(&self, key: &str) -> Option<Member> {
        self.members()
            .into_iter()
            .find(|member| member.member_key().as_deref() == Some(key))
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
        let is_root_child = matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);
        let delimiter_style = detect_delimiter_style(&node, SyntaxKind::LBracket);
        let leading_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_leading_trivia(
                &node,
                SyntaxKind::Value,
                SyntaxKind::LBracket,
                SyntaxKind::RBracket,
            )
        };
        let trailing_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_trailing_trivia(
                &node,
                SyntaxKind::Value,
                SyntaxKind::LBracket,
                SyntaxKind::RBracket,
            )
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
            delimiter_style,
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
    pub fn items(&self) -> Vec<ArrayItem> {
        self.syntax
            .children()
            .filter(|child| child.kind() == SyntaxKind::Value)
            .filter_map(|child| ArrayItem::cast(child).ok())
            .collect()
    }

    pub fn values(&self) -> Vec<Value> {
        self.items()
            .into_iter()
            .filter_map(|item| item.value().ok())
            .collect()
    }

    pub fn append_value_index_before_footer(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
            return children
                .iter()
                .rposition(|child| matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value))
                .map(|idx| idx + 1)
                .unwrap_or(self.empty_body_insert_index);
        }

        self.body_end_index
    }

    pub fn append_value_index_after_comments(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            return self.syntax.children_with_tokens().count();
        }

        self.body_end_index
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.values()
            .into_iter()
            .find_map(|item| item.detected_string_style())
    }
}

impl AstNode for Member {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Member
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Member,
                found: node.kind(),
            });
        }

        let layout = collect_layout(&node, true);
        let (before_colon_trivia, after_colon_trivia) = collect_member_middle_trivia(&node);
        let leading_trivia = collect_leading_trivia(&node, layout.owns_trivia);
        let trailing_trivia = collect_trailing_trivia(&node, layout.owns_trivia);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            before_colon_trivia,
            after_colon_trivia,
            trailing_trivia,
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

    pub fn member_key(&self) -> Option<String> {
        self.key().and_then(|key| key.body_text())
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.value().and_then(|value| value.detected_string_style())
    }

    pub fn span_range(&self) -> Range<usize> {
        let start = self
            .syntax
            .index()
            .saturating_sub(self.leading_trivia.len());
        let end = self.syntax.index() + 1 + self.trailing_trivia.len();
        start..end
    }

    pub fn insert_before_index(&self) -> usize {
        self.span_range().start
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
                    let style = match token.kind() {
                        SyntaxKind::String => QuoteStyle::DoubleQuoted,
                        SyntaxKind::BlockString => QuoteStyle::Block,
                        SyntaxKind::UnquotedString => QuoteStyle::Unquoted,
                        _ => return None,
                    };
                    Some(style.decode_body(&text))
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
            NodeOrToken::Token(token) => {
                let raw = token.text().to_string();
                match token.kind() {
                    SyntaxKind::String => {
                        let style = QuoteStyle::DoubleQuoted;
                        ValueContent::Scalar(ScalarValue::String(StringValue {
                            raw: raw.clone(),
                            body: style.decode_body(&raw),
                            quote_style: style,
                        }))
                    }
                    SyntaxKind::BlockString => {
                        let style = QuoteStyle::Block;
                        ValueContent::Scalar(ScalarValue::String(StringValue {
                            raw: raw.clone(),
                            body: style.decode_body(&raw),
                            quote_style: style,
                        }))
                    }
                    SyntaxKind::UnquotedString => {
                        let style = QuoteStyle::Unquoted;
                        ValueContent::Scalar(ScalarValue::String(StringValue {
                            raw: raw.clone(),
                            body: style.decode_body(&raw),
                            quote_style: style,
                        }))
                    }
                    SyntaxKind::Integer => ValueContent::Scalar(ScalarValue::Integer {
                        raw: raw.clone(),
                        parsed: raw.parse::<i64>().ok(),
                    }),
                    SyntaxKind::Float => ValueContent::Scalar(ScalarValue::Float {
                        raw: raw.clone(),
                        parsed: raw.parse::<f64>().ok(),
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
                }
            }
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
}

impl AstNode for ArrayItem {
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

        let parent_kind = node.parent().map(|parent| parent.kind());
        if !matches!(parent_kind, Some(SyntaxKind::Array)) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: parent_kind.unwrap_or(SyntaxKind::Root),
            });
        }

        let layout = collect_layout(&node, true);
        let leading_trivia = collect_leading_trivia(&node, true);
        let trailing_trivia = collect_trailing_trivia(&node, true);

        Ok(Self {
            syntax: node,
            leading_trivia,
            trailing_trivia,
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl ArrayItem {
    pub fn value(&self) -> Result<Value, HjsonEditError> {
        Value::cast(self.syntax.clone())
    }

    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.value().ok()?.detected_string_style()
    }

    pub fn span_range(&self) -> Range<usize> {
        let start = self
            .syntax
            .index()
            .saturating_sub(self.leading_trivia.len());
        let end = self.syntax.index() + 1 + self.trailing_trivia.len();
        start..end
    }

    pub fn insert_before_index(&self) -> usize {
        self.span_range().start
    }

}

fn collect_comment_trivia(node: &RedNode) -> Option<CommentTrivia> {
    node.children_with_tokens().find_map(|elem| match elem {
        NodeOrToken::Token(token) => {
            let text = token.text().to_string();
            let (kind, body) = match token.kind() {
                SyntaxKind::LineComment => {
                    (CommentKind::Line, CommentKind::Line.decode_body(&text))
                }
                SyntaxKind::HashComment => {
                    (CommentKind::Hash, CommentKind::Hash.decode_body(&text))
                }
                SyntaxKind::BlockComment => {
                    (CommentKind::Block, CommentKind::Block.decode_body(&text))
                }
                _ => return None,
            };
            Some(CommentTrivia { kind, text, body })
        }
        NodeOrToken::Node(_) => None,
    })
}

fn collect_container_leading_trivia(
    node: &RedNode,
    item_kind: SyntaxKind,
    opening_token_kind: SyntaxKind,
    closing_token_kind: SyntaxKind,
) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let open_index = children.iter().position(
        |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == opening_token_kind),
    );

    let start = open_index.map_or(0, |idx| idx + 1);
    let end = children
        .iter()
        .rposition(
            |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == closing_token_kind),
        )
        .unwrap_or(children.len());

    let first_item_index = children[start..end]
        .iter()
        .position(|elem| matches!(elem, NodeOrToken::Node(node) if node.kind() == item_kind))
        .map(|relative| start + relative);

    let leading_end = first_item_index.unwrap_or(end);
    collect_trivia_slice(&children[start..leading_end])
}

fn trailing_trivia_end(
    children: &[NodeOrToken<RedNode, RedToken>],
    start: usize,
) -> usize {
    let mut cursor = start;
    while cursor < children.len() {
        let Some(item) = trivia_from_element(&children[cursor]) else {
            break;
        };
        cursor += 1;
        if matches!(item, Trivia::Newline { .. }) {
            break;
        }
    }
    cursor
}

fn collect_container_trailing_trivia(
    node: &RedNode,
    item_kind: SyntaxKind,
    opening_token_kind: SyntaxKind,
    closing_token_kind: SyntaxKind,
) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let start = children
        .iter()
        .position(
            |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == opening_token_kind),
        )
        .map_or(0, |idx| idx + 1);
    let end = children
        .iter()
        .rposition(
            |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == closing_token_kind),
        )
        .unwrap_or(children.len());

    let last_item_index = children[start..end]
        .iter()
        .rposition(|elem| matches!(elem, NodeOrToken::Node(node) if node.kind() == item_kind))
        .map(|relative| start + relative);

    let trailing_start = last_item_index
        .map_or(start, |idx| trailing_trivia_end(&children, idx + 1));
    collect_trivia_slice(&children[trailing_start..end])
}

fn collect_trivia_slice(slice: &[NodeOrToken<RedNode, RedToken>]) -> Vec<Trivia> {
    let mut trivia = Vec::new();
    for elem in slice {
        if let Some(item) = trivia_from_element(elem) {
            trivia.push(item);
        }
    }
    trivia
}

fn detect_delimiter_style(node: &RedNode, opening_token_kind: SyntaxKind) -> DelimiterStyle {
    let is_braced = node
        .children_with_tokens()
        .find_map(|elem| match elem {
            NodeOrToken::Token(token) if token.kind().is_trivia() => None,
            NodeOrToken::Token(token) => Some(token.kind() == opening_token_kind),
            NodeOrToken::Node(_) => Some(false),
        })
        .unwrap_or(false);

    if is_braced {
        DelimiterStyle::Braced
    } else {
        DelimiterStyle::Braceless
    }
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
            prev_has_trailing = matches!(elem.kind(), SyntaxKind::Member | SyntaxKind::Value);
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

    let start = node.index() + 1;
    let end = trailing_trivia_end(&children, start);
    children[start..end]
        .iter()
        .filter_map(trivia_from_element)
        .collect()
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
    let mut depth = 1usize;
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
            SyntaxKind::LineComment => {
                let text = token.text().to_string();
                Some(Trivia::Comment(CommentTrivia {
                    kind: CommentKind::Line,
                    text: text.clone(),
                    body: CommentKind::Line.decode_body(&text),
                }))
            }
            SyntaxKind::HashComment => {
                let text = token.text().to_string();
                Some(Trivia::Comment(CommentTrivia {
                    kind: CommentKind::Hash,
                    text: text.clone(),
                    body: CommentKind::Hash.decode_body(&text),
                }))
            }
            SyntaxKind::BlockComment => {
                let text = token.text().to_string();
                Some(Trivia::Comment(CommentTrivia {
                    kind: CommentKind::Block,
                    text: text.clone(),
                    body: CommentKind::Block.decode_body(&text),
                }))
            }
            SyntaxKind::Comma => Some(Trivia::Comma(token.text().to_string())),
            _ => None,
        },
        NodeOrToken::Node(_) => None,
    }
}

pub fn detected_string_style_of_node(node: &RedNode) -> Option<QuoteStyle> {
    match node.kind() {
        SyntaxKind::Root => Document::cast(node.clone())
            .ok()
            .and_then(|doc| doc.detected_string_style()),
        SyntaxKind::Object => Object::cast(node.clone())
            .ok()
            .and_then(|object| object.detected_string_style()),
        SyntaxKind::Array => Array::cast(node.clone())
            .ok()
            .and_then(|array| array.detected_string_style()),
        SyntaxKind::Value => Value::cast(node.clone())
            .ok()
            .and_then(|value| value.quote_style()),
        SyntaxKind::Member => Member::cast(node.clone())
            .ok()
            .and_then(|member| member.detected_string_style()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{lexer::Lexer, RedNode, SyntaxKind};
    use crate::syntax::parser::Parser;

    fn parse_and_find_node(src: &str, kind: SyntaxKind) -> RedNode {
        let tokens = Lexer::new(src).lex();
        let parsed = Parser::new(tokens.tokens).parse();
        let root = RedNode::new_root(parsed.green);
        root.descendants()
            .find(|node| node.kind() == kind)
            .unwrap()
    }

    #[test]
    fn test_cast_value_node_to_mismatched_kinds_rejected() {
        let value_node = parse_and_find_node("123", SyntaxKind::Value);

        assert!(Object::cast(value_node.clone()).is_err());
        assert!(Array::cast(value_node.clone()).is_err());
        assert!(Document::cast(value_node.clone()).is_err());
        assert!(Member::cast(value_node.clone()).is_err());
        assert!(Key::cast(value_node.clone()).is_err());
    }

    #[test]
    fn test_cast_object_node_to_mismatched_kinds_rejected() {
        let object_node = parse_and_find_node("{}", SyntaxKind::Object);

        assert!(Value::cast(object_node.clone()).is_err());
        assert!(Array::cast(object_node.clone()).is_err());
    }

    #[test]
    fn test_cast_comment_node_to_value_rejected() {
        let comment_node = parse_and_find_node("// c", SyntaxKind::Comment);

        assert!(Value::cast(comment_node.clone()).is_err());
    }
}
