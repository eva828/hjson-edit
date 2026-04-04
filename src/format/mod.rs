use crate::ast::{Array, Document, DocumentItem, Member, Object, Trivia, Value, ValueContent};

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
    pub fn format(document: &Document, config: FormatConfig) -> String {
        let mut out = String::new();
        render_document(document, 0, &config, &mut out);
        out
    }
}

fn render_document(document: &Document, depth: usize, config: &FormatConfig, out: &mut String) {
    render_trivia(&document.leading_trivia, depth, config, out);

    if let Some(item) = document.item() {
        render_item(item, depth, config, out);
    }

    render_trivia(&document.trailing_trivia, depth, config, out);
}

fn render_item(item: DocumentItem, depth: usize, config: &FormatConfig, out: &mut String) {
    match item {
        DocumentItem::Object(object) => render_object(&object, depth, config, out),
        DocumentItem::Array(array) => render_array(&array, depth, config, out),
        DocumentItem::Value(value) => render_value(&value, depth, config, out),
    }
}

fn render_object(object: &Object, depth: usize, config: &FormatConfig, out: &mut String) {
    render_trivia(&object.leading_trivia, depth, config, out);
    out.push('{');

    let members = object.members();
    if !members.is_empty() {
        for (idx, member) in members.iter().enumerate() {
            render_prefix(&member.leading_trivia, depth + 1, config, out);
            render_member(member, depth + 1, config, out);

            if idx + 1 == members.len() {
                render_closing_gap(&member.trailing_trivia, depth, config, out);
            }
        }
    }

    out.push('}');
}

fn render_array(array: &Array, depth: usize, config: &FormatConfig, out: &mut String) {
    render_trivia(&array.leading_trivia, depth, config, out);
    out.push('[');

    let items = array.items();
    if !items.is_empty() {
        for (idx, item) in items.iter().enumerate() {
            render_prefix(&item.leading_trivia, depth + 1, config, out);
            render_array_item(item, depth + 1, config, out);

            if idx + 1 == items.len() {
                render_closing_gap(&item.trailing_trivia, depth, config, out);
            }
        }
    }

    out.push(']');
}

fn render_array_item(value: &Value, depth: usize, config: &FormatConfig, out: &mut String) {
    render_value(value, depth, config, out);
}

fn render_member(member: &Member, depth: usize, config: &FormatConfig, out: &mut String) {
    if let Some(key) = member.key().and_then(|key| key.raw_text()) {
        out.push_str(&key);
    }
    out.push_str(": ");

    if let Some(value) = member.value() {
        render_value(&value, depth, config, out);
    }
}

fn render_value(value: &Value, depth: usize, config: &FormatConfig, out: &mut String) {
    match value.content() {
        Ok(ValueContent::Object(object)) => render_object(&object, depth, config, out),
        Ok(ValueContent::Array(array)) => render_array(&array, depth, config, out),
        Ok(ValueContent::Scalar(scalar)) => match scalar {
            crate::ast::ScalarValue::String(string) => out.push_str(string.raw_text()),
            crate::ast::ScalarValue::Integer { raw, .. } => out.push_str(&raw),
            crate::ast::ScalarValue::Float { raw, .. } => out.push_str(&raw),
            crate::ast::ScalarValue::Bool(true) => out.push_str("true"),
            crate::ast::ScalarValue::Bool(false) => out.push_str("false"),
            crate::ast::ScalarValue::Null => out.push_str("null"),
        },
        Err(_) => {}
    }
}

fn render_prefix(trivia: &[Trivia], depth: usize, config: &FormatConfig, out: &mut String) {
    if trivia.is_empty() {
        out.push_str(&config.newline);
        out.push_str(&config.indent_unit.repeat(depth));
        return;
    }

    render_trivia(trivia, depth, config, out);
    if !trivia
        .iter()
        .any(|item| matches!(item, Trivia::Newline { .. }))
    {
        out.push_str(&config.newline);
        out.push_str(&config.indent_unit.repeat(depth));
    }
}

fn render_closing_gap(trivia: &[Trivia], depth: usize, config: &FormatConfig, out: &mut String) {
    if trivia.is_empty() {
        out.push_str(&config.newline);
        out.push_str(&config.indent_unit.repeat(depth));
        return;
    }

    render_trivia(trivia, depth, config, out);
    if trivia
        .iter()
        .any(|item| matches!(item, Trivia::Newline { .. }))
    {
        out.push_str(&config.indent_unit.repeat(depth));
    } else {
        out.push_str(&config.newline);
        out.push_str(&config.indent_unit.repeat(depth));
    }
}

fn render_trivia(trivia: &[Trivia], depth: usize, config: &FormatConfig, out: &mut String) {
    for item in trivia {
        match item {
            Trivia::Whitespace(text) => out.push_str(text),
            Trivia::Newline { text, .. } => out.push_str(text),
            Trivia::Comma(_) => {}
            Trivia::Comment(comment) => {
                out.push_str(&config.indent_unit.repeat(depth));
                out.push_str(&comment.text);
            }
        }
    }
}
