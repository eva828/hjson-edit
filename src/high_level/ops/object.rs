use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    ast::QuoteStyle,
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
        let path_str = path.as_ref().to_string();
        let path = HjsonObjectPath::parse(path_str.clone())?;
        let value = value.into();

        if let Ok(current) = self.get(&path_str) {
            if current == value {
                return Ok(self);
            }
        }

        let root = self.root().clone();
        let string_style = self.current_string_style();
        let updated = if path.depth() == 0 {
            replace_root_value(&root, &value, string_style)?
        } else {
            rebuild_tree_with_set(root.clone(), &path, &value, string_style)?
        };

        Ok(finish_document(self, updated))
    }

    fn remove(self, path: impl AsRef<str>) -> Result<Self, HjsonEditError> {
        let path = HjsonObjectPath::parse(path.as_ref().to_string())?;
        let root = self.root().clone();
        let updated = rebuild_tree_with_remove(root.clone(), &path)?;
        Ok(finish_document(self, updated))
    }
}

fn replace_root_value(
    root: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let child = root
        .children()
        .find(|child| {
            matches!(
                child.kind(),
                SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value
            )
        })
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = render_value_like_node(&child, value, string_style);

    Ok(root
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn rebuild_tree_with_set(
    root: RedNode,
    path: &HjsonObjectPath,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let (parent_path, last) = path
        .split_last()
        .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;
    let parent = resolve_parent(root.clone(), &parent_path)?;
    let new_parent = match parent.kind() {
        SyntaxKind::Root => set_root_child(&parent, &last, value, string_style)?,
        SyntaxKind::Object => set_object_child(&parent, &last, value, string_style)?,
        SyntaxKind::Array => set_array_child(&parent, &last, value, string_style)?,
        SyntaxKind::Value => set_value_child(&parent, &last, value, string_style)?,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: parent.kind(),
            })
        }
    };

    if parent.kind() == SyntaxKind::Root {
        Ok(new_parent)
    } else {
        Ok(parent.replace_with(new_parent))
    }
}

fn rebuild_tree_with_remove(
    root: RedNode,
    path: &HjsonObjectPath,
) -> Result<GreenNode, HjsonEditError> {
    let (parent_path, last) = path
        .split_last()
        .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;
    let parent = resolve_parent(root.clone(), &parent_path)?;
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

    if parent.kind() == SyntaxKind::Root {
        Ok(new_parent)
    } else {
        Ok(parent.replace_with(new_parent))
    }
}

fn resolve_parent(root: RedNode, path: &HjsonObjectPath) -> Result<RedNode, HjsonEditError> {
    let mut current = root;
    for segment in path.segments() {
        current = current
            .child(segment)
            .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;
    }
    Ok(current)
}

fn set_root_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let child = parent
        .children()
        .find(|child| {
            matches!(
                child.kind(),
                SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value
            )
        })
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = rebuild_child_with_set(&child, last, value, string_style)?;

    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn set_value_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let child = parent
        .children()
        .find(|child| !child.kind().is_trivia())
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = rebuild_child_with_set(&child, last, value, string_style)?;

    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Object(ObjectPathSegment::Key(key)) => key,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Pair,
                found: SyntaxKind::Object,
            })
        }
    };

    let mut children: Vec<GreenElement> = Vec::new();
    let mut replaced = false;
    let insert_style = resolve_quote_style(string_style, detect_string_style(parent));

    for child in parent.children_with_tokens() {
        match child {
            NodeOrToken::Node(pair)
                if pair.kind() == SyntaxKind::Pair
                    && pair.pair_key().as_deref() == Some(key.as_str()) =>
            {
                children.push(replace_pair_value(&pair, value, string_style)?.into());
                replaced = true;
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBrace && !replaced => {
                children.push(value.render_pair(key, None, Some(insert_style)).into());
                children.push(owned_green(NodeOrToken::Token(token)));
                replaced = true;
            }
            NodeOrToken::Node(node) => children.push(owned_green(NodeOrToken::Node(node))),
            NodeOrToken::Token(token) => children.push(owned_green(NodeOrToken::Token(token))),
        }
    }

    if !replaced {
        let rbrace_pos =
            green_bracket_index(&children, SyntaxKind::RBrace).unwrap_or(children.len());
        let last_pair_pos = children.iter().rposition(
            |child| matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair.into()),
        );
        let tail_start = last_pair_pos.map(|pos| pos + 1).unwrap_or(rbrace_pos);
        let tail_slice: &[GreenElement] = children.get(tail_start..rbrace_pos).unwrap_or(&[]);
        let mut inserted = Vec::new();

        if last_pair_pos.is_some() {
            if tail_slice.is_empty() {
                inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
            } else {
                inserted.extend(copy_green_elements(tail_slice));
            }
        }

        inserted.push(value.render_pair(key, None, Some(insert_style)).into());

        return Ok(parent
            .green()
            .splice_children(tail_start..tail_start, inserted));
    }

    Ok(SyntaxKind::Object.create_node(children))
}

