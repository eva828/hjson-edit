use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    ast::{
        edit as ast_edit, Array as AstArray, AstNode, Document as AstDocument, Object as AstObject,
        QuoteStyle, Trivia,
    },
    high_level::{
        error::HjsonEditError,
        ext::RedNodeExt,
        path::{ArrayPathSegment, HjsonObjectPath, ObjectPathSegment, PathSegment},
        style::DetectNewline,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppendMode {
    #[default]
    BeforeFooter,
    AfterComments,
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
        let root = self.current_root().clone().document_start_node();
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
        let append_mode = self.current_append_mode();
        let path_str = path.as_ref().to_string();
        let path = HjsonObjectPath::parse(path_str.clone())?;
        let value = value.into();

        if let Ok(current) = self.get(&path_str) {
            if current == value {
                return Ok(self);
            }
        }

        let root = self.current_root().clone();
        let string_style = self.current_string_style();
        let updated = if path.depth() == 0 {
            replace_root_value(&root, &value, string_style)?
        } else {
            rebuild_tree_with_set(root.clone(), &path, &value, string_style, append_mode)?
        };

        Ok(self.root(updated))
    }

    fn remove(self, path: impl AsRef<str>) -> Result<Self, HjsonEditError> {
        let path = HjsonObjectPath::parse(path.as_ref().to_string())?;
        let root = self.current_root().clone();
        let updated = rebuild_tree_with_remove(root.clone(), &path)?;
        Ok(self.root(updated))
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
    append_mode: AppendMode,
) -> Result<GreenNode, HjsonEditError> {
    let (parent_path, last) = path
        .split_last()
        .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;
    match resolve_parent(root.clone(), &parent_path) {
        Ok(parent) => {
            let new_parent = match parent.kind() {
                SyntaxKind::Root => {
                    set_root_child(&parent, &last, value, string_style, append_mode, false)?
                }
                SyntaxKind::Object => {
                    set_object_child(&parent, &last, value, string_style, append_mode, false)?
                }
                SyntaxKind::Array => {
                    set_array_child(&parent, &last, value, string_style, append_mode, false)?
                }
                SyntaxKind::Value => {
                    set_value_child(&parent, &last, value, string_style, append_mode, false)?
                }
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
                SyntaxKind::Root => set_root_child(
                    &ancestor,
                    missing,
                    &nested_value,
                    string_style,
                    append_mode,
                    !path.segments()[consumed + 1..].is_empty(),
                )?,
                SyntaxKind::Object => set_object_child(
                    &ancestor,
                    missing,
                    &nested_value,
                    string_style,
                    append_mode,
                    !path.segments()[consumed + 1..].is_empty(),
                )?,
                SyntaxKind::Array => set_array_child(
                    &ancestor,
                    missing,
                    &nested_value,
                    string_style,
                    append_mode,
                    !path.segments()[consumed + 1..].is_empty(),
                )?,
                SyntaxKind::Value => set_value_child(
                    &ancestor,
                    missing,
                    &nested_value,
                    string_style,
                    append_mode,
                    true,
                )?,
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
    append_mode: AppendMode,
    pretty_nested: bool,
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
    let updated_child = rebuild_child_with_set(
        &child,
        last,
        value,
        string_style,
        append_mode,
        pretty_nested,
    )?;

    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn set_value_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
    append_mode: AppendMode,
    pretty_nested: bool,
) -> Result<GreenNode, HjsonEditError> {
    let child = parent
        .children()
        .find(|child| !child.kind().is_trivia())
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_child = rebuild_child_with_set(
        &child,
        last,
        value,
        string_style,
        append_mode,
        pretty_nested,
    )?;

    Ok(parent
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
    append_mode: AppendMode,
    pretty_nested: bool,
) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Object(ObjectPathSegment::Key(key)) => key,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Member,
                found: SyntaxKind::Object,
            })
        }
    };

    let object_ast = AstObject::cast(parent.clone())?;
    let detected = object_ast.detected_string_style();
    let insert_style = match (string_style, detected) {
        (StringStyle::Preserve, None) => None,
        _ => Some(resolve_quote_style(string_style, detected)),
    };
    if let Some(member) = object_ast
        .members()
        .into_iter()
        .find(|member| member.member_key().as_deref() == Some(key.as_str()))
    {
        let member_node = member.syntax().clone();
        let updated = replace_member_value(&member_node, value, string_style)?;
        return Ok(parent.green().splice_children(
            member_node.index()..member_node.index() + 1,
            vec![updated.into()],
        ));
    }

    let insert_pos = match append_mode {
        AppendMode::BeforeFooter => object_ast.append_member_index_before_footer(),
        AppendMode::AfterComments => object_ast.append_member_index_after_comments(),
    };
    let mut inserted = Vec::new();
    let indent = "  ".repeat(object_ast.layout.depth + 1);
    let newline = parent.detect_newline().as_str().to_string();
    let rendered_member = if pretty_nested {
        value.render_member_pretty(key, None, insert_style, object_ast.layout.depth, &newline)
    } else {
        value.render_member(key, None, insert_style)
    };

    if object_ast.members().is_empty() {
        let range_start = if insert_pos > 0
            && matches!(
                parent.children_with_tokens().nth(insert_pos - 1),
                Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Whitespace
            ) {
            insert_pos - 1
        } else {
            insert_pos
        };
        inserted.push(SyntaxKind::Whitespace.create_token(&indent).into());
        inserted.push(rendered_member.into());
        inserted.push(SyntaxKind::Newline.create_token(&newline).into());
        if object_ast.layout.depth > 0 {
            inserted.push(
                SyntaxKind::Whitespace
                    .create_token(&"  ".repeat(object_ast.layout.depth))
                    .into(),
            );
        }
        return Ok(parent
            .green()
            .splice_children(range_start..insert_pos, inserted));
    }

    if pretty_nested {
        inserted.push(SyntaxKind::Newline.create_token(&newline).into());
        inserted.push(SyntaxKind::Whitespace.create_token(&indent).into());
        inserted.push(rendered_member.into());
        inserted.push(SyntaxKind::Newline.create_token(&newline).into());
        inserted.push(SyntaxKind::Whitespace.create_token(&indent).into());
        return Ok(parent
            .green()
            .splice_children(insert_pos..insert_pos, inserted));
    }

    if let Some(last_member) = object_ast.members().last() {
        let needs_separator = last_member
            .trailing_trivia
            .iter()
            .all(|trivia| !matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. }));
        if needs_separator {
            inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
            inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
        }
    }

    inserted.push(rendered_member.into());

    Ok(parent
        .green()
        .splice_children(insert_pos..insert_pos, inserted))
}

