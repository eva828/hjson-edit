use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    ast::{AstNode, QuoteStyle, Value as AstValue},
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
    match resolve_parent(root.clone(), &parent_path) {
        Ok(parent) => {
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
        Err(HjsonEditError::PathNotFound(_)) => {
            let (ancestor, consumed) = resolve_existing_prefix(root.clone(), path.segments());
            if consumed == 0 || consumed >= path.depth() {
                return Err(HjsonEditError::PathNotFound(path.clone()));
            }

            let missing = &path.segments()[consumed];
            let nested_value = build_nested_value(&path.segments()[consumed + 1..], value.clone());
            let new_parent = match ancestor.kind() {
                SyntaxKind::Root => {
                    set_root_child(&ancestor, missing, &nested_value, string_style)?
                }
                SyntaxKind::Object => {
                    set_object_child(&ancestor, missing, &nested_value, string_style)?
                }
                SyntaxKind::Array => {
                    set_array_child(&ancestor, missing, &nested_value, string_style)?
                }
                SyntaxKind::Value => {
                    set_value_child(&ancestor, missing, &nested_value, string_style)?
                }
                _ => {
                    return Err(HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Object,
                        found: ancestor.kind(),
                    })
                }
            };

            if ancestor.kind() == SyntaxKind::Root {
                Ok(new_parent)
            } else {
                Ok(ancestor.replace_with(new_parent))
            }
        }
        Err(err) => Err(err),
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

fn resolve_existing_prefix(root: RedNode, path: &[PathSegment]) -> (RedNode, usize) {
    let mut current = root.document_start_node();
    let mut consumed = 0usize;

    for segment in path {
        match current.child(segment) {
            Some(next) => {
                current = next;
                consumed += 1;
            }
            None => break,
        }
    }

    (current, consumed)
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

    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let insert_style = resolve_quote_style(string_style, detect_string_style(parent));
    let rendered_pair = value.render_pair(key, None, Some(insert_style));

    if let Some((pair_pos, pair)) = children.iter().enumerate().find(|(_, child)| {
        matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair && node.pair_key().as_deref() == Some(key.as_str()))
    }) {
        let updated = match pair {
            NodeOrToken::Node(node) => replace_pair_value(node, value, string_style)?,
            _ => unreachable!(),
        };
        return Ok(parent
            .green()
            .splice_children(pair_pos..pair_pos + 1, vec![updated.into()]));
    }

    let rbrace_pos = red_bracket_index(&children, SyntaxKind::RBrace).unwrap_or(children.len());
    let pair_positions = children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    let Some(&last_pair_pos) = pair_positions.last() else {
        return Ok(parent
            .green()
            .splice_children(rbrace_pos..rbrace_pos, vec![rendered_pair.into()]));
    };

    let before_slice = if pair_positions.len() >= 2 {
        let prev_pair_pos = pair_positions[pair_positions.len() - 2];
        children
            .get(prev_pair_pos + 1..last_pair_pos)
            .unwrap_or(&[])
    } else {
        let lbrace_pos = red_bracket_index(&children, SyntaxKind::LBrace).unwrap_or(0);
        children.get(lbrace_pos + 1..last_pair_pos).unwrap_or(&[])
    };
    let after_slice = children.get(last_pair_pos + 1..rbrace_pos).unwrap_or(&[]);

    let mut inserted = copy_separator_tokens(before_slice);
    if inserted.is_empty() {
        inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
        inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
    }
    inserted.push(rendered_pair.into());
    inserted.extend(clone_green_elements(after_slice));

    Ok(parent
        .green()
        .splice_children(last_pair_pos + 1..rbrace_pos, inserted))
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
            if *idx >= value_positions.len() {
                let insert_pos =
                    red_bracket_index(&children, SyntaxKind::RBracket).unwrap_or(children.len());
                let last_value_pos = value_positions.last().copied();
                let Some(last_value_pos) = last_value_pos else {
                    return Ok(parent.green().splice_children(
                        insert_pos..insert_pos,
                        vec![value.render_value(Some(insert_style)).into()],
                    ));
                };

                let before_slice = if value_positions.len() >= 2 {
                    let prev_value_pos = value_positions[value_positions.len() - 2];
                    children
                        .get(prev_value_pos + 1..last_value_pos)
                        .unwrap_or(&[])
                } else {
                    let lbracket_pos =
                        red_bracket_index(&children, SyntaxKind::LBracket).unwrap_or(0);
                    children
                        .get(lbracket_pos + 1..last_value_pos)
                        .unwrap_or(&[])
                };
                let after_slice = children.get(last_value_pos + 1..insert_pos).unwrap_or(&[]);
                let mut inserted = copy_separator_tokens(before_slice);
                if inserted.is_empty() {
                    inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                    inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                }
                inserted.push(value.render_value(Some(insert_style)).into());
                inserted.extend(clone_green_elements(after_slice));

                Ok(parent
                    .green()
                    .splice_children(last_value_pos + 1..insert_pos, inserted))
            } else {
                let insert_pos = value_positions[*idx];
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
                let current_value = match &children[insert_pos] {
                    NodeOrToken::Node(node) => node,
                    _ => unreachable!(),
                };
                let current_value_ast = AstValue::cast(current_value.clone())?;
                let suffix_len = current_value_ast
                    .leading_trivia
                    .len()
                    .min(separator_slice.len());
                let split_at = separator_slice.len().saturating_sub(suffix_len);
                let (prefix_slice, suffix_slice) = separator_slice.split_at(split_at);

                let mut inserted = clone_green_elements(prefix_slice);
                if inserted.is_empty() {
                    inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                    inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                } else if matches!(
                    prefix_slice.last(),
                    Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Comma
                ) {
                    inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                }
                inserted.push(value.render_value(Some(insert_style)).into());
                inserted.extend(clone_green_elements(suffix_slice));

                Ok(parent
                    .green()
                    .splice_children(prev_boundary + 1..insert_pos, inserted))
            }
        }
        ArrayPathSegment::Push => {
            let insert_pos =
                red_bracket_index(&children, SyntaxKind::RBracket).unwrap_or(children.len());
            let Some(last_value_pos) = value_positions.last().copied() else {
                return Ok(parent.green().splice_children(
                    insert_pos..insert_pos,
                    vec![value.render_value(Some(insert_style)).into()],
                ));
            };

            let before_slice = if value_positions.len() >= 2 {
                let prev_value_pos = value_positions[value_positions.len() - 2];
                children
                    .get(prev_value_pos + 1..last_value_pos)
                    .unwrap_or(&[])
            } else {
                let lbracket_pos = red_bracket_index(&children, SyntaxKind::LBracket).unwrap_or(0);
                children
                    .get(lbracket_pos + 1..last_value_pos)
                    .unwrap_or(&[])
            };
            let after_slice = children.get(last_value_pos + 1..insert_pos).unwrap_or(&[]);
            let mut inserted = copy_separator_tokens(before_slice);
            if inserted.is_empty() {
                inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
            }
            inserted.push(value.render_value(Some(insert_style)).into());
            inserted.extend(clone_green_elements(after_slice));

            Ok(parent
                .green()
                .splice_children(last_value_pos + 1..insert_pos, inserted))
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

    let start = children[..pair_pos]
        .iter()
        .rposition(|child| match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair => true,
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::LBrace => true,
            _ => false,
        })
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let start = start + protected_trivia_prefix_len(&children[start..pair_pos]);

    Ok(parent.green().splice_children(start..pair_pos + 1, vec![]))
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

    let start = children[..value_pos]
        .iter()
        .rposition(|child| match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value => true,
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::LBracket => true,
            _ => false,
        })
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let start = start + protected_trivia_prefix_len(&children[start..value_pos]);

    Ok(parent.green().splice_children(start..value_pos + 1, vec![]))
}

