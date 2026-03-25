use crate::high_level::{
    error::HjsonEditError,
    path::{ArrayPathSegment, HjsonPath, ObjectPathSegment, PathSegment},
    value::HjsonValue,
    HjsonDocument,
};
// use crate::syntax::ext::RedNodeExt;
use crate::syntax::{GreenElement, RedNode, SyntaxKind};
use rowan::{GreenNode, GreenNodeBuilder, GreenToken, Language, NodeOrToken};

pub trait HjsonDocumentObjectEdit {
    fn get(&self, path: impl Into<HjsonPath>) -> Result<HjsonValue, HjsonEditError>;
    fn set(
        self,
        path: impl Into<HjsonPath>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringStyle {
    #[default]
    Preserve,
    Auto,
    DoubleQuoted,
    Block,
    Unquoted,
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(&self, path: impl Into<HjsonPath>) -> Result<HjsonValue, HjsonEditError> {
        let path = path.into();
        if path.segments.is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let red_node = RedNode::new_root(self.green.clone());
        let target_node = navigate_to_path(&red_node, &path.segments, 0, false)?;

        match target_node {
            Some(node) => Ok(HjsonValue::from(node)),
            None => Err(HjsonEditError::PathNotFound(path)),
        }
    }

    fn set(
        self,
        path: impl Into<HjsonPath>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.into();
        let value = value.into();

        if path.segments.is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let new_green = rebuild_tree_with_set(self.green.clone(), &path.segments, &value)?;

        Ok(Self {
            green: new_green,
            string_style: self.string_style,
            commnt_style: self.commnt_style,
        })
    }

    fn remove(self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.into();

        if path.segments.is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let new_green = rebuild_tree_with_remove(self.green.clone(), &path.segments)?;

        Ok(Self {
            green: new_green,
            string_style: self.string_style,
            commnt_style: self.commnt_style,
        })
    }
}

fn navigate_to_path(
    node: &RedNode,
    segments: &[PathSegment],
    idx: usize,
    create_missing: bool,
) -> Result<Option<RedNode>, HjsonEditError> {
    if idx >= segments.len() {
        return Ok(Some(node.clone()));
    }

    // If we're at Root or Value, traverse to the actual content first
    if node.kind() == SyntaxKind::Root || node.kind() == SyntaxKind::Value {
        // First look for Object, Array, or Pair directly
        let direct = node.children().find(|child| {
            matches!(
                child.kind(),
                SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Pair
            )
        });

        if let Some(n) = direct {
            // Found Object/Array/Pair directly, navigate into it
            return navigate_to_path(&n, segments, idx, create_missing);
        }

        // If not found, look for Value and navigate into it
        if let Some(value_node) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::Value)
        {
            return navigate_to_path(&value_node, segments, idx, create_missing);
        }

        // return Err(HjsonEditError::ChildNotFound(path));
        unimplemented!()
    }

    let segment = &segments[idx];

    match segment {
        PathSegment::Object(ObjectPathSegment::Key(key)) => {
            if node.kind() == SyntaxKind::Object {
                let pair = node.find_pair_by_key(key);
                match pair {
                    Some(p) => {
                        // Navigate to the value inside the pair
                        let value_node =
                            p.children().find(|child| child.kind() == SyntaxKind::Value);
                        match value_node {
                            Some(v) => navigate_to_path(&v, segments, idx + 1, create_missing),
                            None => {
                                if create_missing {
                                    Ok(None)
                                } else {
                                    Err(HjsonEditError::PathNotFound(segments[..idx + 1].to_vec()))
                                }
                            }
                        }
                    }
                    None => {
                        if create_missing {
                            Ok(None)
                        } else {
                            Err(HjsonEditError::PathNotFound(segments[..idx + 1].to_vec()))
                        }
                    }
                }
            } else {
                Err(HjsonEditError::PathNotFound(segments[..idx + 1].to_vec()))
            }
        }
        PathSegment::Array(op) => {
            if node.kind() == SyntaxKind::Array {
                let result = node.get_array_element(op)?;
                match result {
                    Some(element) => navigate_to_path(&element, segments, idx + 1, create_missing),
                    None => {
                        if create_missing
                            || matches!(op, ArrayPathSegment::Push | ArrayPathSegment::InsertAt(_))
                        {
                            Ok(None)
                        } else {
                            Err(HjsonEditError::PathNotFound(segments[..idx + 1].to_vec()))
                        }
                    }
                }
            } else {
                Err(HjsonEditError::PathNotFound(segments[..idx + 1].to_vec()))
            }
        }
    }
}
