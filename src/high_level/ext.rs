use rowan::{GreenNode, NodeOrToken};

use crate::{
    format::{FormatConfig, Formatter},
    high_level::{
        error::HjsonEditError,
        ops::{comment::CommentStyle, object::StringStyle},
        path::HjsonObjectPath,
        style::DetectNewline,
        value::HjsonValue,
        HjsonDocument,
    },
    ArrayPathSegment, ObjectPathSegment, PathSegment, RedNode, SyntaxKind,
};

pub trait RedNodeExt {
    fn children_by_kind(&self, kind: SyntaxKind) -> impl Iterator<Item = RedNode>;
    fn child(&self, path: &PathSegment) -> Option<RedNode>;
    fn path(&self) -> HjsonObjectPath;
    fn document_start_node(&self) -> RedNode;
    fn container_depth(&self) -> usize;
    fn extract_key(&self) -> Option<String>;
    fn pair_key(&self) -> Option<String>;
    fn node_to_value(&self) -> Result<HjsonValue, HjsonEditError>;
    fn to_hjson_value(&self) -> Result<HjsonValue, HjsonEditError> {
        self.node_to_value()
    }
    fn format_like(&self, root: GreenNode) -> GreenNode;
}

impl RedNodeExt for RedNode {
    fn children_by_kind(&self, kind: SyntaxKind) -> impl Iterator<Item = RedNode> {
        self.children().filter(move |child| child.kind() == kind)
    }

    fn child(&self, segment: &PathSegment) -> Option<RedNode> {
        if self.kind() == SyntaxKind::Value {
            let next = self
                .children()
                .next()
                .and_then(|child| child.child(segment));
            return next;
        }

        match segment {
            PathSegment::Object(segment) if self.kind() == SyntaxKind::Object => self
                .children_by_kind(SyntaxKind::Pair)
                .find(|pair| {
                    pair.children_by_kind(SyntaxKind::Key)
                        .find_map(|key| key.extract_key())
                        .is_some_and(|key| segment.match_key(&key))
                })
                .and_then(|pair| pair.children_by_kind(SyntaxKind::Value).next()),
            PathSegment::Array(segment) if self.kind() == SyntaxKind::Array => {
                let mut values = self.children_by_kind(SyntaxKind::Value);
                match segment {
                    ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => {
                        values.nth(*index)
                    }
                    ArrayPathSegment::Push => values.last(),
                }
            }
            _ => {
                let next = self.children().find_map(|child| child.child(segment));
                next
            }
        }
    }

    fn path(&self) -> HjsonObjectPath {
        let mut segments = Vec::new();
        let mut current = self.clone();

        while let Some(parent) = current.parent() {
            match parent.kind() {
                SyntaxKind::Pair => {
                    segments.push(PathSegment::Object(ObjectPathSegment::Key(
                        parent.pair_key().unwrap_or_default(),
                    )));
                }
                SyntaxKind::Array => {
                    let index = parent
                        .children_by_kind(SyntaxKind::Value)
                        .position(|c| c == current)
                        .unwrap_or(0);
                    segments.push(PathSegment::Array(ArrayPathSegment::Index(index)));
                }
                _ => {}
            }
            current = parent;
        }

        segments.reverse();
        HjsonObjectPath::new(segments)
    }

    fn document_start_node(&self) -> RedNode {
        if self.kind() == SyntaxKind::Root {
            self.children().next().unwrap_or_else(|| self.clone())
        } else {
            self.clone()
        }
    }

    fn container_depth(&self) -> usize {
        let mut depth = 0;
        let mut current = self.parent();

        while let Some(parent) = current {
            if matches!(parent.kind(), SyntaxKind::Object | SyntaxKind::Array) {
                depth += 1;
            }
            current = parent.parent();
        }

        depth
    }

    fn extract_key(&self) -> Option<String> {
        self.children_with_tokens().find_map(|child| match child {
            NodeOrToken::Token(token)
                if matches!(
                    token.kind(),
                    SyntaxKind::String | SyntaxKind::UnquotedString
                ) =>
            {
                Some(token.text().trim_matches('"').to_string())
            }
            _ => None,
        })
    }

    fn pair_key(&self) -> Option<String> {
        self.children_by_kind(SyntaxKind::Key)
            .find_map(|key| key.extract_key())
    }

