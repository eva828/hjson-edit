use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{
    ast::{
        detected_string_style_of_node, Array as AstArray, AstNode, DelimiterStyle,
        Object as AstObject, QuoteStyle, Trivia,
    },
    high_level::{
        error::HjsonEditError,
        path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
        resolve::{resolve_body, ExistingPrefix, ParentResolution, PathResolver},
        value::{HjsonValue, RenderConfig},
        Prefer, HjsonDocument,
    },
    syntax::{RedNode, SyntaxKind},
};

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
        let resolved = PathResolver::new(self.syntax_root().clone())
            .resolve_node(&path)?;
        HjsonValue::from_node(&resolved)
    }

    fn set(
        self,
        path: HjsonObjectPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let append_mode = self.current_append_mode();
        let value = value.into();
        let newline = self.current_newline();

        let root = self.syntax_root().clone();
        let string_style = self.current_string_style();
        let updated = if path.depth() == 0 {
            replace_root_value(&root, &value, string_style, newline)?
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
    string_style: Prefer<QuoteStyle>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let child = resolve_body(root)?;
    let config = RenderConfig {
        string_style: None,
        pretty: false,
        depth: 1,
        newline,
    };
    let updated_child = render_value_like_node(&child, value, string_style, &config);

    Ok(root
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

pub(crate) fn rebuild_tree_with_set(
    root: RedNode,
    path: &HjsonObjectPath,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
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

fn transform_node(
    node: &RedNode,
    last: &PathSegment,
    on_object: impl FnOnce(&RedNode, &PathSegment) -> Result<GreenNode, HjsonEditError>,
    on_array: impl FnOnce(&RedNode, &PathSegment) -> Result<GreenNode, HjsonEditError>,
) -> Result<GreenNode, HjsonEditError> {
    match node.kind() {
        SyntaxKind::Root | SyntaxKind::Value => {
            let child = resolve_body(node)?;
            let updated_child = transform_node(&child, last, on_object, on_array)?;
            Ok(node
                .green()
                .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
        }
        SyntaxKind::Object => on_object(node, last),
        SyntaxKind::Array => on_array(node, last),
        // resolve_parent の結果は常に Object/Array/Value/Root のいずれかなので到達しない
        _ => Err(HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![
            last.clone(),
        ]))),
    }
}

fn apply_set_to_node(
    node: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    append_mode: AppendMode,
    pretty_nested: bool,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    transform_node(
        node,
        last,
        |node, last| {
            set_object_child(
                node,
                last,
                value,
                string_style,
                append_mode,
                pretty_nested,
                newline,
            )
        },
        |node, last| {
            set_array_child(
                node,
                last,
                value,
                string_style,
                append_mode,
                pretty_nested,
                newline,
            )
        },
    )
}

fn apply_remove_to_node(
    node: &RedNode,
    last: &PathSegment,
) -> Result<GreenNode, HjsonEditError> {
    transform_node(node, last, remove_object_child, remove_array_child)
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
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
    let insert_style = resolve_insert_style(string_style, object_ast.detected_string_style());
    if let Some(member) = object_ast
        .members()
        .into_iter()
        .find(|member| member.member_key().as_deref() == Some(key.as_str()))
    {
        let member_node = member.syntax().clone();
        let replace_config = RenderConfig {
            string_style: None,
            pretty: false,
            depth: object_ast.layout.depth,
            newline,
        };
        let updated = replace_member_value(&member_node, value, string_style, &replace_config)?;
        return Ok(parent.green().splice_children(
            member_node.index()..member_node.index() + 1,
            vec![updated.into()],
        ));
    }

    let insert_pos = match append_mode {
        AppendMode::BeforeFooter => object_ast.append_member_index_before_footer(),
        AppendMode::AfterComments => object_ast.append_member_index_after_comments(),
    };
    let is_root_braceless =
        object_ast.delimiter_style == DelimiterStyle::Braceless && object_ast.layout.depth == 1;
    let member_config = RenderConfig {
        string_style: insert_style,
        pretty: pretty_nested,
        depth: if is_root_braceless { object_ast.layout.depth.saturating_sub(1) } else { object_ast.layout.depth },
        newline,
    };

    let is_empty = object_ast.members().is_empty();
    if is_empty {
        let range_start = resolve_insert_range_start(parent, insert_pos);
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
    string_style: Prefer<QuoteStyle>,
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
    let insert_style = resolve_insert_style(string_style, array_ast.detected_string_style());
    let items = array_ast.items();

    // Normalize Push to InsertAt at the end
    let seg = match seg {
        ArrayPathSegment::Push => &ArrayPathSegment::InsertAt(items.len()),
        other => other,
    };

    match seg {
        ArrayPathSegment::Index(idx) => {
            let Some(current_value) = items.get(*idx) else {
                return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![
                    last.clone(),
                ])));
            };
                let value_config = RenderConfig {
                    string_style: insert_style,
                    pretty: pretty_nested,
                    depth: array_ast.layout.depth,
                    newline,
                };
                let updated = render_value_like_node(current_value.syntax(), value, string_style, &value_config);
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
                inserted.extend(value.render_array_item_elements(&value_config, items.is_empty()));

                if items.is_empty() {
                    let range_start = resolve_insert_range_start(parent, insert_pos);
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
                let insert_config = RenderConfig {
                    string_style: insert_style,
                    pretty: pretty_nested,
                    depth: array_ast.layout.depth,
                    newline,
                };
                let inserted = vec![value.render_value_node(&insert_config).into()];

                Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted))
            }
        }
        ArrayPathSegment::Push => unreachable!("Push should have been normalized"),
    }
}

fn remove_object_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Key(key) => key,
        _ => {
            return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![
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
        return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![
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
            return Err(HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![
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
        .ok_or_else(|| HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![last.clone()])))?;

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
    string_style: Prefer<QuoteStyle>,
    config: &RenderConfig<'_>,
) -> GreenNode {
    let detected = detected_string_style_of_node(node);
    let quote_style = resolve_quote_style(string_style, detected);
    value.render_value_node(&RenderConfig {
        string_style: Some(quote_style),
        ..*config
    })
}

fn replace_member_value(
    member: &RedNode,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    config: &RenderConfig<'_>,
) -> Result<GreenNode, HjsonEditError> {
    let value_child = member
        .children()
        .find(|child| child.kind() == SyntaxKind::Value)
        .ok_or(HjsonEditError::ParentNotFound)?;
    let updated_value = render_value_like_node(&value_child, value, string_style, config);

    Ok(member.green().splice_children(
        value_child.index()..value_child.index() + 1,
        vec![updated_value.into()],
    ))
}

fn resolve_quote_style(preference: Prefer<QuoteStyle>, detected: Option<QuoteStyle>) -> QuoteStyle {
    match preference {
        Prefer::Preserve => detected.unwrap_or(QuoteStyle::DoubleQuoted),
        Prefer::Explicit(style) => style,
    }
}

fn resolve_insert_style(
    string_style: Prefer<QuoteStyle>,
    detected: Option<QuoteStyle>,
) -> Option<QuoteStyle> {
    match (string_style, detected) {
        (Prefer::Preserve, None) => None,
        _ => Some(resolve_quote_style(string_style, detected)),
    }
}

fn resolve_insert_range_start(parent: &RedNode, insert_pos: usize) -> usize {
    if insert_pos > 0
        && matches!(
            parent.children_with_tokens().nth(insert_pos - 1),
            Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Whitespace
        )
    {
        insert_pos - 1
    } else {
        insert_pos
    }
}
