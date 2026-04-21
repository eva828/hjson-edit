use rowan::NodeOrToken;
use rowan::{GreenNode, GreenToken};

use crate::{
    ast::{Array, AstNode, Document, Member, Object, QuoteStyle, Trivia, Value},
    high_level::error::HjsonEditError,
    syntax::SyntaxKind,
    GreenElement, RedNode, RedToken,
};

pub fn detected_string_style(document: &Document) -> Option<QuoteStyle> {
    document.detected_string_style()
}

pub fn detected_string_style_of_object(object: &Object) -> Option<QuoteStyle> {
    object.detected_string_style()
}

pub fn detected_string_style_of_array(array: &Array) -> Option<QuoteStyle> {
    array.detected_string_style()
}

pub fn detected_string_style_of_value(value: &Value) -> Option<QuoteStyle> {
    value.quote_style()
}

pub fn detected_string_style_of_node(node: &RedNode) -> Option<QuoteStyle> {
    match node.kind() {
        SyntaxKind::Root => Document::cast(node.clone())
            .ok()
            .and_then(|doc| doc.detected_string_style()),
        SyntaxKind::Object => Object::cast(node.clone())
            .ok()
            .and_then(|object| object.detected_string_style()),
        SyntaxKind::Array => Array::cast(node.clone())
            .ok()
            .and_then(|array| array.detected_string_style()),
        SyntaxKind::Value => Value::cast(node.clone())
            .ok()
            .and_then(|value| value.quote_style()),
        SyntaxKind::Pair => Member::cast(node.clone())
            .ok()
            .and_then(|member| member.detected_string_style()),
        _ => None,
    }
}

pub fn replace_member_value(
    member: &Member,
    new_value: GreenNode,
) -> Result<GreenNode, HjsonEditError> {
    let value_child = member
        .syntax()
        .children()
        .find(|child| child.kind() == SyntaxKind::Value)
        .ok_or(HjsonEditError::ParentNotFound)?;

    Ok(member.syntax().green().splice_children(
        value_child.index()..value_child.index() + 1,
        vec![new_value.into()],
    ))
}

pub fn render_trivia(trivia: &[Trivia]) -> Vec<GreenElement> {
    trivia
        .iter()
        .map(|item| match item {
            Trivia::Whitespace(text) => GreenToken::new(SyntaxKind::Whitespace.into(), text).into(),
            Trivia::Newline { kind: _, text } => {
                GreenToken::new(SyntaxKind::Newline.into(), text).into()
            }
            Trivia::Comment(comment) => {
                let kind = match comment.kind {
                    crate::ast::CommentKind::Line => SyntaxKind::LineComment,
                    crate::ast::CommentKind::Hash => SyntaxKind::HashComment,
                    crate::ast::CommentKind::Block => SyntaxKind::BlockComment,
                };
                GreenToken::new(kind.into(), &comment.text).into()
            }
            Trivia::Comma(text) => GreenToken::new(SyntaxKind::Comma.into(), text).into(),
        })
        .collect()
}

pub fn protected_trivia_prefix_len(trivia: &[Trivia]) -> usize {
    let mut boundary = 0usize;
    let mut index = 0usize;
    let mut saw_comment_since_boundary = false;

    while index < trivia.len() {
        match &trivia[index] {
            Trivia::Comment(_) => {
                saw_comment_since_boundary = true;
                index += 1;
            }
            Trivia::Newline { .. } => {
                let mut cursor = index;
                let mut newline_count = 0usize;

                while cursor < trivia.len() {
                    match &trivia[cursor] {
                        Trivia::Newline { .. } => {
                            newline_count += 1;
                            cursor += 1;
                        }
                        Trivia::Whitespace(_) => cursor += 1,
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

pub fn red_bracket_index(
    children: &[NodeOrToken<RedNode, RedToken>],
    kind: SyntaxKind,
) -> Option<usize> {
    children
        .iter()
        .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == kind))
}

pub fn copy_separator_tokens(
    separator_slice: &[NodeOrToken<RedNode, RedToken>],
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

pub fn clone_green_elements(slice: &[NodeOrToken<RedNode, RedToken>]) -> Vec<GreenElement> {
    slice
        .iter()
        .map(|child| match child {
            NodeOrToken::Node(node) => node.green().clone().into(),
            NodeOrToken::Token(token) => GreenToken::new(token.kind().into(), token.text()).into(),
        })
        .collect()
}

pub fn remove_structural_child(
    children: &[NodeOrToken<RedNode, RedToken>],
    target_pos: usize,
    structural_kind: SyntaxKind,
    opening_kind: SyntaxKind,
    closing_kind: SyntaxKind,
) -> Vec<GreenElement> {
    let mut remove = vec![false; children.len()];
    remove[target_pos] = true;

    let boundary = children[..target_pos]
        .iter()
        .rposition(|child| match child {
            NodeOrToken::Node(node) if node.kind() == structural_kind => true,
            NodeOrToken::Token(token) if token.kind() == opening_kind => true,
            _ => false,
        })
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let right_boundary = children[target_pos + 1..]
        .iter()
        .position(|child| {
            matches!(child, NodeOrToken::Node(node) if node.kind() == structural_kind)
                || matches!(child, NodeOrToken::Token(token) if token.kind() == closing_kind)
        })
        .map(|idx| target_pos + 1 + idx)
        .unwrap_or(children.len());

    let target_is_last = !children[target_pos + 1..right_boundary]
        .iter()
        .any(|child| matches!(child, NodeOrToken::Node(node) if node.kind() == structural_kind));

    let protected_prefix_len = protected_trivia_prefix_len_nodes(&children[boundary..target_pos]);
    let comment_start = boundary + protected_prefix_len;

    for idx in comment_start..target_pos {
        match &children[idx] {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => remove[idx] = true,
            NodeOrToken::Token(token) if token.kind().is_comment() => remove[idx] = true,
            _ => {}
        }
    }

    if target_is_last {
        if let Some(comma_pos) = children[boundary..target_pos].iter().rposition(
            |child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma),
        ) {
            remove[boundary + comma_pos] = true;
        }
    }

    let mut newline_run = 0usize;
    for idx in target_pos + 1..right_boundary {
        match &children[idx] {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                newline_run += 1;
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace => {}
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => {
                if newline_run < 2 {
                    remove[idx] = true;
                } else {
                    break;
                }
            }
            NodeOrToken::Token(token) if token.kind().is_comment() => {
                if newline_run < 2 {
                    remove[idx] = true;
                } else {
                    break;
                }
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma => {
                if newline_run < 2 {
                    remove[idx] = true;
                } else {
                    break;
                }
            }
            NodeOrToken::Node(node) if node.kind() == structural_kind => break,
            NodeOrToken::Token(token) if token.kind() == closing_kind => break,
            _ => {}
        }
    }

    children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            if remove[idx] {
                None
            } else {
                Some(match child {
                    NodeOrToken::Node(node) => node.green().clone().into(),
                    NodeOrToken::Token(token) => {
                        GreenToken::new(token.kind().into(), token.text()).into()
                    }
                })
            }
        })
        .collect()
}

pub fn protected_trivia_prefix_len_nodes(slice: &[NodeOrToken<RedNode, RedToken>]) -> usize {
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
