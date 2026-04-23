use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    ast::{
        edit as ast_edit, Array as AstArray, AstNode, Document as AstDocument, Object as AstObject,
        QuoteStyle, Trivia, Value as AstValue,
    },
    high_level::{
        error::HjsonEditError,
        ext::RedNodeExt,
        ops::finish_document,
        path::{ArrayPathSegment, HjsonObjectPath, ObjectPathSegment, PathSegment},
        value::HjsonValue,
        HjsonDocument,
    },
    syntax::{RedNode, SyntaxKind},
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

pub(crate) fn replace_root_value(
    root: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let doc_ast = AstDocument::cast(root.clone())?;
    let child = root
        .children_with_tokens()
        .nth(doc_ast.body_insert_index())
        .ok_or(HjsonEditError::ParentNotFound)?;
    let child = match child {
        NodeOrToken::Node(node) => node,
        NodeOrToken::Token(_) => return Err(HjsonEditError::ParentNotFound),
    };
    let updated_child = render_value_like_node(&child, value, string_style);

    Ok(root
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

pub(crate) fn rebuild_tree_with_set(
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

pub(crate) fn rebuild_tree_with_remove(
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
    let detected = AstObject::cast(parent.clone())
        .ok()
        .and_then(|object| object.detected_string_style());
    let insert_style = resolve_quote_style(string_style, detected);
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

    let object_ast = AstObject::cast(parent.clone())?;
    let rbrace_pos = object_ast.empty_body_insert_index();
    let pair_positions = children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    let Some(&last_pair_pos) = pair_positions.last() else {
        return Ok(parent.green().splice_children(
            object_ast.empty_body_insert_index()..object_ast.empty_body_insert_index(),
            vec![rendered_pair.into()],
        ));
    };

    let before_slice = if pair_positions.len() >= 2 {
        let prev_pair_pos = pair_positions[pair_positions.len() - 2];
        children
            .get(prev_pair_pos + 1..last_pair_pos)
            .unwrap_or(&[])
    } else {
        let lbrace_pos = ast_edit::red_bracket_index(&children, SyntaxKind::LBrace).unwrap_or(0);
        children.get(lbrace_pos + 1..last_pair_pos).unwrap_or(&[])
    };
    let after_slice = children.get(last_pair_pos + 1..rbrace_pos).unwrap_or(&[]);

    let mut inserted = ast_edit::copy_separator_tokens(before_slice);
    if inserted.is_empty() {
        inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
        inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
    }
    inserted.push(rendered_pair.into());
    inserted.extend(ast_edit::clone_green_elements(after_slice));

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
    let detected = AstArray::cast(parent.clone())
        .ok()
        .and_then(|array| array.detected_string_style());
    let insert_style = resolve_quote_style(string_style, detected);
    let array_ast = AstArray::cast(parent.clone())?;

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
                let insert_pos = array_ast.empty_body_insert_index();
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
                        ast_edit::red_bracket_index(&children, SyntaxKind::LBracket).unwrap_or(0);
                    children
                        .get(lbracket_pos + 1..last_value_pos)
                        .unwrap_or(&[])
                };
                let after_slice = children.get(last_value_pos + 1..insert_pos).unwrap_or(&[]);
                let mut inserted = ast_edit::copy_separator_tokens(before_slice);
                if inserted.is_empty() {
                    inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                    inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                }
                inserted.push(value.render_value(Some(insert_style)).into());
                inserted.extend(ast_edit::clone_green_elements(after_slice));

                Ok(parent
                    .green()
                    .splice_children(last_value_pos + 1..insert_pos, inserted))
            } else {
                let insert_pos = value_positions[*idx];
                let prev_boundary = if *idx == 0 {
                    ast_edit::red_bracket_index(&children, SyntaxKind::LBracket).unwrap_or(0)
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

                let mut inserted = ast_edit::clone_green_elements(prefix_slice);
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
                inserted.extend(ast_edit::clone_green_elements(suffix_slice));

                Ok(parent
                    .green()
                    .splice_children(prev_boundary + 1..insert_pos, inserted))
            }
        }
        ArrayPathSegment::Push => {
            let insert_pos = array_ast.append_value_index();
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
                let lbracket_pos =
                    ast_edit::red_bracket_index(&children, SyntaxKind::LBracket).unwrap_or(0);
                children
                    .get(lbracket_pos + 1..last_value_pos)
                    .unwrap_or(&[])
            };
            let after_slice = children.get(last_value_pos + 1..insert_pos).unwrap_or(&[]);
            let mut inserted = ast_edit::copy_separator_tokens(before_slice);
            if inserted.is_empty() {
                inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
            }
            inserted.push(value.render_value(Some(insert_style)).into());
            inserted.extend(ast_edit::clone_green_elements(after_slice));

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

    let object_ast = AstObject::cast(parent.clone())?;
    let members = object_ast.members();
    let pair_pos = members
        .iter()
        .position(|member| member.pair_key().as_deref() == Some(key.as_str()));

    let Some(pair_pos) = pair_pos else {
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ])));
    };

    let pair = &members[pair_pos];
    let mut range = pair.removal_range();
    let mut replacement = Vec::new();

    if pair_pos + 1 == members.len() {
        if pair_pos > 0
            && matches!(
                members[pair_pos - 1].trailing_trivia.first(),
                Some(Trivia::Comma(_))
            )
        {
            range.start = members[pair_pos - 1].post_node_index();
        }

        let prefix_len = pair
            .trivia_boundary
            .leading_protected_prefix_len
            .min(pair.leading_trivia.len());
        replacement.extend(ast_edit::render_trivia(&pair.leading_trivia[..prefix_len]));
    }

    Ok(parent.green().splice_children(range, replacement))
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

    let array_ast = AstArray::cast(parent.clone())?;
    let items = array_ast.items();
    let idx = match seg {
        ArrayPathSegment::Index(idx) | ArrayPathSegment::InsertAt(idx) => *idx,
        ArrayPathSegment::Push => items.len().saturating_sub(1),
    };
    let item = items
        .get(idx)
        .ok_or_else(|| HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![last.clone()])))?;

    let mut range = item.removal_range();
    let mut replacement = Vec::new();

    if idx + 1 == items.len() {
        if idx > 0
            && matches!(
                items[idx - 1].trailing_trivia.first(),
                Some(Trivia::Comma(_))
            )
        {
            range.start = items[idx - 1].post_node_index();
        }

        let prefix_len = item
            .trivia_boundary
            .leading_protected_prefix_len
            .min(item.leading_trivia.len());
        replacement.extend(ast_edit::render_trivia(&item.leading_trivia[..prefix_len]));
    }

    Ok(parent.green().splice_children(range, replacement))
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
    let detected = ast_edit::detected_string_style_of_node(node);
    let quote_style = resolve_quote_style(string_style, detected);
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

fn resolve_quote_style(preference: StringStyle, detected: Option<QuoteStyle>) -> QuoteStyle {
    match preference {
        StringStyle::Preserve => detected.unwrap_or(QuoteStyle::DoubleQuoted),
        StringStyle::DoubleQuoted => QuoteStyle::DoubleQuoted,
        StringStyle::Block => QuoteStyle::Block,
        StringStyle::Unquoted => QuoteStyle::Unquoted,
    }
}
