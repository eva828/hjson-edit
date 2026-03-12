use crate::edit::api::{create_node, create_token};
use crate::edit::high_level::navigation::RedNodeNavigateExt;
use crate::edit::high_level::{
    clone_element, HjsonDocument, HjsonEditError, HjsonValue, PathSegment, StringStyle,
};
use crate::format::FormatTarget;
use crate::syntax::green::GreenElement;
use crate::syntax::kind::{HjsonLanguage, SyntaxKind};
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::{GreenNode, Language, NodeOrToken};

pub(crate) fn value_needs_contextual_formatting(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> bool {
    match value {
        HjsonValue::Object(_) | HjsonValue::Array(_) => true,
        HjsonValue::String(text) => {
            resolve_string_style(text, string_style, existing_style) == StringStyle::Block
        }
        _ => false,
    }
}

pub(crate) fn format_target_for_value(
    document: &HjsonDocument,
    path: &[PathSegment],
    prefer_container: bool,
) -> Option<FormatTarget> {
    let target_node = document.navigate_to_node_internal(path)?;
    let target_node = if prefer_container {
        target_node
            .resolve_object_child()
            .or_else(|| target_node.resolve_array_child())
            .unwrap_or(target_node)
    } else {
        target_node
    };
    Some(FormatTarget::node(target_node.path_from_root()))
}

pub(crate) fn node_to_value(node: &RedNode) -> Option<HjsonValue> {
    let target: RedNode = if node.kind() == SyntaxKind::Value {
        if let Some(child) = node.children_nodes().next() {
            child
        } else {
            return token_to_value(node);
        }
    } else {
        node.clone()
    };

    match target.kind() {
        SyntaxKind::Object => {
            let mut pairs = Vec::new();
            for child in target.children_nodes() {
                if child.kind() == SyntaxKind::Pair {
                    if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                        if let Some(key_text) = extract_key_text(&key_node) {
                            if let Some(val_node) = child.child_by_kind(SyntaxKind::Value) {
                                if let Some(val) = node_to_value(&val_node) {
                                    pairs.push((key_text, val));
                                }
                            }
                        }
                    }
                }
            }
            Some(HjsonValue::Object(pairs))
        }
        SyntaxKind::Array => {
            let mut items = Vec::new();
            for child in target.children_nodes() {
                if child.kind() == SyntaxKind::Value {
                    if let Some(val) = node_to_value(&child) {
                        items.push(val);
                    }
                }
            }
            Some(HjsonValue::Array(items))
        }
        _ => token_to_value(&target),
    }
}

pub(crate) fn token_to_value(node: &RedNode) -> Option<HjsonValue> {
    for elem in node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            let text = token.text();

            if matches!(
                kind,
                SyntaxKind::Whitespace
                    | SyntaxKind::Newline
                    | SyntaxKind::LineComment
                    | SyntaxKind::HashComment
                    | SyntaxKind::BlockComment
            ) {
                continue;
            }

            return match kind {
                SyntaxKind::String => {
    let unquoted = strip_surrounding(text, 1);
    let unescaped = unescape_json_string(&unquoted);
    Some(HjsonValue::String(unescaped))
}
                SyntaxKind::BlockString => {
                    Some(HjsonValue::String(parse_multiline_string(text, 0)))
                }
                SyntaxKind::UnquotedString | SyntaxKind::Ident => {
                    Some(HjsonValue::String(text.to_string()))
                }
                SyntaxKind::Integer => text.parse::<i64>().ok().map(HjsonValue::Integer),
                SyntaxKind::Float => text.parse::<f64>().ok().map(HjsonValue::Float),
                SyntaxKind::True => Some(HjsonValue::Bool(true)),
                SyntaxKind::False => Some(HjsonValue::Bool(false)),
                SyntaxKind::Null => Some(HjsonValue::Null),
                _ => None,
            };
        }
    }
    None
}

