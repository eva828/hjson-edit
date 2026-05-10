use rowan::NodeOrToken;

use crate::{
    ast::{AstNode, Key, QuoteStyle},
    high_level::{error::HjsonEditError, value::HjsonValue},
    RedNode, RedToken, SyntaxKind,
};

fn token_to_value(token: &RedToken) -> Result<HjsonValue, HjsonEditError> {
    let text = token.text();
    match token.kind() {
        SyntaxKind::String => Ok(HjsonValue::String(
            QuoteStyle::DoubleQuoted.decode_body(text),
        )),
        SyntaxKind::BlockString => {
            Ok(HjsonValue::String(QuoteStyle::Block.decode_body(text)))
        }
        SyntaxKind::UnquotedString => {
            Ok(HjsonValue::String(QuoteStyle::Unquoted.decode_body(text)))
        }
        SyntaxKind::Integer => text
            .parse::<i64>()
            .map(HjsonValue::Integer)
            .map_err(|_| HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Integer,
                found: token.kind(),
            }),
        SyntaxKind::Float => text
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
                    NodeOrToken::Token(token) => token_to_value(&token),
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
            SyntaxKind::String
            | SyntaxKind::BlockString
            | SyntaxKind::UnquotedString
            | SyntaxKind::Integer
            | SyntaxKind::Float
            | SyntaxKind::True
            | SyntaxKind::False
            | SyntaxKind::Null => self
                .children_with_tokens()
                .find_map(|child| match child {
                    NodeOrToken::Token(token) if !token.kind().is_trivia() => Some(token),
                    _ => None,
                })
                .ok_or(HjsonEditError::ParentNotFound)
                .and_then(|token| token_to_value(&token)),
            _ => Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: self.kind(),
            }),
        }
    }
}
