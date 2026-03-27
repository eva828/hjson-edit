use crate::{high_level::ext::RedNodeExt, HjsonDocument, HjsonPath, PathSegment, RedNode};

pub struct PathWalker<'a> {
    current: RedNode,
    remaining: std::slice::Iter<'a, PathSegment>,
}

impl<'a> PathWalker<'a> {
    pub fn new(root: RedNode, path: &'a HjsonPath) -> Self {
        Self {
            current: root,
            remaining: path.segments.iter(),
        }
    }
}

impl<'a> Iterator for PathWalker<'a> {
    type Item = RedNode;

    fn next(&mut self) -> Option<Self::Item> {
        self.current = self.current.child(self.remaining.next()?)?;
        Some(self.current.clone())
    }
}
