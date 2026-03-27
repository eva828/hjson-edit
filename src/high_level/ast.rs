use rowan::NodeOrToken;

use crate::{HjsonEditError, RedNode, RedToken, SyntaxKind};

pub trait AstNode: Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(node: RedNode) -> Result<Self, HjsonEditError>;
    fn node(&self) -> &RedNode;
}

pub struct Pair {
    node: RedNode,
}

impl AstNode for Pair {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Pair
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if Self::can_cast(node.kind()) {
            Ok(Self { node })
        } else {
            Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Pair,
                found: node.kind(),
            })
        }
    }

    fn node(&self) -> &RedNode {
        &self.node
    }
}

impl Pair {
    pub fn leading_trivia(&self) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
        let parent = self
            .node()
            .parent()
            .ok_or_else(|| HjsonEditError::ParentNotFound)?;

        let index = self.node().index();
        let mut trivia = Vec::new();

        let mut prev_is_newline = false;
        for i in (0..index).rev() {
            let sibling = parent
                .children_with_tokens()
                .nth(i)
                .ok_or_else(|| HjsonEditError::ParentNotFound)?;
            if !sibling.kind().is_trivia() {
                break;
            }
            if sibling.kind() == SyntaxKind::Newline {
                if prev_is_newline {
                    break;
                }
                prev_is_newline = true;
            }
            trivia.push(sibling);
        }

        Ok(trivia)
    }

    pub fn trailing_trivia(&self) -> Result<Vec<NodeOrToken<RedNode, RedToken>>, HjsonEditError> {
        let parent = self
            .node()
            .parent()
            .ok_or_else(|| HjsonEditError::ParentNotFound)?;

        let index = self.node().index();
        let mut trivia = Vec::new();

        for i in index + 1..parent.children_with_tokens().count() {
            let sibling = parent
                .children_with_tokens()
                .nth(i)
                .ok_or_else(|| HjsonEditError::ParentNotFound)?;
            println!("Sibling kind: {:?}", sibling.kind());
            if !sibling.kind().is_trivia() && sibling.kind() != SyntaxKind::Comma {
                break;
            }
            if sibling.kind() == SyntaxKind::Newline {
                trivia.push(sibling);
                break;
            }
            trivia.push(sibling);
        }

        Ok(trivia)
    }
}
