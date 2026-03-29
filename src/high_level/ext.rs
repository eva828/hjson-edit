use crate::{ArrayPathSegment, HjsonPath, ObjectPathSegment, PathSegment, RedNode, SyntaxKind};

pub trait RedNodeExt {
    fn children_by_kind(&self, kind: SyntaxKind) -> impl Iterator<Item = RedNode>;
    fn child(&self, path: &PathSegment) -> Option<RedNode>;
    fn path(&self) -> HjsonPath;
}

impl RedNodeExt for RedNode {
    fn children_by_kind(&self, kind: SyntaxKind) -> impl Iterator<Item = RedNode> {
        self.children().filter(move |child| child.kind() == kind)
    }

    fn child(&self, segment: &PathSegment) -> Option<RedNode> {
        if self.kind() == SyntaxKind::Value {
            return self
                .children()
                .next()
                .and_then(|child| child.child(segment));
        }

        match segment {
            PathSegment::Object(segment) if self.kind() == SyntaxKind::Object => self
                .children_by_kind(SyntaxKind::Pair)
                .filter(|pair| {
                    pair.children_by_kind(SyntaxKind::Key)
                        .map(|key| {
                            let text = key.text().to_string();
                            text.trim_matches('"').to_string()
                        })
                        .any(|key| segment.match_key(&key))
                })
                .flat_map(|pair| pair.children_by_kind(SyntaxKind::Value).next())
                .next(),
            PathSegment::Array(segment) if self.kind() == SyntaxKind::Array => {
                let mut values = self.children_by_kind(SyntaxKind::Value);
                match segment {
                    ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => {
                        values.nth(*index)
                    }
                    ArrayPathSegment::Push => values.last(),
                }
            }
            _ => self
                .children()
                .flat_map(|child| child.child(segment))
                .next(),
        }
    }

    fn path(&self) -> HjsonPath {
        let mut segments = Vec::new();
        let mut current = self.clone();

        while let Some(parent) = current.parent() {
            match parent.kind() {
                SyntaxKind::Pair => {
                    segments.push(PathSegment::Object(ObjectPathSegment::Key(
                        parent
                            .children_by_kind(SyntaxKind::Key)
                            .map(|key| key.text().to_string())
                            .collect(),
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
        HjsonPath { segments }
    }
}
