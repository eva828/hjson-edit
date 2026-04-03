use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    high_level::{
        error::HjsonEditError,
        ext::RedNodeExt,
        ops::finish_document,
        path::{ArrayPathSegment, HjsonObjectPath, ObjectPathSegment, PathSegment},
        value::HjsonValue,
        HjsonDocument,
    },
    syntax::{GreenElement, RedNode, SyntaxKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringStyle {
    #[default]
    Preserve,
    DoubleQuoted,
    Block,
    Unquoted,
}

pub trait HjsonDocumentObjectEdit {
    fn get(&self, path: impl AsRef<str>) -> Result<HjsonValue, HjsonEditError>;
    fn set(
        self,
        path: impl AsRef<str>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: impl AsRef<str>) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(&self, path: impl AsRef<str>) -> Result<HjsonValue, HjsonEditError> {
        let path = HjsonObjectPath::parse(path.as_ref().to_string())?;
        let root = self.root().clone().document_start_node();
        let node = path.segments().iter().try_fold(root, |current, segment| {
            current
                .child(segment)
                .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))
        })?;
        node.node_to_value()
    }

    fn set(
        self,
        path: impl AsRef<str>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let path = HjsonObjectPath::parse(path.as_ref().to_string())?;
        let value = value.into();
        let root = self.root().clone().document_start_node();
        let updated = if path.depth() == 0 {
            value.value_to_green()
        } else {
            rebuild_tree_with_set(root.clone(), &path, &value)?
        };
        let updated = root.format_like(updated);
        Ok(finish_document(self, updated))
    }

    fn remove(self, path: impl AsRef<str>) -> Result<Self, HjsonEditError> {
        let path = HjsonObjectPath::parse(path.as_ref().to_string())?;
        let root = self.root().clone().document_start_node();
        let updated = rebuild_tree_with_remove(root.clone(), &path)?;
        let updated = root.format_like(updated);
        Ok(finish_document(self, updated))
    }
}

fn rebuild_tree_with_set(
    root: RedNode,
    path: &HjsonObjectPath,
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    let (parent_path, last) = path
        .split_last()
        .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;
    let parent = resolve_parent(root.clone(), &parent_path, true)?;
    let new_parent = match parent.kind() {
        SyntaxKind::Root => set_root_child(&parent, &last, value)?,
        SyntaxKind::Object => set_object_child(&parent, &last, value)?,
        SyntaxKind::Array => set_array_child(&parent, &last, value)?,
        SyntaxKind::Value => set_value_child(&parent, &last, value)?,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: parent.kind(),
            })
        }
    };
    Ok(parent.replace_with(new_parent))
}

fn rebuild_tree_with_remove(
    root: RedNode,
    path: &HjsonObjectPath,
) -> Result<GreenNode, HjsonEditError> {
    let (parent_path, last) = path
        .split_last()
        .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;
    let parent = resolve_parent(root.clone(), &parent_path, false)?;
    let new_parent = match parent.kind() {
        SyntaxKind::Root => remove_root_child(&parent, &last)?,
        SyntaxKind::Object => remove_object_child(&parent, &last)?,
        SyntaxKind::Array => remove_array_child(&parent, &last)?,
        SyntaxKind::Value => remove_value_child(&parent, &last)?,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: parent.kind(),
            })
        }
    };
    Ok(parent.replace_with(new_parent))
}

fn resolve_parent(
    root: RedNode,
    path: &HjsonObjectPath,
    create_missing: bool,
) -> Result<RedNode, HjsonEditError> {
    let mut current = root.document_start_node();
    for (idx, segment) in path.segments().iter().enumerate() {
        if let Some(next) = current.child(segment) {
            current = next;
            continue;
        }
        if create_missing {
            return Err(HjsonEditError::PathNotFound(
                path.split_at(idx + 1).unwrap().0,
            ));
        }
        return Err(HjsonEditError::PathNotFound(
            path.split_at(idx + 1).unwrap().0,
        ));
    }
    Ok(current)
}

fn set_root_child(
    _parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    match last {
        PathSegment::Object(ObjectPathSegment::Key(_)) | PathSegment::Array(_) => {
            Ok(value.value_to_green())
        }
    }
}

fn set_value_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    let child = parent
        .children()
        .find(|child| !child.kind().is_trivia())
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = rebuild_child_with_set(&child, last, value)?;
    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();
    let mut replaced = false;
    let key = match last {
        PathSegment::Object(ObjectPathSegment::Key(key)) => key,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Pair,
                found: SyntaxKind::Object,
            })
        }
    };

    for child in parent.children_with_tokens() {
        match child {
            NodeOrToken::Node(pair)
                if pair.kind() == SyntaxKind::Pair
                    && pair.pair_key().as_deref() == Some(key.as_str()) =>
            {
                children.push(value.pair_to_green(key).into());
                replaced = true;
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBrace && !replaced => {
                children.push(value.pair_to_green(key).into());
                children.push(owned_green(NodeOrToken::Token(token)));
                replaced = true;
            }
            NodeOrToken::Node(node) => children.push(owned_green(NodeOrToken::Node(node))),
            NodeOrToken::Token(token) => children.push(owned_green(NodeOrToken::Token(token))),
        }
    }

    if !replaced {
        children.push(value.pair_to_green(key).into());
    }

    Ok(SyntaxKind::Object.create_node(children))
}