fn set_array_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
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

    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let value_positions = children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    let insert_style = resolve_quote_style(string_style, detect_string_style(parent));

    match seg {
        ArrayPathSegment::Index(idx) => {
            let Some(value_pos) = value_positions.get(*idx).copied() else {
                return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
                    last.clone(),
                ])));
            };
            let current_value = match &children[value_pos] {
                NodeOrToken::Node(node) => node,
                _ => unreachable!(),
            };
            let updated = render_value_like_node(current_value, value, string_style);
            Ok(parent
                .green()
                .splice_children(value_pos..value_pos + 1, vec![updated.into()]))
        }
        ArrayPathSegment::InsertAt(idx) => {
            let insert_pos = if *idx < value_positions.len() {
                value_positions[*idx]
            } else {
                red_bracket_index(&children, SyntaxKind::RBracket).unwrap_or(children.len())
            };

            let prev_boundary = if *idx == 0 {
                red_bracket_index(&children, SyntaxKind::LBracket).unwrap_or(0)
            } else {
                value_positions
                    .get(idx.saturating_sub(1))
                    .copied()
                    .unwrap_or(0)
            };
            let separator_slice: &[NodeOrToken<RedNode, crate::RedToken>] =
                children.get(prev_boundary + 1..insert_pos).unwrap_or(&[]);
            let mut inserted = copy_separator_tokens(separator_slice);
            inserted.push(value.render_value(Some(insert_style)).into());

            Ok(parent
                .green()
                .splice_children(prev_boundary + 1..prev_boundary + 1, inserted))
        }
        ArrayPathSegment::Push => {
            let insert_pos =
                red_bracket_index(&children, SyntaxKind::RBracket).unwrap_or(children.len());
            let last_value_pos = value_positions.last().copied();
            let tail_start = last_value_pos.map(|pos| pos + 1).unwrap_or(insert_pos);
            let tail_slice: &[NodeOrToken<RedNode, crate::RedToken>] =
                children.get(tail_start..insert_pos).unwrap_or(&[]);
            let mut inserted = Vec::new();

            if last_value_pos.is_some() {
                if tail_slice.is_empty() {
                    inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                    inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                } else {
                    inserted.extend(copy_separator_tokens(tail_slice));
                }
            }

            inserted.push(value.render_value(Some(insert_style)).into());

            Ok(parent
                .green()
                .splice_children(tail_start..tail_start, inserted))
        }
    }
}

fn remove_root_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let child = parent
        .children()
        .find(|child| {
            matches!(
                child.kind(),
                SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value
            )
        })
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = rebuild_child_with_remove(&child, last)?;

    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
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

    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let pair_pos = children.iter().position(|child| {
        matches!(child, NodeOrToken::Node(pair) if pair.kind() == SyntaxKind::Pair.into() && pair.pair_key().as_deref() == Some(key.as_str()))
    });

    let Some(pair_pos) = pair_pos else {
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ])));
    };

    let boundary = children
        .iter()
        .enumerate()
        .skip(pair_pos + 1)
        .find_map(|(idx, child)| match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair => Some(idx),
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBrace => Some(idx),
            _ => None,
        })
        .unwrap_or(children.len());

    Ok(parent.green().splice_children(pair_pos..boundary, vec![]))
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

    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let value_positions = children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value)
                .then_some(idx)
        })
        .collect::<Vec<_>>();

    let value_pos = match seg {
        ArrayPathSegment::Index(idx) | ArrayPathSegment::InsertAt(idx) => {
            value_positions.get(*idx).copied().ok_or_else(|| {
                HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![last.clone()]))
            })?
        }
        ArrayPathSegment::Push => value_positions.last().copied().ok_or_else(|| {
            HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![last.clone()]))
        })?,
    };

    let boundary = match next_value_or_bracket_index(&children, value_pos) {
        Some(idx) => idx,
        None => children.len(),
    };

    Ok(parent.green().splice_children(value_pos..boundary, vec![]))
}