fn set_array_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
    append_mode: AppendMode,
    pretty_nested: bool,
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

    let array_ast = AstArray::cast(parent.clone())?;
    let detected = array_ast.detected_string_style();
    let insert_style = match (string_style, detected) {
        (StringStyle::Preserve, None) => None,
        _ => Some(resolve_quote_style(string_style, detected)),
    };
    let items = array_ast.items();

    match seg {
        ArrayPathSegment::Index(idx) => {
            let Some(current_value) = items.get(*idx) else {
                return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
                    last.clone(),
                ])));
            };
            let updated = render_value_like_node(current_value.syntax(), value, string_style);
            Ok(parent.green().splice_children(
                current_value.syntax().index()..current_value.syntax().index() + 1,
                vec![updated.into()],
            ))
        }
        ArrayPathSegment::InsertAt(idx) => {
            if *idx >= items.len() {
                let insert_pos = match append_mode {
                    AppendMode::BeforeFooter => array_ast.append_value_index_before_footer(),
                    AppendMode::AfterComments => array_ast.append_value_index_after_comments(),
                };
                let mut inserted = Vec::new();
                let indent = "  ".repeat(array_ast.layout.depth + 1);
                let newline = parent.detect_newline().as_str().to_string();
                let rendered_value = if pretty_nested {
                    value.render_value_pretty(insert_style, array_ast.layout.depth, &newline)
                } else {
                    value.render_value(insert_style)
                };

                if items.is_empty() {
                    let range_start = if insert_pos > 0
                        && matches!(
                            parent.children_with_tokens().nth(insert_pos - 1),
                            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Whitespace
                        ) {
                        insert_pos - 1
                    } else {
                        insert_pos
                    };
                    inserted.push(SyntaxKind::Whitespace.create_token(&indent).into());
                    inserted.push(rendered_value.into());
                    inserted.push(SyntaxKind::Newline.create_token(&newline).into());
                    return Ok(parent
                        .green()
                        .splice_children(range_start..insert_pos, inserted));
                }

                if pretty_nested {
                    inserted.push(SyntaxKind::Newline.create_token(&newline).into());
                    inserted.push(SyntaxKind::Whitespace.create_token(&indent).into());
                    inserted.push(rendered_value.into());
                    inserted.push(SyntaxKind::Newline.create_token(&newline).into());
                    inserted.push(SyntaxKind::Whitespace.create_token(&indent).into());
                    return Ok(parent
                        .green()
                        .splice_children(insert_pos..insert_pos, inserted));
                }

                if let Some(last_value) = items.last() {
                    let needs_separator = last_value
                        .trailing_trivia
                        .iter()
                        .all(|trivia| !matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. }));
                    if needs_separator {
                        inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                        inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                    }
                }
                inserted.push(rendered_value.into());

                Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted))
            } else {
                let target_value = &items[*idx];
                let insert_pos = target_value.insert_before_index();
                let inserted = vec![value.render_value(insert_style).into()];

                Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted))
            }
        }
        ArrayPathSegment::Push => {
            let insert_pos = match append_mode {
                AppendMode::BeforeFooter => array_ast.append_value_index_before_footer(),
                AppendMode::AfterComments => array_ast.append_value_index_after_comments(),
            };
            let mut inserted = Vec::new();

            if items.is_empty() {
                let newline = parent.detect_newline().as_str().to_string();
                inserted.push(SyntaxKind::Whitespace.create_token("  ").into());
                inserted.push(value.render_value(insert_style).into());
                inserted.push(SyntaxKind::Newline.create_token(&newline).into());
                return Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted));
            }

            if let Some(last_value) = items.last() {
                let needs_separator = last_value
                    .trailing_trivia
                    .iter()
                    .all(|trivia| !matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. }));
                if needs_separator {
                    inserted.push(GreenToken::new(SyntaxKind::Comma.into(), ",").into());
                    inserted.push(GreenToken::new(SyntaxKind::Whitespace.into(), " ").into());
                }
            }
            inserted.push(value.render_value(insert_style).into());

            Ok(parent
                .green()
                .splice_children(insert_pos..insert_pos, inserted))
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
    let member_pos = members
        .iter()
        .position(|member| member.member_key().as_deref() == Some(key.as_str()));

    let Some(member_pos) = member_pos else {
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::new(vec![
            last.clone(),
        ])));
    };

    let member = &members[member_pos];
    Ok(parent
        .green()
        .splice_children(member.span_range(), Vec::new()))
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

    Ok(parent
        .green()
        .splice_children(item.span_range(), Vec::new()))
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
    append_mode: AppendMode,
    pretty_nested: bool,
) -> Result<GreenNode, HjsonEditError> {
    match child.kind() {
        SyntaxKind::Object => {
            set_object_child(child, last, value, string_style, append_mode, pretty_nested)
        }
        SyntaxKind::Array => {
            set_array_child(child, last, value, string_style, append_mode, pretty_nested)
        }
        SyntaxKind::Value => {
            set_value_child(child, last, value, string_style, append_mode, pretty_nested)
        }
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

fn replace_member_value(
    member: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let value_child = member
        .children()
        .find(|child| child.kind() == SyntaxKind::Value)
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_value = render_value_like_node(&value_child, value, string_style);

    Ok(member.green().splice_children(
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
