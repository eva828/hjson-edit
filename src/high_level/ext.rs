use rowan::NodeOrToken;

use crate::{
    ast::{AstNode, Key, QuoteStyle},
    high_level::{error::HjsonEditError, value::HjsonValue},
    RedNode, SyntaxKind,
};

pub(crate) trait RedNodeExt {
    fn node_to_value(&self) -> Result<HjsonValue, HjsonEditError>;
}

impl RedNodeExt for RedNode {
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
                    NodeOrToken::Token(token) => {
                        let text = token.text().to_string();
                        match token.kind() {
                            SyntaxKind::String => Ok(HjsonValue::String(
                                QuoteStyle::DoubleQuoted.decode_body(&text),
                            )),
                            SyntaxKind::BlockString => {
                                Ok(HjsonValue::String(QuoteStyle::Block.decode_body(&text)))
                            }
                            SyntaxKind::UnquotedString => {
                                Ok(HjsonValue::String(QuoteStyle::Unquoted.decode_body(&text)))
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
                        }
                    }
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
                        .and_then(|key_node| Key::cast(key_node).ok())
                        .and_then(|k| k.body_text())
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
            SyntaxKind::String => Ok(HjsonValue::String(
                QuoteStyle::DoubleQuoted.decode_body(&self.text().to_string()),
            )),
            SyntaxKind::BlockString => Ok(HjsonValue::String(
                QuoteStyle::Block.decode_body(&self.text().to_string()),
            )),
            SyntaxKind::UnquotedString => Ok(HjsonValue::String(
                QuoteStyle::Unquoted.decode_body(&self.text().to_string()),
            )),
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