fn rebuild_child_with_set(
    child: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    match child.kind() {
        SyntaxKind::Object => set_object_child(child, last, value, string_style),
        SyntaxKind::Array => set_array_child(child, last, value, string_style),
        SyntaxKind::Value => set_value_child(child, last, value, string_style),
        _ => Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ]))),
    }
}

fn rebuild_child_with_remove(
    child: &RedNode,
    last: &PathSegment,
) -> Result<GreenNode, HjsonEditError> {
    match child.kind() {
        SyntaxKind::Object => remove_object_child(child, last),
        SyntaxKind::Array => remove_array_child(child, last),
        SyntaxKind::Value => remove_value_child(child, last),
        _ => Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ]))),
    }
}

fn render_value_like_node(
    node: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> GreenNode {
    let quote_style = resolve_quote_style(string_style, detect_string_style(node));
    value.render_value(Some(quote_style))
}

fn replace_pair_value(
    pair: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let value_child = pair
        .children()
        .find(|child| child.kind() == SyntaxKind::Value)
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_value = render_value_like_node(&value_child, value, string_style);

    Ok(pair.green().splice_children(
        value_child.index()..value_child.index() + 1,
        vec![updated_value.into()],
    ))
}

fn detect_string_style(node: &RedNode) -> Option<QuoteStyle> {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) => match token.kind() {
                SyntaxKind::String => return Some(QuoteStyle::DoubleQuoted),
                SyntaxKind::BlockString => return Some(QuoteStyle::Block),
                SyntaxKind::UnquotedString => return Some(QuoteStyle::Unquoted),
                _ => {}
            },
            NodeOrToken::Node(node) => {
                if let Some(style) = detect_string_style(&node) {
                    return Some(style);
                }
            }
        }
    }
    None
}

fn green_bracket_index(children: &[GreenElement], kind: SyntaxKind) -> Option<usize> {
    children
        .iter()
        .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == kind.into()))
}

fn red_bracket_index(
    children: &[NodeOrToken<RedNode, crate::RedToken>],
    kind: SyntaxKind,
) -> Option<usize> {
    children
        .iter()
        .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == kind))
}

fn next_value_or_bracket_index(
    children: &[NodeOrToken<RedNode, crate::RedToken>],
    start: usize,
) -> Option<usize> {
    children
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(idx, child)| match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value => Some(idx),
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBracket => Some(idx),
            _ => None,
        })
}

fn copy_green_elements(separator_slice: &[GreenElement]) -> Vec<GreenElement> {
    separator_slice.to_vec()
}

fn copy_separator_tokens(
    separator_slice: &[NodeOrToken<RedNode, crate::RedToken>],
) -> Vec<GreenElement> {
    separator_slice
        .iter()
        .filter_map(|child| match child {
            NodeOrToken::Token(token)
                if token.kind() == SyntaxKind::Comma
                    || token.kind() == SyntaxKind::Whitespace
                    || token.kind() == SyntaxKind::Newline =>
            {
                Some(GreenToken::new(token.kind().into(), token.text()).into())
            }
            _ => None,
        })
        .collect()
}

fn resolve_quote_style(preference: StringStyle, detected: Option<QuoteStyle>) -> QuoteStyle {
    match preference {
        StringStyle::Preserve => detected.unwrap_or(QuoteStyle::DoubleQuoted),
        StringStyle::DoubleQuoted => QuoteStyle::DoubleQuoted,
        StringStyle::Block => QuoteStyle::Block,
        StringStyle::Unquoted => QuoteStyle::Unquoted,
    }
}

fn owned_green(child: NodeOrToken<RedNode, crate::RedToken>) -> GreenElement {
    match child {
        NodeOrToken::Node(node) => node.green().clone().into(),
        NodeOrToken::Token(token) => GreenToken::new(token.kind().into(), token.text()).into(),
    }
}
