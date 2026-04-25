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
        SyntaxKind::Member => Member::cast(node.clone())
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

pub fn clone_green_elements(slice: &[NodeOrToken<RedNode, RedToken>]) -> Vec<GreenElement> {
    slice
        .iter()
        .map(|child| match child {
            NodeOrToken::Node(node) => node.green().clone().into(),
            NodeOrToken::Token(token) => GreenToken::new(token.kind().into(), token.text()).into(),
        })
        .collect()
}

