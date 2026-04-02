use crate::{
    high_level::{path::HjsonObjectPath, walker::PathWalker},
    HjsonDocument, HjsonEditError, RedNode, SyntaxKind,
};

pub mod comment;
pub mod object;

impl HjsonDocument {
    #[allow(dead_code)]
    fn get_internal(&self, path: &HjsonObjectPath) -> Result<RedNode, HjsonEditError> {
        let root = self.root().clone();

        if path.depth() == 0 {
            return Ok(root);
        }

        let start_node = if root.kind() == SyntaxKind::Root {
            root.children()
                .next()
                .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?
        } else {
            root
        };

        let walker = PathWalker::new(start_node, path);
        walker
            .last()
            .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))
            .flatten()
    }
}
