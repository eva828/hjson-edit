use rowan::{GreenNode, NodeOrToken};

use crate::{
    ast::{
        detected_string_style_of_node, Array as AstArray, AstNode, DelimiterStyle,
        Object as AstObject, QuoteStyle, Trivia,
    },
    high_level::{
        error::HjsonEditError,
        ops::{container_depth_of_value, ensure_depth_within_limit, receiving_container_depth},
        path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
        resolve::{resolve_body, ExistingPrefix, ParentResolution, PathResolver},
        value::{HjsonValue, RenderConfig},
        HjsonDocument, Prefer,
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
        let resolved = PathResolver::new(self.syntax_root().clone()).resolve_node(&path)?;
        HjsonValue::from_node(&resolved)
    }

    fn set(
        self,
        path: HjsonObjectPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let append_mode = self.current_append_mode();
        let value = value.into();

        let root = self.syntax_root().clone();
        let string_style = self.current_string_style();
        let updated = if path.depth() == 0 {
            replace_root_value(&root, &value, string_style)?
        } else {
            rebuild_tree_with_set(root.clone(), &path, &value, string_style, append_mode)?
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
) -> Result<GreenNode, HjsonEditError> {
    ensure_depth_within_limit(container_depth_of_value(value))?;

    let child = resolve_body(root)?;
    let config = RenderConfig { string_style: None };
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
) -> Result<GreenNode, HjsonEditError> {
    let resolver = PathResolver::new(root.clone());
    match resolver.resolve_parent(path) {
        Ok(ParentResolution { parent, last }) => {
            ensure_depth_within_limit(
                receiving_container_depth(&parent) + container_depth_of_value(value),
            )?;

            let new_parent = apply_set_to_node(&parent, &last, value, string_style, append_mode)?;

            if parent.kind() == SyntaxKind::Root {
                Ok(new_parent)
            } else {
                Ok(parent.replace_with(new_parent))
            }
        }
        Err(HjsonEditError::PathNotFound(_)) => {
            let ExistingPrefix { ancestor, consumed } = resolver.resolve_existing_prefix(path);
            if consumed == 0 {
                return Err(HjsonEditError::PathNotFound(path.clone()));
            }

            let missing = &path.segments()[consumed];

            // 深さの検査は値の構築前に行う。深い値を作ってから捨てると、
            // HjsonValue の再帰的な Drop でスタックを使い切るため。
            // `missing` は受け取り先のコンテナ内のキーなので、新しく増える
            // コンテナは残りのセグメント分だけ。
            let created = path.depth() - consumed - 1;
            let carried = container_depth_of_value(value);
            ensure_depth_within_limit(receiving_container_depth(&ancestor) + created + carried)?;

            let nested_value = build_nested_value(&path.segments()[consumed + 1..], value.clone());
            let new_parent =
                apply_set_to_node(&ancestor, missing, &nested_value, string_style, append_mode)?;

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
        // resolve_parent は Object/Array/Value/Root を返すはずだが、
        // 万一それ以外のノードが返った場合も安全のためフォールバックとする
        _ => Err(HjsonEditError::PathNotFound(
            HjsonObjectPath::from_segments(vec![last.clone()]),
        )),
    }
}

fn apply_set_to_node(
    node: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    append_mode: AppendMode,
) -> Result<GreenNode, HjsonEditError> {
    transform_node(
        node,
        last,
        |node, last| set_object_child(node, last, value, string_style, append_mode),
        |node, last| set_array_child(node, last, value, string_style, append_mode),
    )
}

fn apply_remove_to_node(node: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    transform_node(node, last, remove_object_child, remove_array_child)
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    append_mode: AppendMode,
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
    let members = object_ast.members();

    if let Some(member) = members
        .iter()
        .find(|member| member.member_key().as_deref() == Some(key.as_str()))
    {
        let member_node = member.syntax().clone();
        let replace_config = RenderConfig { string_style: None };
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
    let member_config = RenderConfig {
        string_style: insert_style,
    };

    let is_empty = members.is_empty();
    let (before_colon_trivia, after_colon_trivia) = if let Some(first_member) = members.first() {
        (
            first_member.before_colon_trivia.clone(),
            first_member.after_colon_trivia.clone(),
        )
    } else {
        (vec![], vec![Trivia::Whitespace(" ".to_string())])
    };
    if is_empty {
        let range_start = resolve_insert_range_start(parent, insert_pos);
        let inserted = vec![value
            .render_member_node(
                key,
                None,
                &member_config,
                &before_colon_trivia,
                &after_colon_trivia,
            )
            .into()];
        return Ok(parent
            .green()
            .splice_children(range_start..insert_pos, inserted));
    }

    let mut inserted: Vec<crate::GreenElement> = Vec::new();
    if let Some(last_member) = members.last() {
        let needs_separator = last_member
            .trailing_trivia
            .iter()
            .all(|trivia| !matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. }));
        if needs_separator {
            inserted.push(
                SyntaxKind::Comma
                    .create_token(SyntaxKind::Comma.canonical_text())
                    .into(),
            );
            inserted.push(
                SyntaxKind::Whitespace
                    .create_token(SyntaxKind::Whitespace.canonical_text())
                    .into(),
            );
        } else {
            let kind = last_member
                .trailing_trivia
                .iter()
                .find_map(|t| t.newline_kind())
                .unwrap_or_default();
            inserted.push(SyntaxKind::Newline.create_token(kind.as_str()).into());
        }
    }
    inserted.push(
        value
            .render_member_node(
                key,
                None,
                &member_config,
                &before_colon_trivia,
                &after_colon_trivia,
            )
            .into(),
    );

    let splice_start = if object_ast.delimiter_style == DelimiterStyle::Braced {
        let mut removable_trailing = 0usize;
        let mut found_non_trivial_in_container = false;
        for triv in object_ast.trailing_trivia.iter().rev() {
            if matches!(triv, Trivia::Whitespace(_) | Trivia::Newline { .. }) {
                removable_trailing += 1;
            } else {
                found_non_trivial_in_container = true;
                break;
            }
        }
        if !found_non_trivial_in_container {
            if let Some(last_member) = members.last() {
                for triv in last_member.trailing_trivia.iter().rev() {
                    if matches!(triv, Trivia::Whitespace(_) | Trivia::Newline { .. }) {
                        removable_trailing += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        insert_pos.saturating_sub(removable_trailing)
    } else {
        insert_pos
    };

    Ok(parent
        .green()
        .splice_children(splice_start..insert_pos, inserted))
}

fn set_array_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    append_mode: AppendMode,
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
    let push_target = ArrayPathSegment::InsertAt(items.len());
    let seg = match seg {
        ArrayPathSegment::Push => &push_target,
        other => other,
    };

    match seg {
        ArrayPathSegment::Index(idx) => {
            let Some(current_value) = items.get(*idx) else {
                return Err(HjsonEditError::PathNotFound(
                    HjsonObjectPath::from_segments(vec![last.clone()]),
                ));
            };
            let value_config = RenderConfig {
                string_style: insert_style,
            };
            let updated =
                render_value_like_node(current_value.syntax(), value, string_style, &value_config);
            Ok(parent.green().splice_children(
                current_value.syntax().index()..current_value.syntax().index() + 1,
                vec![updated.into()],
            ))
        }
        ArrayPathSegment::InsertAt(idx) => {
            if *idx > items.len() {
                return Err(HjsonEditError::ArrayIndexOutOfBounds {
                    index: *idx,
                    len: items.len(),
                });
            }
            if *idx >= items.len() {
                let insert_pos = match append_mode {
                    AppendMode::BeforeFooter => array_ast.append_value_index_before_footer(),
                    AppendMode::AfterComments => array_ast.append_value_index_after_comments(),
                };
                let value_config = RenderConfig {
                    string_style: insert_style,
                };

                let mut inserted = Vec::new();
                if !items.is_empty() {
                    if let Some(last_value) = items.last() {
                        let needs_separator = last_value.trailing_trivia.iter().all(|trivia| {
                            !matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. })
                        });
                        if needs_separator {
                            inserted.push(
                                SyntaxKind::Comma
                                    .create_token(SyntaxKind::Comma.canonical_text())
                                    .into(),
                            );
                            inserted.push(
                                SyntaxKind::Whitespace
                                    .create_token(SyntaxKind::Whitespace.canonical_text())
                                    .into(),
                            );
                        } else {
                            let kind = last_value
                                .trailing_trivia
                                .iter()
                                .find_map(|t| t.newline_kind())
                                .unwrap_or_default();
                            inserted.push(SyntaxKind::Newline.create_token(kind.as_str()).into());
                        }
                    }
                }
                inserted.push(value.render_value_node(&value_config).into());

                if items.is_empty() {
                    let range_start = resolve_insert_range_start(parent, insert_pos);
                    return Ok(parent
                        .green()
                        .splice_children(range_start..insert_pos, inserted));
                }

                let splice_start = if array_ast.delimiter_style == DelimiterStyle::Braced {
                    let mut removable_trailing = 0usize;
                    let mut found_non_trivial_in_container = false;
                    for triv in array_ast.trailing_trivia.iter().rev() {
                        if matches!(triv, Trivia::Whitespace(_) | Trivia::Newline { .. }) {
                            removable_trailing += 1;
                        } else {
                            found_non_trivial_in_container = true;
                            break;
                        }
                    }
                    if !found_non_trivial_in_container {
                        if let Some(last_item) = items.last() {
                            for triv in last_item.trailing_trivia.iter().rev() {
                                if matches!(triv, Trivia::Whitespace(_) | Trivia::Newline { .. }) {
                                    removable_trailing += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                    insert_pos.saturating_sub(removable_trailing)
                } else {
                    insert_pos
                };

                Ok(parent
                    .green()
                    .splice_children(splice_start..insert_pos, inserted))
            } else {
                let target_value = &items[*idx];
                let insert_pos = target_value.insert_before_index();
                let insert_config = RenderConfig {
                    string_style: insert_style,
                };
                let mut inserted: Vec<crate::GreenElement> = Vec::new();
                if *idx > 0 {
                    if let Some(prev_value) = items.get(*idx - 1) {
                        let needs_separator = prev_value.trailing_trivia.iter().all(|trivia| {
                            !matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. })
                        });
                        if needs_separator {
                            inserted.push(
                                SyntaxKind::Comma
                                    .create_token(SyntaxKind::Comma.canonical_text())
                                    .into(),
                            );
                            inserted.push(
                                SyntaxKind::Whitespace
                                    .create_token(SyntaxKind::Whitespace.canonical_text())
                                    .into(),
                            );
                        }
                    }
                }
                inserted.push(value.render_value_node(&insert_config).into());
                inserted.push(
                    SyntaxKind::Comma
                        .create_token(SyntaxKind::Comma.canonical_text())
                        .into(),
                );

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
            return Err(HjsonEditError::PathNotFound(
                HjsonObjectPath::from_segments(vec![last.clone()]),
            ))
        }
    };

    let object_ast = AstObject::cast(parent.clone())?;
    let members = object_ast.members();
    let member_pos = members
        .iter()
        .position(|member| member.member_key().as_deref() == Some(key.as_str()));

    let Some(member_pos) = member_pos else {
        return Err(HjsonEditError::PathNotFound(
            HjsonObjectPath::from_segments(vec![last.clone()]),
        ));
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
            return Err(HjsonEditError::PathNotFound(
                HjsonObjectPath::from_segments(vec![last.clone()]),
            ))
        }
    };

    let array_ast = AstArray::cast(parent.clone())?;
    let items = array_ast.items();
    let idx = match seg {
        ArrayPathSegment::Index(idx) | ArrayPathSegment::InsertAt(idx) => *idx,
        ArrayPathSegment::Push => items.len().saturating_sub(1),
    };
    let item = items.get(idx).ok_or_else(|| {
        HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![last.clone()]))
    })?;

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
    _config: &RenderConfig,
) -> GreenNode {
    let detected = detected_string_style_of_node(node);
    let quote_style = resolve_quote_style(string_style, detected);
    value.render_value_node(&RenderConfig {
        string_style: Some(quote_style),
    })
}

fn replace_member_value(
    member: &RedNode,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    config: &RenderConfig,
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
    let mut start = insert_pos;
    while start > 0 {
        let Some(elem) = parent.children_with_tokens().nth(start - 1) else {
            break;
        };
        match &elem {
            NodeOrToken::Token(token)
                if matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Newline) =>
            {
                start -= 1;
            }
            _ => break,
        }
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::QuoteStyle;

    #[test]
    fn test_resolve_quote_style() {
        assert_eq!(
            resolve_quote_style(Prefer::Preserve, Some(QuoteStyle::DoubleQuoted)),
            QuoteStyle::DoubleQuoted
        );
        assert_eq!(
            resolve_quote_style(Prefer::Preserve, None),
            QuoteStyle::DoubleQuoted
        );
        assert_eq!(
            resolve_quote_style(Prefer::Explicit(QuoteStyle::Unquoted), None),
            QuoteStyle::Unquoted
        );
    }

    #[test]
    fn test_resolve_insert_style() {
        assert_eq!(
            resolve_insert_style(Prefer::Preserve, Some(QuoteStyle::DoubleQuoted)),
            Some(QuoteStyle::DoubleQuoted)
        );
        assert_eq!(resolve_insert_style(Prefer::Preserve, None), None);
        assert_eq!(
            resolve_insert_style(
                Prefer::Explicit(QuoteStyle::Unquoted),
                Some(QuoteStyle::DoubleQuoted)
            ),
            Some(QuoteStyle::Unquoted)
        );
        assert_eq!(
            resolve_insert_style(Prefer::Explicit(QuoteStyle::Unquoted), None),
            Some(QuoteStyle::Unquoted)
        );
    }

    #[test]
    fn test_build_nested_value_single_key_wraps_in_object() {
        let path = vec![PathSegment::Key("a".to_string())];
        let result = build_nested_value(&path, HjsonValue::Integer(1));
        assert_eq!(
            result,
            HjsonValue::Object(vec![("a".into(), HjsonValue::Integer(1))])
        );
    }

    #[test]
    fn test_build_nested_value_nested_keys() {
        let path = vec![
            PathSegment::Key("a".to_string()),
            PathSegment::Key("b".to_string()),
        ];
        let result = build_nested_value(&path, HjsonValue::Integer(1));
        assert_eq!(
            result,
            HjsonValue::Object(vec![(
                "a".into(),
                HjsonValue::Object(vec![("b".into(), HjsonValue::Integer(1))])
            )])
        );
    }

    #[test]
    fn test_build_nested_value_array_segment() {
        let path = vec![PathSegment::Array(ArrayPathSegment::Index(0))];
        let result = build_nested_value(&path, HjsonValue::Integer(1));
        assert_eq!(result, HjsonValue::Array(vec![HjsonValue::Integer(1)]));
    }
}
