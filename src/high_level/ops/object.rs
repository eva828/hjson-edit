use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    ast::{
        detected_string_style_of_node, Array as AstArray, AstNode,
        Object as AstObject, QuoteStyle, Trivia,
    },
    high_level::{
        error::HjsonEditError,
        ext::RedNodeExt,
        path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
        resolve::{ExistingPrefix, ParentResolution, PathResolver},
        value::{HjsonValue, RenderConfig},
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
    fn get(&self, path: HjsonObjectPath) -> Result<HjsonValue, HjsonEditError>;
    fn set(
        self,
        path: HjsonObjectPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: HjsonObjectPath) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(&self, path: HjsonObjectPath) -> Result<HjsonValue, HjsonEditError> {
        PathResolver::new(self.syntax_root().clone())
            .resolve_node(&path)?
            .node_to_value()
    }

    fn set(
        self,
        path: HjsonObjectPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let append_mode = self.current_append_mode();
        let value = value.into();
        let newline = self.current_newline();

        if let Ok(current) = self.get(path.clone()) {
            if current == value {
                return Ok(self);
            }
        }

        let root = self.syntax_root().clone();
        let string_style = self.current_string_style();
        let updated = if path.depth() == 0 {
            replace_root_value(&root, &value, string_style)?
        } else {
            rebuild_tree_with_set(
                root.clone(),
                &path,
                &value,
                string_style,
                append_mode,
                newline,
            )?
        };

        Ok(self.root(updated))
    }

    fn remove(self, path: HjsonObjectPath) -> Result<Self, HjsonEditError> {
        let root = self.syntax_root().clone();
        let updated = rebuild_tree_with_remove(root.clone(), &path)?;
        Ok(self.root(updated))
    }
}

pub(crate) fn replace_root_value(
    root: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let child = find_mutable_child(root)?;
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
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let resolver = PathResolver::new(root.clone());
    match resolver.resolve_parent(path) {
        Ok(ParentResolution { parent, last }) => {
            let pretty = false;
            let new_parent = apply_set_to_node(
                &parent,
                &last,
                value,
                string_style,
                append_mode,
                pretty,
                newline,
            )?;

            if parent.kind() == SyntaxKind::Root {
                Ok(new_parent)
            } else {
                Ok(parent.replace_with(new_parent))
            }
        }
        Err(HjsonEditError::PathNotFound(_)) => {
            let ExistingPrefix { ancestor, consumed } = resolver.resolve_existing_prefix(path);
            if consumed == 0 || consumed >= path.depth() {
                return Err(HjsonEditError::PathNotFound(path.clone()));
            }

            let missing = &path.segments()[consumed];
            let nested_value = build_nested_value(&path.segments()[consumed + 1..], value.clone());
            let pretty = !path.segments()[consumed + 1..].is_empty();
            let new_parent = apply_set_to_node(
                &ancestor,
                missing,
                &nested_value,
                string_style,
                append_mode,
                pretty,
                newline,
            )?;

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
    let ParentResolution { parent, last } = PathResolver::new(root.clone()).resolve_parent(path)?;
    let new_parent = apply_remove_to_node(&parent, &last)?;

    if parent.kind() == SyntaxKind::Root {
        Ok(new_parent)
    } else {
        Ok(parent.replace_with(new_parent))
    }
}

fn find_mutable_child(parent: &RedNode) -> Result<RedNode, HjsonEditError> {
    match parent.kind() {
        SyntaxKind::Root => parent
            .children()
            .find(|child| {
                matches!(
                    child.kind(),
                    SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value
                )
            })
            .ok_or(HjsonEditError::ParentNotFound),
        SyntaxKind::Value => parent
            .children()
            .find(|child| !child.kind().is_trivia())
            .ok_or(HjsonEditError::ParentNotFound),
        _ => Ok(parent.clone()),
    }
}

fn apply_set_to_node(
    node: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
    append_mode: AppendMode,
    pretty_nested: bool,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    match node.kind() {
        SyntaxKind::Root | SyntaxKind::Value => {
            let child = find_mutable_child(node)?;
            let updated_child = apply_set_to_node(
                &child,
                last,
                value,
                string_style,
                append_mode,
                pretty_nested,
                newline,
            )?;
            Ok(node
                .green()
                .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
        }
        SyntaxKind::Object => set_object_child(
            node,
            last,
            value,
            string_style,
            append_mode,
            pretty_nested,
            newline,
        ),
        SyntaxKind::Array => set_array_child(
            node,
            last,
            value,
            string_style,
            append_mode,
            pretty_nested,
            newline,
        ),
        _ => Err(HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![
            last.clone(),
        ]))),
    }
}

fn apply_remove_to_node(
    node: &RedNode,
    last: &PathSegment,
) -> Result<GreenNode, HjsonEditError> {
    match node.kind() {
        SyntaxKind::Root | SyntaxKind::Value => {
            let child = find_mutable_child(node)?;
            let updated_child = apply_remove_to_node(&child, last)?;
            Ok(node
                .green()
                .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
        }
        SyntaxKind::Object => remove_object_child(node, last),
        SyntaxKind::Array => remove_array_child(node, last),
        _ => Err(HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![
            last.clone(),
        ]))),
    }
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: StringStyle,
    append_mode: AppendMode,
    pretty_nested: bool,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Key(key) => key,
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
    let member_config = RenderConfig {
        string_style: insert_style,
        pretty: pretty_nested,
        depth: object_ast.layout.depth,
        newline,
    };

    let is_empty = object_ast.members().is_empty();
    if is_empty {
        let range_start = if insert_pos > 0
            && matches!(
                parent.children_with_tokens().nth(insert_pos - 1),
                Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Whitespace
            ) {
            insert_pos - 1
        } else {
            insert_pos
        };
        let inserted = value.render_object_member_elements(key, None, &member_config, true);
        return Ok(parent
            .green()
            .splice_children(range_start..insert_pos, inserted));
    }

    let mut inserted: Vec<crate::GreenElement> = Vec::new();
    if !pretty_nested {
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
    }
    inserted.extend(value.render_object_member_elements(
        key,
        None,
        &member_config,
        false,
    ));

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
    newline: &str,
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

    // Normalize Push to InsertAt at the end
    let seg = match seg {
        ArrayPathSegment::Push => &ArrayPathSegment::InsertAt(items.len()),
        other => other,
    };

    match seg {
        ArrayPathSegment::Index(idx) => {
            let Some(current_value) = items.get(*idx) else {
                return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![
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
                let inserted = build_append_array_elements(
                    parent,
                    &array_ast,
                    value,
                    insert_style,
                    pretty_nested,
                    newline,
                )?;

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
                    return Ok(parent
                        .green()
                        .splice_children(range_start..insert_pos, inserted));
                }

                Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted))
            } else {
                let target_value = &items[*idx];
                let insert_pos = target_value.insert_before_index();
                let inserted = vec![value.render_value_node(&RenderConfig {
                    string_style: insert_style,
                    ..RenderConfig::default()
                }).into()];

                Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted))
            }
        }
        ArrayPathSegment::Push => unreachable!("Push should have been normalized"),
    }
}