fn build_nested_value(path: &[PathSegment], value: HjsonValue) -> HjsonValue {
    let mut current = value;
    for segment in path.iter().rev() {
        current = match segment {
            PathSegment::Object(ObjectPathSegment::Key(key)) => {
                HjsonValue::Object(vec![(key.clone(), current)])
            }
            PathSegment::Array(ArrayPathSegment::Index(_))
            | PathSegment::Array(ArrayPathSegment::InsertAt(_))
            | PathSegment::Array(ArrayPathSegment::Push) => HjsonValue::Array(vec![current]),
        };
    }

    current
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

fn red_bracket_index(
    children: &[NodeOrToken<RedNode, crate::RedToken>],
    kind: SyntaxKind,
) -> Option<usize> {
    children
        .iter()
        .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == kind))
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

fn clone_green_elements(slice: &[NodeOrToken<RedNode, crate::RedToken>]) -> Vec<GreenElement> {
    slice
        .iter()
        .map(|child| match child {
            NodeOrToken::Node(node) => node.green().clone().into(),
            NodeOrToken::Token(token) => GreenToken::new(token.kind().into(), token.text()).into(),
        })
        .collect()
}

fn protected_trivia_prefix_len(slice: &[NodeOrToken<RedNode, crate::RedToken>]) -> usize {
    let mut boundary = 0usize;
    let mut index = 0usize;
    let mut saw_comment_since_boundary = false;

    while index < slice.len() {
        match &slice[index] {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => {
                saw_comment_since_boundary = true;
                index += 1;
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                let mut cursor = index;
                let mut newline_count = 0usize;

                while cursor < slice.len() {
                    match &slice[cursor] {
                        NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                            newline_count += 1;
                            cursor += 1;
                        }
                        NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace => {
                            cursor += 1;
                        }
                        _ => break,
                    }
                }

                if newline_count >= 2 && saw_comment_since_boundary {
                    boundary = cursor;
                    saw_comment_since_boundary = false;
                }

                index = cursor;
            }
            _ => {
                index += 1;
            }
        }
    }

    boundary
}

fn resolve_quote_style(preference: StringStyle, detected: Option<QuoteStyle>) -> QuoteStyle {
    match preference {
        StringStyle::Preserve => detected.unwrap_or(QuoteStyle::DoubleQuoted),
        StringStyle::DoubleQuoted => QuoteStyle::DoubleQuoted,
        StringStyle::Block => QuoteStyle::Block,
        StringStyle::Unquoted => QuoteStyle::Unquoted,
    }
}
