use std::ops::Range;

use crate::{
    high_level::ext::RedNodeExt, HjsonDocument, HjsonEditError, HjsonPath, PathSegment, RedNode,
};

pub struct PathWalker<'a> {
    current: RedNode,
    path: &'a HjsonPath,
    index_iter: Range<usize>,
}

impl<'a> PathWalker<'a> {
    pub fn new(root: RedNode, path: &'a HjsonPath) -> Self {
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
        let (path_part, _) = self.path.split_at(index).unwrap();
        println!("path_part: {:?}", path_part);
        let (_, path_last) = path_part.split_last().unwrap();

        let child = self
            .current
            .child(&path_last)
            .ok_or_else(|| HjsonEditError::PathNotFound(path_part));

        Some(child)
    }
}
