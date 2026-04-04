use rowan::{GreenNode, GreenNodeBuilder};

use crate::ast::{Array, Document, DocumentItem, Member, Object, Trivia, Value, ValueContent};
use crate::syntax::SyntaxKind;

#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub(crate) indent_unit: String,
    pub(crate) newline: String,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent_unit: "  ".to_string(),
            newline: "\n".to_string(),
        }
    }
}

impl FormatConfig {
    pub fn with_newline(mut self, newline: impl Into<String>) -> Self {
        self.newline = newline.into();
        self
    }
}

pub struct Formatter;

impl Formatter {
    pub fn build(document: &Document, config: FormatConfig) -> GreenNode {
        let mut builder = GreenNodeBuilder::new();
        build_document(&mut builder, document, &config);
        builder.finish()
    }
}

fn build_document(
    builder: &mut GreenNodeBuilder<'static>,
    document: &Document,
    config: &FormatConfig,
) {
    builder.start_node(SyntaxKind::Root.into());
    write_comments(builder, &document.leading_trivia, 0, config);

    if let Some(item) = document.item() {
        build_item(builder, item, config, 0);
    }

    write_comments(builder, &document.trailing_trivia, 0, config);
    builder.finish_node();
}

fn build_item(
    builder: &mut GreenNodeBuilder<'static>,
    item: DocumentItem,
    config: &FormatConfig,
    depth: usize,
) {
    match item {
        DocumentItem::Object(object) => build_object(builder, &object, config, depth),
        DocumentItem::Array(array) => build_array(builder, &array, config, depth),
        DocumentItem::Value(value) => build_value(builder, &value, config, depth),
    }
}

fn build_object(
    builder: &mut GreenNodeBuilder<'static>,
    object: &Object,
    config: &FormatConfig,
    depth: usize,
) {
    builder.start_node(SyntaxKind::Object.into());
    write_comments(builder, &object.leading_trivia, depth, config);
    if object.braced {
        builder.token(SyntaxKind::LBrace.into(), "{");
    }

    let members = object.members();
    if !members.is_empty() {
        for member in &members {
            emit_newline(builder, config);
            build_member(builder, member, config, depth + 1);
        }
        emit_newline(builder, config);
        emit_indent(builder, depth, config);
    }

    write_inline_trivia(builder, &object.trailing_trivia, config);
    if object.braced {
        builder.token(SyntaxKind::RBrace.into(), "}");
    }
    builder.finish_node();
}

fn build_array(
    builder: &mut GreenNodeBuilder<'static>,
    array: &Array,
    config: &FormatConfig,
    depth: usize,
) {
    builder.start_node(SyntaxKind::Array.into());
    write_comments(builder, &array.leading_trivia, depth, config);
    if array.braced {
        builder.token(SyntaxKind::LBracket.into(), "[");
    }

    let items = array.items();
    if !items.is_empty() {
        for value in &items {
            emit_newline(builder, config);
            build_value(builder, value, config, depth + 1);
        }
        emit_newline(builder, config);
        emit_indent(builder, depth, config);
    }

    write_inline_trivia(builder, &array.trailing_trivia, config);
    if array.braced {
        builder.token(SyntaxKind::RBracket.into(), "]");
    }
    builder.finish_node();
}

fn build_member(
    builder: &mut GreenNodeBuilder<'static>,
    member: &Member,
    config: &FormatConfig,
    depth: usize,
) {
    builder.start_node(SyntaxKind::Pair.into());
    write_comments(builder, &member.leading_trivia, depth, config);
    emit_indent(builder, depth, config);

    builder.start_node(SyntaxKind::Key.into());
    if let Some((kind, text)) = member.key().and_then(|key| key.token_text_and_kind()) {
        builder.token(kind.into(), &text);
    }
    builder.finish_node();

    write_inline_comments(builder, &member.before_colon_trivia, config);
    builder.token(SyntaxKind::Colon.into(), ":");
    if !member.after_colon_trivia.is_empty() {
        for item in &member.after_colon_trivia {
            match item {
                Trivia::Whitespace(text) => builder.token(SyntaxKind::Whitespace.into(), text),
                Trivia::Comment(comment) => {
                    builder.token(SyntaxKind::Whitespace.into(), " ");
                    builder.token(comment_token_kind(comment).into(), &comment.text);
                }
                Trivia::Newline { text, .. } => builder.token(SyntaxKind::Newline.into(), text),
                Trivia::Comma(text) => builder.token(SyntaxKind::Comma.into(), text),
            }
        }
    } else {
        builder.token(SyntaxKind::Whitespace.into(), " ");
    }

    if let Some(value) = member.value() {
        build_value(builder, &value, config, depth);
    }

    write_inline_trivia(builder, &member.trailing_trivia, config);
    builder.finish_node();
}

