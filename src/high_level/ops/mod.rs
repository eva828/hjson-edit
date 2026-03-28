use rowan::{GreenNode, GreenToken, Language, NodeOrToken};

use crate::{
    high_level::{
        ast::{AstNode, Pair},
        walker::PathWalker,
    },
    syntax::HjsonLanguage,
    HjsonDocument, HjsonEditError, HjsonPath, RedNode, SyntaxKind,
};

pub mod comment;
pub mod object;

impl HjsonDocument {
    // fn has_path(&self, path: &HjsonPath) -> bool {
    //     self.get_internal(path).is_ok()
    // }

    fn get_internal(&self, path: &HjsonPath) -> Result<RedNode, HjsonEditError> {
        let walker = PathWalker::new(self.root().clone(), path);
        walker
            .last()
            .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))
            .flatten()
    }

    pub fn set_internal(
        &mut self,
        path: &HjsonPath,
        value: impl Into<NodeOrToken<GreenNode, GreenToken>>,
    ) -> Result<(), HjsonEditError> {
        let node_or_token = value.into();
        let new_value = match node_or_token {
            NodeOrToken::Node(node)
                if HjsonLanguage::kind_from_raw(node_or_token.kind()) == SyntaxKind::Value =>
            {
                node
            }
            NodeOrToken::Node(child) => {
                GreenNode::new(SyntaxKind::Value.into(), vec![child.into()])
            }
            NodeOrToken::Token(child) => {
                GreenNode::new(SyntaxKind::Value.into(), vec![child.into()])
            }
        };

        let target = self.get_internal(path)?;
        let new_root = target.replace_with(new_value);
        self.root = RedNode::new_root(new_root);
        Ok(())
    }

    pub fn remove_internal(&mut self, path: &HjsonPath) -> Result<(), HjsonEditError> {
        let mut target = self.get_internal(path)?;
        while target.kind() != SyntaxKind::Pair {
            target = target
                .parent()
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Pair,
                    found: target.kind(),
                })?;
        }

        let target_parent = target
            .parent()
            .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: target.kind(),
            })?;

        let target_index = target.index();
        let pair = Pair::cast(target)?;

        let start = pair
            .leading_trivia()?
            .first()
            .map(|t| t.index())
            .unwrap_or(target_index);
        let end = pair
            .trailing_trivia()?
            .last()
            .map(|t| t.index())
            .unwrap_or(target_index);

        // TODO: Greenに対するsplice_childrenとRedへのreplace_withにすべき
        target_parent.splice_children(start..end + 1, Vec::new());
        let new_root = target_parent.ancestors().last().unwrap();
        self.root = new_root;
        Ok(())
    }
}
