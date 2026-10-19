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
        style::detect_document_newline,
        value::{ends_with_unquoted_string, HjsonValue, RenderConfig},
        HjsonDocument, Prefer,
    },
    syntax::HjsonLanguage,
    syntax::{NewlineKind, RedNode, SyntaxKind},
    GreenElement,
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
    let config = RenderConfig {
        string_style: None,
        newline: detect_document_newline(&child),
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
        let replace_config = RenderConfig {
            string_style: None,
            newline: detect_document_newline(parent),
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
    let member_config = RenderConfig {
        string_style: insert_style,
        newline: detect_document_newline(parent),
    };

    let (before_colon_trivia, after_colon_trivia) = if let Some(first_member) = members.first() {
        (
            first_member.before_colon_trivia.clone(),
            first_member.after_colon_trivia.clone(),
        )
    } else {
        (vec![], vec![Trivia::Whitespace(" ".to_string())])
    };

    let member_node: GreenElement = value
        .render_member_node(
            key,
            None,
            &member_config,
            &before_colon_trivia,
            &after_colon_trivia,
        )
        .into();

    let last_member = members.last();
    let separator = append_separator(
        last_member.map(|member| member.trailing_trivia.as_slice()),
        last_member.is_some_and(|member| node_ends_with_unquoted_string(member.syntax())),
        detect_document_newline(parent),
    );

    if object_ast.delimiter_style == DelimiterStyle::Braced {
        let (insert_at, elements) = plan_braced_append(parent, insert_pos, separator, member_node);
        return Ok(parent
            .green()
            .splice_children(insert_at..insert_at, elements));
    }

    // ブレースレス: 閉じ区切りが無いので区切りとメンバーを末尾へ追加するだけでよい
    let mut inserted = separator;
    inserted.push(member_node);
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
                newline: detect_document_newline(parent),
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
                    newline: detect_document_newline(parent),
                };

                let value_node: GreenElement = value.render_value_node(&value_config).into();
                let last_item = items.last();
                let separator = append_separator(
                    last_item.map(|item| item.trailing_trivia.as_slice()),
                    last_item.is_some_and(|item| node_ends_with_unquoted_string(item.syntax())),
                    detect_document_newline(parent),
                );

                if array_ast.delimiter_style == DelimiterStyle::Braced {
                    let (insert_at, elements) =
                        plan_braced_append(parent, insert_pos, separator, value_node);
                    return Ok(parent
                        .green()
                        .splice_children(insert_at..insert_at, elements));
                }

                // 閉じ区切りを持たない配列（通常は発生しない）は末尾へ追加する
                let mut inserted = separator;
                inserted.push(value_node);
                Ok(parent
                    .green()
                    .splice_children(insert_pos..insert_pos, inserted))
            } else {
                let target_value = &items[*idx];
                let insert_pos = target_value.insert_before_index();
                let insert_config = RenderConfig {
                    string_style: insert_style,
                    newline: detect_document_newline(parent),
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
                let value_node: GreenElement = value.render_value_node(&insert_config).into();
                let ends_with_unquoted = ends_with_unquoted_string(&value_node);
                inserted.push(value_node);
                inserted.push(if ends_with_unquoted {
                    // 引用符なし文字列の直後のカンマは値に吸収される
                    newline_element(detect_document_newline(parent))
                } else {
                    SyntaxKind::Comma
                        .create_token(SyntaxKind::Comma.canonical_text())
                        .into()
                });

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
    config: &RenderConfig,
) -> GreenNode {
    let detected = detected_string_style_of_node(node);
    let quote_style = resolve_quote_style(string_style, detected);
    value.render_value_node(&RenderConfig {
        string_style: Some(quote_style),
        newline: config.newline,
    })
}

/// 赤ノードの要素（トリビアを値の前後へ移すために使う）。
type RedElement = rowan::SyntaxElement<HjsonLanguage>;

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
    let value_index = value_child.index();

    let is_empty_value = value_child.children_with_tokens().all(|child| match child {
        NodeOrToken::Node(node) => node.kind().is_trivia(),
        NodeOrToken::Token(token) => token.kind().is_trivia(),
    });

    if is_empty_value {
        // 値が空のとき（`key:` や `key: //c`）は、値の位置が行コメントの直後になる。
        // そのまま差し込むとコメントの内側に入って値が設定されないため、
        // コロンの直後に置き、トリビアは値の後ろへ回す。
        let elements: Vec<RedElement> = member.children_with_tokens().collect();
        let colon_index = elements
            .iter()
            .position(|element| {
                matches!(element, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Colon)
            })
            .ok_or(HjsonEditError::ParentNotFound)?;
        let between = &elements[colon_index + 1..value_index];
        let has_comment = between.iter().any(|element| {
            matches!(element, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment)
                || matches!(element, NodeOrToken::Token(token) if token.kind().is_comment())
        });

        if has_comment {
            let to_green = |element: &RedElement| -> GreenElement {
                match element {
                    NodeOrToken::Node(node) => node.green().clone().into(),
                    NodeOrToken::Token(token) => token.kind().create_token(token.text()).into(),
                }
            };

            let mut children: Vec<GreenElement> =
                elements[..=colon_index].iter().map(to_green).collect();
            let mut rest = between.iter().peekable();
            // コロン直後の空白は値の前の区切りとして残す
            if let Some(NodeOrToken::Token(token)) = rest.peek() {
                if token.kind() == SyntaxKind::Whitespace {
                    children.push(to_green(rest.next().unwrap()));
                }
            }
            children.push(updated_value.into());
            // 行コメントが値に吸収されないよう区切りを入れる
            children.push(SyntaxKind::Whitespace.create_token(" ").into());
            children.extend(rest.map(to_green));
            children.extend(elements[value_index + 1..].iter().map(to_green));
            return Ok(GreenNode::new(SyntaxKind::Member.into(), children));
        }
    }

    Ok(member
        .green()
        .splice_children(value_index..value_index + 1, vec![updated_value.into()]))
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

/// 波括弧コンテナの末尾へ要素を追加する挿入位置と挿入する要素を返す。
///
/// - 閉じ区切りが独立した行にあるときは、その行頭の改行の直前に `[改行, 要素]` を入れ、
///   閉じ区切りの行はそのまま残す。追加要素のインデントは整形層に任せる。
/// - 閉じ区切りが同一行にあるときは、要素が区切り文字で終端できるなら同じ行へ `separator`
///   と共に置き、終端できない（引用符なし文字列で終わる）なら前後に改行を補って独立した行にする。
pub(super) fn plan_braced_append(
    parent: &RedNode,
    insert_pos: usize,
    separator: Vec<GreenElement>,
    value: GreenElement,
) -> (usize, Vec<GreenElement>) {
    let children: Vec<_> = parent.children_with_tokens().collect();

    // 閉じ区切りの直前にある空白・改行・コメントの先頭
    let mut trivia_start = insert_pos;
    while trivia_start > 0 {
        let is_trivia = match &children[trivia_start - 1] {
            NodeOrToken::Token(token) => token.kind().is_trivia(),
            NodeOrToken::Node(node) => node.kind() == SyntaxKind::Comment,
        };
        if !is_trivia {
            break;
        }
        trivia_start -= 1;
    }

    // 閉じ区切りが独立した行にあるなら、その行頭の改行
    let line_break = (trivia_start..insert_pos)
        .rev()
        .find(|index| match &children[*index] {
            NodeOrToken::Token(token) => token.kind() == SyntaxKind::Newline,
            NodeOrToken::Node(_) => false,
        });

    if let Some(line_break) = line_break {
        let kind = match &children[line_break] {
            NodeOrToken::Token(token) => NewlineKind::from_text(token.text()).unwrap_or_default(),
            _ => NewlineKind::default(),
        };
        return (line_break, vec![newline_element(kind), value]);
    }

    if ends_with_unquoted_string(&value) {
        let kind = detect_document_newline(parent);
        return (
            trivia_start,
            vec![newline_element(kind), value, newline_element(kind)],
        );
    }

    let mut elements = separator;
    elements.push(value);
    (insert_pos, elements)
}

/// 末尾へ要素を追加するときの区切りを返す。
///
/// 直前の要素が無ければ区切りは不要。既にカンマか改行で区切られている場合と、
/// 直前の値が引用符なし文字列で終わる場合（カンマが文字列に吸収される）は改行を返す。
pub(super) fn append_separator(
    last_trailing_trivia: Option<&[Trivia]>,
    last_ends_with_unquoted_string: bool,
    newline: NewlineKind,
) -> Vec<GreenElement> {
    let Some(trailing_trivia) = last_trailing_trivia else {
        return Vec::new();
    };
    let already_separated = trailing_trivia
        .iter()
        .any(|trivia| matches!(trivia, Trivia::Comma(_) | Trivia::Newline { .. }));
    if already_separated || last_ends_with_unquoted_string {
        return vec![newline_element(newline)];
    }
    vec![
        SyntaxKind::Comma
            .create_token(SyntaxKind::Comma.canonical_text())
            .into(),
        SyntaxKind::Whitespace
            .create_token(SyntaxKind::Whitespace.canonical_text())
            .into(),
    ]
}

fn newline_element(kind: NewlineKind) -> GreenElement {
    SyntaxKind::Newline.create_token(kind.as_str()).into()
}

/// メンバーや配列項目の値の末尾が引用符なし文字列かどうか。
pub(super) fn node_ends_with_unquoted_string(node: &RedNode) -> bool {
    node.last_token()
        .is_some_and(|token| token.kind() == SyntaxKind::UnquotedString)
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