fn build_value(
    builder: &mut GreenNodeBuilder<'static>,
    value: &Value,
    config: &FormatConfig,
    depth: usize,
) {
    builder.start_node(SyntaxKind::Value.into());
    write_comments(builder, &value.leading_trivia, depth, config);

    match value.content() {
        Ok(ValueContent::Object(object)) => build_object(builder, &object, config, depth),
        Ok(ValueContent::Array(array)) => build_array(builder, &array, config, depth),
        Ok(ValueContent::Scalar(scalar)) => match scalar {
            crate::ast::ScalarValue::String(string) => builder.token(
                token_kind_for_quote(string.quote_style()).into(),
                string.raw_text(),
            ),
            crate::ast::ScalarValue::Integer { raw, .. } => {
                builder.token(SyntaxKind::Integer.into(), &raw)
            }
            crate::ast::ScalarValue::Float { raw, .. } => {
                builder.token(SyntaxKind::Float.into(), &raw)
            }
            crate::ast::ScalarValue::Bool(true) => builder.token(SyntaxKind::True.into(), "true"),
            crate::ast::ScalarValue::Bool(false) => {
                builder.token(SyntaxKind::False.into(), "false")
            }
            crate::ast::ScalarValue::Null => builder.token(SyntaxKind::Null.into(), "null"),
        },
        Err(_) => {}
    }

    write_inline_trivia(builder, &value.trailing_trivia, config);
    builder.finish_node();
}

fn token_kind_for_quote(style: crate::ast::QuoteStyle) -> SyntaxKind {
    match style {
        crate::ast::QuoteStyle::DoubleQuoted => SyntaxKind::String,
        crate::ast::QuoteStyle::Block => SyntaxKind::BlockString,
        crate::ast::QuoteStyle::Unquoted => SyntaxKind::UnquotedString,
    }
}

fn write_comments(
    builder: &mut GreenNodeBuilder<'static>,
    trivia: &[Trivia],
    depth: usize,
    config: &FormatConfig,
) {
    let mut first = true;
    for item in trivia {
        if let Trivia::Comment(comment) = item {
            if !first {
                emit_newline(builder, config);
            }
            emit_indent(builder, depth, config);
            builder.token(comment_token_kind(comment).into(), &comment.text);
            first = false;
        }
    }
    if !first {
        emit_newline(builder, config);
    }
}

fn write_inline_trivia(
    builder: &mut GreenNodeBuilder<'static>,
    trivia: &[Trivia],
    config: &FormatConfig,
) {
    for item in trivia {
        match item {
            Trivia::Comma(text) => builder.token(SyntaxKind::Comma.into(), text),
            Trivia::Comment(comment) => {
                builder.token(SyntaxKind::Whitespace.into(), " ");
                builder.token(comment_token_kind(comment).into(), &comment.text);
            }
            Trivia::Whitespace(_) | Trivia::Newline { .. } => {}
        }
    }
    let _ = config;
}

fn write_inline_comments(
    builder: &mut GreenNodeBuilder<'static>,
    trivia: &[Trivia],
    config: &FormatConfig,
) {
    for item in trivia {
        if let Trivia::Comment(comment) = item {
            builder.token(SyntaxKind::Whitespace.into(), " ");
            builder.token(comment_token_kind(comment).into(), &comment.text);
            if !comment.text.ends_with(&config.newline) {
                builder.token(SyntaxKind::Whitespace.into(), " ");
            }
        }
    }
}

fn emit_newline(builder: &mut GreenNodeBuilder<'static>, config: &FormatConfig) {
    builder.token(SyntaxKind::Newline.into(), &config.newline);
}

fn emit_indent(builder: &mut GreenNodeBuilder<'static>, depth: usize, config: &FormatConfig) {
    builder.token(
        SyntaxKind::Whitespace.into(),
        &config.indent_unit.repeat(depth),
    );
}

fn comment_token_kind(comment: &crate::ast::CommentTrivia) -> SyntaxKind {
    match comment.kind {
        crate::ast::CommentKind::Line => SyntaxKind::LineComment,
        crate::ast::CommentKind::Hash => SyntaxKind::HashComment,
        crate::ast::CommentKind::Block => SyntaxKind::BlockComment,
    }
}