fn build_append_array_elements(
    _parent: &RedNode,
    array_ast: &AstArray,
    value: &HjsonValue,
    insert_style: Option<QuoteStyle>,
    pretty_nested: bool,
    newline: &str,
) -> Result<Vec<crate::GreenElement>, HjsonEditError> {
    let items = array_ast.items();
    let value_config = RenderConfig {
        string_style: insert_style,
        pretty: pretty_nested,
        depth: array_ast.layout.depth,
        newline,
    };

    let mut inserted = Vec::new();
    if !pretty_nested && !items.is_empty() {
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
    }
    inserted.extend(value.render_array_item_elements(
        &value_config,
        items.is_empty(),
    ));

    Ok(inserted)
}



fn remove_object_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Key(key) => key,
        _ => {
            return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![
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
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![
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
            return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![
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
        .ok_or_else(|| HjsonEditError::PathNotFound(HjsonObjectPath::from(vec![last.clone()])))?;

    Ok(parent
        .green()
        .splice_children(item.span_range(), Vec::new()))
}

fn build_nested_value(path: &[PathSegment], value: HjsonValue) -> HjsonValue {
    let mut current = value;
    for segment in path.iter().rev() {
        current = match segment {
            PathSegment::Key(key) => HjsonValue::Object(vec![(key.clone(), current)]),
            PathSegment::Array(ArrayPathSegment::Index(_))
            | PathSegment::Array(ArrayPathSegment::InsertAt(_))
            | PathSegment::Array(ArrayPathSegment::Push) => HjsonValue::Array(vec![current]),
        };
    }

    current
}



fn render_value_like_node(
    node: &RedNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> GreenNode {
    let detected = detected_string_style_of_node(node);
    let quote_style = resolve_quote_style(string_style, detected);
    value.render_value_node(&RenderConfig {
        string_style: Some(quote_style),
        ..RenderConfig::default()
    })
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