pub(crate) fn strip_surrounding(text: &str, delimiter_len: usize) -> String {
    let min_len = delimiter_len * 2;
    if text.len() >= min_len {
        text[delimiter_len..text.len() - delimiter_len].to_string()
    } else {
        text.to_string()
    }
}

/// JSONエスケープシーケンスを実際の文字に変換
/// 
/// RFC 8259準拠のエスケープ処理:
/// - \\ -> \
/// - \" -> "
/// - \/ -> /
/// - \b -> backspace (0x08)
/// - \f -> form feed (0x0C)
/// - \n -> newline
/// - \r -> carriage return
/// - \t -> tab
/// - \uXXXX -> Unicode文字（サロゲートペア対応）
pub(crate) fn unescape_json_string(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('/') => result.push('/'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('u') => {
                    // 4桁の16進数を読み取る
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() == 4 {
                        if let Ok(codepoint) = u16::from_str_radix(&hex, 16) {
                            // サロゲートペアの処理
                            let high_surrogate_start: u16 = 0xD800;
                            let high_surrogate_end: u16 = 0xDBFF;
                            let low_surrogate_start: u16 = 0xDC00;
                            let low_surrogate_end: u16 = 0xDFFF;

                            if (high_surrogate_start..=high_surrogate_end).contains(&codepoint) {
                                // 上位サロゲートを検出、次の\u低位サロゲートを確認
                                if chars.peek() == Some(&'\\') {
                                    chars.next(); // 消費 '\'
                                    if chars.peek() == Some(&'u') {
                                        chars.next(); // 消費 'u'
                                        let low_hex: String = chars.by_ref().take(4).collect();
                                        if low_hex.len() == 4 {
                                            if let Ok(low_codepoint) = u16::from_str_radix(&low_hex, 16) {
                                                if (low_surrogate_start..=low_surrogate_end).contains(&low_codepoint) {
                                                    // サロゲートペアを結合
                                                    let high = codepoint as u32;
                                                    let low = low_codepoint as u32;
                                                    let combined = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                                                    if let Some(ch) = char::from_u32(combined) {
                                                        result.push(ch);
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                        // 不完全なサロゲートペア - 上位サロゲートのみ出力
                                        result.push_str("\\u");
                                        result.push_str(&hex);
                                        // 読み取った文字を戻して再処理
                                        result.push('\\');
                                        result.push('u');
                                        result.push_str(&low_hex);
                                    } else {
                                        // 不完全なサロゲートペア
                                        result.push_str("\\u");
                                        result.push_str(&hex);
                                        result.push('\\');
                                        if let Some(ch) = chars.peek() {
                                            result.push(*ch);
                                        }
                                    }
                                } else {
                    // 孤立した上位サロゲート
                                    result.push_str("\\u");
                                    result.push_str(&hex);
                                }
                            } else if (low_surrogate_start..=low_surrogate_end).contains(&codepoint) {
                                // 孤立した低位サロゲート
                                result.push_str("\\u");
                                result.push_str(&hex);
                            } else if let Some(ch) = char::from_u32(codepoint as u32) {
                                result.push(ch);
                            } else {
                                // 無効なUnicodeコードポイント
                                result.push_str("\\u");
                                result.push_str(&hex);
                            }
                        } else {
                            // 無効な16進数
                            result.push_str("\\u");
                            result.push_str(&hex);
                        }
                    } else {
                        // 不完全なUnicodeエスケープ
                        result.push_str("\\u");
                        result.push_str(&hex);
                    }
                }
                Some(other) => {
                    // 不明なエスケープシーケンス - 元のテキストを保持
                    result.push('\\');
                    result.push(other);
                }
                None => {
                    // 末尾のバックスラッシュ
                    result.push('\\');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub(crate) fn parse_multiline_string(text: &str, head_column: usize) -> String {
    let normalized = strip_surrounding(text, 3)
        .replace("\r\n", "\n")
        .replace('\r', "");
    let body = normalized.strip_prefix('\n').unwrap_or(&normalized);
    let mut lines: Vec<&str> = body.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }

    let inferred_column = if lines
        .last()
        .map(|line| line.trim().is_empty())
        .unwrap_or(false)
    {
        let closing_indent = lines.pop().unwrap_or_default();
        closing_indent.chars().count()
    } else {
        head_column
    };

    let stripped = lines
        .into_iter()
        .map(|line| strip_indent_to_column(line, inferred_column))
        .collect::<Vec<_>>();

    stripped.join("\n")
}

pub(crate) fn render_multiline_string(value: &str) -> String {
    create_multiline_string_literal(value, "", "\n")
}

pub(crate) fn strip_indent_to_column(line: &str, target_column: usize) -> String {
    let mut consumed = 0usize;
    let mut byte_idx = 0usize;
    for (idx, ch) in line.char_indices() {
        if consumed >= target_column {
            break;
        }
        match ch {
            ' ' | '\t' => {
                consumed += 1;
                byte_idx = idx + ch.len_utf8();
            }
            _ => break,
        }
    }
    line[byte_idx..].to_string()
}

pub(crate) fn is_hjson_number(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let bytes = value.as_bytes();
    let mut idx = 0usize;
    if bytes[idx] == b'-' {
        idx += 1;
        if idx == bytes.len() {
            return false;
        }
    }

    if bytes[idx] == b'0' {
        idx += 1;
        if idx < bytes.len() && bytes[idx].is_ascii_digit() {
            return false;
        }
    } else if bytes[idx].is_ascii_digit() {
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
    } else {
        return false;
    }

    if idx < bytes.len() && bytes[idx] == b'.' {
        idx += 1;
        let frac_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if frac_start == idx {
            return false;
        }
    }

    if idx < bytes.len() && matches!(bytes[idx], b'e' | b'E') {
        idx += 1;
        if idx < bytes.len() && matches!(bytes[idx], b'+' | b'-') {
            idx += 1;
        }
        let exp_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if exp_start == idx {
            return false;
        }
    }

    idx == bytes.len()
}

pub(crate) fn string_style_for_red_value_node(node: &RedNode) -> Option<StringStyle> {
    for elem in node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            if matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline) {
                continue;
            }
            return match kind {
                SyntaxKind::String => Some(StringStyle::DoubleQuoted),
                SyntaxKind::BlockString => Some(StringStyle::Block),
                SyntaxKind::UnquotedString => Some(StringStyle::Unquoted),
                _ => None,
            };
        }
    }
    None
}

pub(crate) fn string_style_for_value_node(node: &GreenElement) -> Option<StringStyle> {
    match node {
        NodeOrToken::Node(green) => {
            let red = RedNode::new_root(green.clone());
            string_style_for_red_value_node(&red)
        }
        NodeOrToken::Token(token) => match HjsonLanguage::kind_from_raw(token.kind()) {
            SyntaxKind::String => Some(StringStyle::DoubleQuoted),
            SyntaxKind::BlockString => Some(StringStyle::Block),
            SyntaxKind::UnquotedString => Some(StringStyle::Unquoted),
            _ => None,
        },
    }
}

pub(crate) fn resolve_string_style(
    value: &str,
    requested_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> StringStyle {
    match requested_style {
        StringStyle::Preserve => existing_style.unwrap_or_else(|| auto_string_style(value)),
        StringStyle::Auto => auto_string_style(value),
        explicit => explicit,
    }
}

pub(crate) fn auto_string_style(value: &str) -> StringStyle {
    if value.contains('\n') || value.contains('\r') {
        return StringStyle::Block;
    }

    if value.starts_with('[') || value.starts_with('{') {
        return StringStyle::DoubleQuoted;
    }

    if is_safe_unquoted_string(value) {
        StringStyle::Unquoted
    } else {
        StringStyle::DoubleQuoted
    }
}

pub(crate) fn is_safe_unquoted_string(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    if matches!(value, "true" | "false" | "null") {
        return false;
    }

    if is_hjson_number(value) {
        return false;
    }

    if value.starts_with("//") || value.starts_with("/*") || value.starts_with('#') {
        return false;
    }

    if matches!(
        value.chars().next(),
        Some('{') | Some('}') | Some('[') | Some(']') | Some(',') | Some(':')
    ) {
        return false;
    }

    !value
        .chars()
        .any(|ch| matches!(ch, '\n' | '\r' | '\u{0000}'..='\u{001F}'))
}

pub(crate) fn create_string_token(value: &str, style: StringStyle) -> rowan::GreenToken {
    match style {
        StringStyle::DoubleQuoted | StringStyle::Preserve | StringStyle::Auto => create_token(
            SyntaxKind::String,
            format!("\"{}\"", escape_double_quoted(value)),
        ),
        StringStyle::Block => create_token(SyntaxKind::BlockString, render_multiline_string(value)),
        StringStyle::Unquoted => create_token(SyntaxKind::UnquotedString, value),
    }
}

pub(crate) fn escape_double_quoted(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            c if c.is_control() || (c as u32) < 0x20 => {
                // 制御文字は\uXXXX形式でエスケープ
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

pub(crate) fn value_to_green_element(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenElement, HjsonEditError> {
    match value {
        HjsonValue::String(s) => {
            let token =
                create_string_token(s, resolve_string_style(s, string_style, existing_style));
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Integer(n) => {
            let token = create_token(SyntaxKind::Integer, n.to_string());
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Float(n) => {
            let token = create_token(SyntaxKind::Float, format!("{}", n));
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Bool(true) => {
            let token = create_token(SyntaxKind::True, "true");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Bool(false) => {
            let token = create_token(SyntaxKind::False, "false");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Null => {
            let token = create_token(SyntaxKind::Null, "null");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Array(items) => {
            let mut children = vec![NodeOrToken::Token(create_token(SyntaxKind::LBracket, "["))];

            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")));
                }
                let val_node = create_value_node(item, string_style, None)?;
                children.push(NodeOrToken::Node(val_node));
            }

            children.push(NodeOrToken::Token(create_token(SyntaxKind::RBracket, "]")));

            let node = create_node(SyntaxKind::Array, children);
            Ok(NodeOrToken::Node(node))
        }
        HjsonValue::Object(pairs) => {
            let mut children = vec![NodeOrToken::Token(create_token(SyntaxKind::LBrace, "{"))];

            for (i, (key, val)) in pairs.iter().enumerate() {
                if i > 0 {
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")));
                }
                let pair_node = create_pair_node(key, val, string_style, None)?;
                children.push(NodeOrToken::Node(pair_node));
            }

            children.push(NodeOrToken::Token(create_token(SyntaxKind::RBrace, "}")));

            let node = create_node(SyntaxKind::Object, children);
            Ok(NodeOrToken::Node(node))
        }
    }
}

pub(crate) fn value_to_green_node(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    match value_to_green_element(value, string_style, existing_style)? {
        NodeOrToken::Node(n) => Ok(n),
        NodeOrToken::Token(t) => Ok(create_node(SyntaxKind::Value, vec![NodeOrToken::Token(t)])),
    }
}

pub(crate) fn create_value_node(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    let inner = value_to_green_element(value, string_style, existing_style)?;
    Ok(create_node(SyntaxKind::Value, vec![inner]))
}

pub(crate) fn replace_value_node(
    _existing_value_node: &GreenNode,
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    create_value_node(value, string_style, existing_style)
}

pub(crate) fn create_pair_node(
    key: &str,
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();

    let key_token = if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        create_token(SyntaxKind::Ident, key)
    } else {
        create_token(
            SyntaxKind::String,
            format!("\"{}\"", escape_double_quoted(key)),
        )
    };
    let key_node = create_node(SyntaxKind::Key, vec![NodeOrToken::Token(key_token)]);
    children.push(NodeOrToken::Node(key_node));

    children.push(NodeOrToken::Token(create_token(SyntaxKind::Colon, ":")));
    children.push(NodeOrToken::Token(create_token(
        SyntaxKind::Whitespace,
        " ",
    )));

    let val_node = create_value_node(value, string_style, existing_style)?;
    children.push(NodeOrToken::Node(val_node));

    Ok(create_node(SyntaxKind::Pair, children))
}

pub(crate) fn create_multiline_string_literal(value: &str, indent: &str, newline: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let escaped = normalized.replace("'''", "''\\'");
    let mut rendered = String::from("'''");
    rendered.push_str(newline);
    for line in escaped.split('\n') {
        rendered.push_str(indent);
        rendered.push_str(line);
        rendered.push_str(newline);
    }
    rendered.push_str(indent);
    rendered.push_str("'''");
    rendered
}

pub(crate) fn extract_key_text(key_node: &RedNode) -> Option<String> {
    for elem in key_node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            match kind {
                SyntaxKind::Ident | SyntaxKind::UnquotedString => {
                    return Some(token.text().to_string());
                }
                SyntaxKind::String => {
                    let text = token.text();
                    if text.len() >= 2 {
                        let unquoted = &text[1..text.len() - 1];
                        return Some(unescape_json_string(unquoted));
                    }
                    return Some(text.to_string());
                }
                _ => {}
            }
        }
    }
    None
}

pub(crate) fn replace_pair_value(
    pair_green: &GreenNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let pair_red = RedNode::new_root(pair_green.clone());
    let existing_value_red = pair_red.child_by_kind(SyntaxKind::Value);
    let existing_style = existing_value_red
        .as_ref()
        .and_then(string_style_for_red_value_node);
    let mut replaced = false;
    let mut children = Vec::new();

    for child in pair_green.children() {
        let cloned = clone_element(child);
        match &cloned {
            NodeOrToken::Node(n) if HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Value => {
                children.push(NodeOrToken::Node(replace_value_node(
                    n,
                    value,
                    string_style,
                    existing_style,
                )?));
                replaced = true;
            }
            NodeOrToken::Token(t)
                if HjsonLanguage::kind_from_raw(t.kind()) == SyntaxKind::Colon =>
            {
                children.push(cloned.clone());
                // We need to check if there is a space *after* the colon.
                let mut found_space_after_colon = false;
                let mut after_colon = false;
                for c in pair_green.children() {
                    if let NodeOrToken::Token(tok) = c {
                        let k = HjsonLanguage::kind_from_raw(tok.kind());
                        if k == SyntaxKind::Colon {
                            after_colon = true;
                        } else if after_colon && k == SyntaxKind::Whitespace {
                            found_space_after_colon = true;
                            break;
                        } else if after_colon && k == SyntaxKind::Value {
                            break; // Stop looking when we reach the value
                        }
                    } else if after_colon {
                        // Found a node after colon (presumably Value)
                        break;
                    }
                }
                if !found_space_after_colon {
                    children.push(NodeOrToken::Token(create_token(
                        SyntaxKind::Whitespace,
                        " ",
                    )));
                }
            }
            _ => children.push(cloned),
        }
    }

    if !replaced {
        return create_pair_node(
            &extract_key_text(
                &RedNode::new_root(pair_green.clone())
                    .child_by_kind(SyntaxKind::Key)
                    .ok_or(HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Key,
                        found: SyntaxKind::Pair,
                    })?,
            )
            .ok_or_else(|| HjsonEditError::InvalidPath {
                path: "pair key".to_string(),
            })?,
            value,
            string_style,
            existing_style,
        );
    }

    Ok(create_node(SyntaxKind::Pair, children))
}
