use rowan::NodeOrToken;

use crate::{
    high_level::{error::HjsonEditError, value::HjsonValue},
    ArrayPathSegment, PathSegment, RedNode, SyntaxKind,
};

pub(crate) trait RedNodeExt {
    fn children_by_kind(&self, kind: SyntaxKind) -> impl Iterator<Item = RedNode>;
    fn child(&self, path: &PathSegment) -> Option<RedNode>;
    fn document_start_node(&self) -> RedNode;
    fn extract_key(&self) -> Option<String>;
    fn node_to_value(&self) -> Result<HjsonValue, HjsonEditError>;
}

impl RedNodeExt for RedNode {
    fn children_by_kind(&self, kind: SyntaxKind) -> impl Iterator<Item = RedNode> {
        self.children().filter(move |child| child.kind() == kind)
    }

    fn child(&self, segment: &PathSegment) -> Option<RedNode> {
        match self.kind() {
            SyntaxKind::Root | SyntaxKind::Value => self
                .children()
                .find(|child| !child.kind().is_trivia())
                .and_then(|child| child.child(segment)),
            SyntaxKind::Object => match segment {
                PathSegment::Key(key) => self
                    .children_by_kind(SyntaxKind::Member)
                    .find(|member| {
                        member
                            .children_by_kind(SyntaxKind::Key)
                            .find_map(|key_node| key_node.extract_key())
                            .is_some_and(|member_key| &member_key == key)
                    })
                    .and_then(|member| member.children_by_kind(SyntaxKind::Value).next()),
                _ => None,
            },
            SyntaxKind::Array => match segment {
                PathSegment::Array(seg) => {
                    let mut values = self.children_by_kind(SyntaxKind::Value);
                    match seg {
                        ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => {
                            values.nth(*index)
                        }
                        ArrayPathSegment::Push => values.last(),
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn document_start_node(&self) -> RedNode {
        if self.kind() == SyntaxKind::Root {
            self.children()
                .find(|child| !child.kind().is_trivia())
                .unwrap_or_else(|| self.clone())
        } else {
            self.clone()
        }
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
                        SyntaxKind::String => Ok(HjsonValue::String(
                            decode_double_quoted_lossless(token.text().to_string()),
                        )),
                        SyntaxKind::BlockString => Ok(HjsonValue::String(
                            strip_block_string_lossless(token.text().to_string()),
                        )),
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
                for member in self
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Member)
                {
                    let key = member
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Key)
                        .and_then(|key| key.extract_key())
                        .ok_or(HjsonEditError::ParentNotFound)?;
                    let value = member
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
            SyntaxKind::String => Ok(HjsonValue::String(decode_double_quoted_lossless(
                self.text().to_string(),
            ))),
            SyntaxKind::BlockString => Ok(HjsonValue::String(strip_block_string_lossless(
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
}

fn decode_double_quoted_lossless(text: String) -> String {
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(&text);
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
            Some('u') => {
                let mut hex = String::new();
                for _ in 0..4 {
                    if let Some(ch) = chars.next() {
                        hex.push(ch);
                    }
                }

                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                    if (0xD800..=0xDBFF).contains(&code) {
                        let mut lookahead = chars.clone();
                        if lookahead.next() == Some('\\') && lookahead.next() == Some('u') {
                            let mut low_hex = String::new();
                            for _ in 0..4 {
                                if let Some(ch) = lookahead.next() {
                                    low_hex.push(ch);
                                }
                            }
                            if let Ok(low) = u32::from_str_radix(&low_hex, 16) {
                                if (0xDC00..=0xDFFF).contains(&low) {
                                    let combined =
                                        0x10000 + (((code - 0xD800) << 10) | (low - 0xDC00));
                                    if let Some(ch) = char::from_u32(combined) {
                                        out.push(ch);
                                        chars = lookahead;
                                        continue;
                                    }
                                }
                            }
                        }
                    }

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

fn strip_block_string_lossless(text: String) -> String {
    let inner = text
        .strip_prefix("'''")
        .and_then(|s| s.strip_suffix("'''"))
        .unwrap_or(&text)
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