    fn node_to_value(&self) -> Result<HjsonValue, HjsonEditError> {
        match self.kind() {
            SyntaxKind::Root => self
                .children()
                .find(|child| {
                    matches!(
                        child.kind(),
                        SyntaxKind::Value | SyntaxKind::Object | SyntaxKind::Array
                    )
                })
                .ok_or(HjsonEditError::ParentNotFound)?
                .node_to_value(),
            SyntaxKind::Value => self
                .children_with_tokens()
                .find(|child| match child {
                    NodeOrToken::Node(node) => !node.kind().is_trivia(),
                    NodeOrToken::Token(token) => !token.kind().is_trivia(),
                })
                .ok_or(HjsonEditError::ParentNotFound)
                .and_then(|child| match child {
                    NodeOrToken::Node(node) => node.node_to_value(),
                    NodeOrToken::Token(token) => match token.kind() {
                        SyntaxKind::String => Ok(HjsonValue::String(decode_double_quoted(
                            token.text().to_string(),
                        ))),
                        SyntaxKind::BlockString => Ok(HjsonValue::String(strip_block_string(
                            token.text().to_string(),
                        ))),
                        SyntaxKind::UnquotedString => {
                            Ok(HjsonValue::String(token.text().to_string()))
                        }
                        SyntaxKind::Integer => token
                            .text()
                            .parse::<i64>()
                            .map(HjsonValue::Integer)
                            .map_err(|_| HjsonEditError::UnexpectedStructure {
                                expected: SyntaxKind::Integer,
                                found: token.kind(),
                            }),
                        SyntaxKind::Float => token
                            .text()
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
                    },
                }),
            SyntaxKind::Object => {
                let mut entries = Vec::new();
                for pair in self
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Pair)
                {
                    let key = pair
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Key)
                        .and_then(|key| key.extract_key())
                        .ok_or(HjsonEditError::ParentNotFound)?;
                    let value = pair
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Value)
                        .ok_or(HjsonEditError::ParentNotFound)?;
                    entries.push((key, value.node_to_value()?));
                }
                Ok(HjsonValue::Object(entries))
            }
            SyntaxKind::Array => {
                let mut items = Vec::new();
                for value in self
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Value)
                {
                    items.push(value.node_to_value()?);
                }
                Ok(HjsonValue::Array(items))
            }
            SyntaxKind::String => Ok(HjsonValue::String(decode_double_quoted(
                self.text().to_string(),
            ))),
            SyntaxKind::BlockString => Ok(HjsonValue::String(strip_block_string(
                self.text().to_string(),
            ))),
            SyntaxKind::UnquotedString => Ok(HjsonValue::String(self.text().to_string())),
            SyntaxKind::Integer => self
                .text()
                .to_string()
                .parse::<i64>()
                .map(HjsonValue::Integer)
                .map_err(|_| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Integer,
                    found: self.kind(),
                }),
            SyntaxKind::Float => self
                .text()
                .to_string()
                .parse::<f64>()
                .map(HjsonValue::Float)
                .map_err(|_| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Float,
                    found: self.kind(),
                }),
            SyntaxKind::True => Ok(HjsonValue::Bool(true)),
            SyntaxKind::False => Ok(HjsonValue::Bool(false)),
            SyntaxKind::Null => Ok(HjsonValue::Null),
            _ => Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: self.kind(),
            }),
        }
    }

    fn format_like(&self, root: GreenNode) -> GreenNode {
        let document = HjsonDocument {
            root: RedNode::new_root(root.clone()),
            string_style: StringStyle::default(),
            comment_style: CommentStyle::default(),
            newline: self.detect_newline().as_str().to_string(),
        };

        let ast = match document.ast() {
            Ok(ast) => ast,
            Err(_) => return root,
        };

        Formatter::build(
            &ast,
            FormatConfig::default().with_newline(self.detect_newline().as_str()),
        )
    }
}

fn strip_block_string(text: String) -> String {
    text.strip_prefix("'''")
        .and_then(|s| s.strip_suffix("'''"))
        .unwrap_or(&text)
        .to_string()
}

fn decode_double_quoted(text: String) -> String {
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(&text);
    let mut out = String::new();
    let mut chars = inner.chars();

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
            Some('u') => {
                let mut hex = String::new();
                for _ in 0..4 {
                    if let Some(ch) = chars.next() {
                        hex.push(ch);
                    }
                }
                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                }
            }
            Some(other) => out.push(other),
            None => break,
        }
    }

    out
}