fn set_array_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    let seg = match last {
        PathSegment::Array(seg) => seg,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: SyntaxKind::Object,
            })
        }
    };
    let items = array_values(parent)?;
    let mut values = Vec::new();

    match seg {
        ArrayPathSegment::Index(idx) => {
            if *idx >= items.len() {
                return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
                    last.clone(),
                ])));
            }
            for (i, item) in items.iter().enumerate() {
                if i == *idx {
                    values.push(value.value_to_green().into());
                } else {
                    values.push(item.clone().into());
                }
            }
        }
        ArrayPathSegment::InsertAt(idx) => {
            for (i, item) in items.iter().enumerate() {
                if i == *idx {
                    values.push(value.value_to_green().into());
                }
                values.push(item.clone().into());
            }
            if *idx >= items.len() {
                values.push(value.value_to_green().into());
            }
        }
        ArrayPathSegment::Push => {
            values.extend(items.into_iter().map(Into::into));
            values.push(value.value_to_green().into());
        }
    }

    Ok(SyntaxKind::Array.create_node(values))
}

fn remove_root_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    match last {
        PathSegment::Object(_) => Ok(parent
            .children()
            .find(|c| !c.kind().is_trivia())
            .ok_or(HjsonEditError::ParentNotFound)?
            .replace_with(SyntaxKind::Value.create_node(vec![]))),
        PathSegment::Array(_) => Ok(parent
            .children()
            .find(|c| !c.kind().is_trivia())
            .ok_or(HjsonEditError::ParentNotFound)?
            .replace_with(SyntaxKind::Value.create_node(vec![]))),
    }
}

fn remove_value_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let child = parent
        .children()
        .find(|child| !child.kind().is_trivia())
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = rebuild_child_with_remove(&child, last)?;
    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn remove_object_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Object(ObjectPathSegment::Key(key)) => key,
        _ => {
            return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
                last.clone(),
            ])))
        }
    };
    let mut children = Vec::new();
    let mut removed = false;
    for child in parent.children_with_tokens() {
        match child {
            NodeOrToken::Node(pair)
                if pair.kind() == SyntaxKind::Pair
                    && pair.pair_key().as_deref() == Some(key.as_str()) =>
            {
                removed = true;
            }
            NodeOrToken::Node(node) => children.push(owned_green(NodeOrToken::Node(node))),
            NodeOrToken::Token(token) => children.push(owned_green(NodeOrToken::Token(token))),
        }
    }
    if !removed {
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ])));
    }
    Ok(SyntaxKind::Object.create_node(children))
}

fn remove_array_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let seg = match last {
        PathSegment::Array(seg) => seg,
        _ => {
            return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
                last.clone(),
            ])))
        }
    };
    let items = array_values(parent)?;
    let mut values = Vec::new();
    let mut removed = false;
    match seg {
        ArrayPathSegment::Index(idx) => {
            for (i, item) in items.into_iter().enumerate() {
                if i == *idx {
                    removed = true;
                    continue;
                }
                values.push(item.into());
            }
        }
        ArrayPathSegment::InsertAt(idx) => {
            for (i, item) in items.into_iter().enumerate() {
                if i == *idx {
                    removed = true;
                    continue;
                }
                values.push(item.into());
            }
        }
        ArrayPathSegment::Push => {
            let keep_len = items.len().saturating_sub(1);
            if items.is_empty() {
                return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
                    last.clone(),
                ])));
            }
            removed = true;
            values.extend(items.into_iter().take(keep_len).map(Into::into));
        }
    }
    if !removed {
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ])));
    }
    Ok(SyntaxKind::Array.create_node(values))
}

fn rebuild_child_with_set(
    child: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    match child.kind() {
        SyntaxKind::Object => set_object_child(child, last, value),
        SyntaxKind::Array => set_array_child(child, last, value),
        SyntaxKind::Value => set_value_child(child, last, value),
        _ => Ok(value.value_to_green()),
    }
}

fn rebuild_child_with_remove(
    child: &RedNode,
    last: &PathSegment,
) -> Result<GreenNode, HjsonEditError> {
    match child.kind() {
        SyntaxKind::Object => remove_object_child(child, last),
        SyntaxKind::Array => remove_array_child(child, last),
        SyntaxKind::Value => Ok(child.replace_with(SyntaxKind::Value.create_node(vec![]))),
        _ => Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ]))),
    }
}

fn array_values(node: &RedNode) -> Result<Vec<GreenNode>, HjsonEditError> {
    let array = if node.kind() == SyntaxKind::Array {
        node.clone()
    } else {
        node.children()
            .find(|child| child.kind() == SyntaxKind::Array)
            .ok_or(HjsonEditError::ParentNotFound)?
    };
    Ok(array
        .children()
        .filter(|child| child.kind() == SyntaxKind::Value)
        .map(|child| child.green().clone().into())
        .collect())
}

fn owned_green(child: NodeOrToken<RedNode, crate::RedToken>) -> GreenElement {
    match child {
        NodeOrToken::Node(node) => node.green().clone().into(),
        NodeOrToken::Token(token) => GreenToken::new(token.kind().into(), token.text()).into(),
    }
}
