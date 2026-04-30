use std::ops::Range;

use crate::{
    high_level::{ext::RedNodeExt, path::HjsonObjectPath},
    HjsonDocument, HjsonEditError, RedNode,
};

pub struct PathWalker<'a> {
    current: RedNode,
    path: &'a HjsonObjectPath,
    index_iter: Range<usize>,
}

impl<'a> PathWalker<'a> {
    pub fn new(root: RedNode, path: &'a HjsonObjectPath) -> Self {
        Self {
            current: root,
            path,
            index_iter: (0..path.depth()).into_iter(),
        }
    }
}

impl<'a> Iterator for PathWalker<'a> {
    type Item = Result<RedNode, HjsonEditError>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index_iter.next()?;
        let segment = &self.path.segment(index)?;
        let (path_part, _) = self.path.split_at(index + 1).unwrap();

        let child = self
            .current
            .child(segment)
            .ok_or_else(|| HjsonEditError::PathNotFound(path_part));

        match child {
            Ok(ref node) => {
                self.current = node.clone();
                Some(child)
            }
            Err(_) => Some(child),
        }
    }
}

impl<'a> HjsonDocument {
    pub fn walker(&'a self, path: &'a HjsonObjectPath) -> PathWalker<'a> {
        PathWalker::new(self.root.clone(), path)
    }
}
