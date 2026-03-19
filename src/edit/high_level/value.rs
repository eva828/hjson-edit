use crate::edit::high_level::navigation::RedNodeNavigateExt;
use crate::edit::high_level::{
    clone_element, HjsonDocument, HjsonEditError, HjsonValue, PathSegment, StringStyle,
};
use crate::number::HjsonNumber;
use crate::syntax::ext::GreenElement;
use crate::syntax::ext::{RedNode, RedNodeExt};
use crate::syntax::kind::{HjsonLanguage, SyntaxKind};
use rowan::{GreenNode, Language, NodeOrToken};
use std::iter::Peekable;
use std::str::Chars;

pub(crate) struct ValueCodec;

impl ValueCodec {
    pub(crate) fn needs_contextual_formatting(
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> bool {
        match value {
            HjsonValue::Object(_) | HjsonValue::Array(_) => true,
            HjsonValue::String(text) => {
                StringStyleResolver::resolve(text, string_style, existing_style)
                    == StringStyle::Block
            }
            _ => false,
        }
    }

    // pub(crate) fn format_target_for_value(
    //     document: &HjsonDocument,
    //     path: &[PathSegment],
    //     prefer_container: bool,
    // ) -> Option<FormatTargetUnit> {
    //     let target_node = document.navigate_to_node_internal(path)?;
    //     let target_node = if prefer_container {
    //         target_node
    //             .resolve_object_child()
    //             .or_else(|| target_node.resolve_array_child())
    //             .unwrap_or(target_node)
    //     } else {
    //         target_node
    //     };
    //     Some(FormatTargetUnit::node(target_node.path_from_root()))
    // }

    pub(crate) fn node_to_value(node: &RedNode) -> Option<HjsonValue> {
        let target = Self::value_target(node)?;
        match target.kind() {
            SyntaxKind::Object => Self::object_to_value(&target),
            SyntaxKind::Array => Self::array_to_value(&target),
            _ => Self::token_to_value(&target),
        }
    }

    fn value_target(node: &RedNode) -> Option<RedNode> {
        if node.kind() != SyntaxKind::Value {
            return Some(node.clone());
        }

        node.children()
            .next()
            .or_else(|| Self::token_to_value(node).map(|_| node.clone()))
    }

    fn object_to_value(node: &RedNode) -> Option<HjsonValue> {
        let mut pairs = Vec::new();
        for child in node.children() {
            if child.kind() != SyntaxKind::Pair {
                continue;
            }
            let key_node = child.child_by_kind(SyntaxKind::Key)?;
            let value_node = child.child_by_kind(SyntaxKind::Value)?;
            let key_text = KeyText::extract(&key_node)?;
            let value = Self::node_to_value(&value_node)?;
            pairs.push((key_text, value));
        }
        Some(HjsonValue::Object(pairs))
    }

    fn array_to_value(node: &RedNode) -> Option<HjsonValue> {
        let mut items = Vec::new();
        for child in node.children() {
            if child.kind() != SyntaxKind::Value {
                continue;
            }
            items.push(Self::node_to_value(&child)?);
        }
        Some(HjsonValue::Array(items))
    }

    pub(crate) fn token_to_value(node: &RedNode) -> Option<HjsonValue> {
        for elem in node.children_with_tokens() {
            let Some(token) = elem.as_token() else {
                continue;
            };

            let kind = token.kind();
            if Self::is_skippable_token(kind) {
                continue;
            }

            return match kind {
                SyntaxKind::String => Some(HjsonValue::String(
                    StringLiteralCodec::unescape_json_string(
                        &StringLiteralCodec::strip_surrounding(token.text(), 1),
                    ),
                )),
                SyntaxKind::BlockString => Some(HjsonValue::String(
                    StringLiteralCodec::parse_multiline_string(token.text(), 0),
                )),
                SyntaxKind::UnquotedString => Some(HjsonValue::String(token.text().to_string())),
                SyntaxKind::Integer => token.text().parse::<i64>().ok().map(HjsonValue::Integer),
                SyntaxKind::Float => token.text().parse::<f64>().ok().map(HjsonValue::Float),
                SyntaxKind::True => Some(HjsonValue::Bool(true)),
                SyntaxKind::False => Some(HjsonValue::Bool(false)),
                SyntaxKind::Null => Some(HjsonValue::Null),
                _ => None,
            };
        }
        None
    }

    fn is_skippable_token(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::LineComment
                | SyntaxKind::HashComment
                | SyntaxKind::BlockComment
        )
    }

    pub(crate) fn value_to_green_element(
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> Result<GreenElement, HjsonEditError> {
        match value {
            HjsonValue::String(text) => Ok(NodeOrToken::Token(Self::create_string_token(
                text,
                StringStyleResolver::resolve(text, string_style, existing_style),
            ))),
            HjsonValue::Integer(value) => Ok(NodeOrToken::Token(
                SyntaxKind::Integer.create_token(value.to_string()),
            )),
            HjsonValue::Float(value) => Ok(NodeOrToken::Token(
                SyntaxKind::Float.create_token(format!("{value}")),
            )),
            HjsonValue::Bool(true) => Ok(NodeOrToken::Token(SyntaxKind::True.create_token("true"))),
            HjsonValue::Bool(false) => {
                Ok(NodeOrToken::Token(SyntaxKind::False.create_token("false")))
            }
            HjsonValue::Null => Ok(NodeOrToken::Token(SyntaxKind::Null.create_token("null"))),
            HjsonValue::Array(items) => Self::array_to_green_element(items, string_style),
            HjsonValue::Object(pairs) => Self::object_to_green_element(pairs, string_style),
        }
    }

    fn array_to_green_element(
        items: &[HjsonValue],
        string_style: StringStyle,
    ) -> Result<GreenElement, HjsonEditError> {
        let mut children = vec![NodeOrToken::Token(SyntaxKind::LBracket.create_token("["))];
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                children.push(NodeOrToken::Token(SyntaxKind::Comma.create_token(",")));
            }
            children.push(NodeOrToken::Node(Self::create_value_node(
                item,
                string_style,
                None,
            )?));
        }
        children.push(NodeOrToken::Token(SyntaxKind::RBracket.create_token("]")));
        Ok(NodeOrToken::Node(SyntaxKind::Array.create_node(children)))
    }

    fn object_to_green_element(
        pairs: &[(String, HjsonValue)],
        string_style: StringStyle,
    ) -> Result<GreenElement, HjsonEditError> {
        let mut children = vec![NodeOrToken::Token(SyntaxKind::LBrace.create_token("{"))];
        for (index, (key, value)) in pairs.iter().enumerate() {
            if index > 0 {
                children.push(NodeOrToken::Token(SyntaxKind::Comma.create_token(",")));
            }
            children.push(NodeOrToken::Node(Self::create_pair_node(
                key,
                value,
                string_style,
                None,
            )?));
        }
        children.push(NodeOrToken::Token(SyntaxKind::RBrace.create_token("}")));
        Ok(NodeOrToken::Node(SyntaxKind::Object.create_node(children)))
    }

    pub(crate) fn value_to_green_node(
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> Result<GreenNode, HjsonEditError> {
        match Self::value_to_green_element(value, string_style, existing_style)? {
            NodeOrToken::Node(node) => Ok(node),
            NodeOrToken::Token(token) => {
                Ok(SyntaxKind::Value.create_node(vec![NodeOrToken::Token(token)]))
            }
        }
    }

    pub(crate) fn create_value_node(
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> Result<GreenNode, HjsonEditError> {
        Ok(
            SyntaxKind::Value.create_node(vec![Self::value_to_green_element(
                value,
                string_style,
                existing_style,
            )?]),
        )
    }

    fn replace_value_node(
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> Result<GreenNode, HjsonEditError> {
        Self::create_value_node(value, string_style, existing_style)
    }

    pub(crate) fn create_pair_node(
        key: &str,
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> Result<GreenNode, HjsonEditError> {
        // According to RFC, keynames may be any sequence of non-punctuator
        // characters (i.e. excluding whitespace and the structural
        // punctuators {},:[]). Allow a wider set of characters for
        // unquoted keys while keeping a few conservative exclusions
        // (control characters, quotes, and sequences that would be
        // interpreted as comments by the lexer).
        fn is_unquoted_key(key: &str) -> bool {
            if key.is_empty() {
                return false;
            }
            // disallow control or whitespace characters
            if key.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
                return false;
            }
            // disallow structural punctuators
            if key
                .chars()
                .any(|ch| matches!(ch, ',' | ':' | '[' | ']' | '{' | '}'))
            {
                return false;
            }
            // disallow double quote inside unquoted key
            if key.contains('"') {
                return false;
            }
            // avoid keys that start with common comment starters
            if key.starts_with('#') || key.starts_with("//") || key.starts_with("/*") {
                return false;
            }
            true
        }

        let key_token = if is_unquoted_key(key) {
            SyntaxKind::UnquotedString.create_token(key)
        } else {
            SyntaxKind::String.create_token(format!(
                "\"{}\"",
                StringLiteralCodec::escape_double_quoted(key)
            ))
        };

        let value_node = Self::create_value_node(value, string_style, existing_style)?;
        Ok(SyntaxKind::Pair.create_node(vec![
            NodeOrToken::Node(SyntaxKind::Key.create_node(vec![NodeOrToken::Token(key_token)])),
            NodeOrToken::Token(SyntaxKind::Colon.create_token(":")),
            NodeOrToken::Token(SyntaxKind::Whitespace.create_token(" ")),
            NodeOrToken::Node(value_node),
        ]))
    }

    fn create_string_token(value: &str, style: StringStyle) -> rowan::GreenToken {
        match style {
            StringStyle::DoubleQuoted | StringStyle::Preserve | StringStyle::Auto => {
                SyntaxKind::String.create_token(format!(
                    "\"{}\"",
                    StringLiteralCodec::escape_double_quoted(value)
                ))
            }
            StringStyle::Block => SyntaxKind::BlockString.create_token(
                StringLiteralCodec::create_multiline_string_literal(value, "", "\n"),
            ),
            StringStyle::Unquoted => SyntaxKind::UnquotedString.create_token(value),
        }
    }
}

pub(crate) struct PairValueEditor;

impl PairValueEditor {
    pub(crate) fn replace(
        pair_green: &GreenNode,
        value: &HjsonValue,
        string_style: StringStyle,
    ) -> Result<GreenNode, HjsonEditError> {
        let pair_red = RedNode::new_root(pair_green.clone());
        let existing_style = pair_red
            .child_by_kind(SyntaxKind::Value)
            .as_ref()
            .and_then(StringStyleResolver::for_red_value_node);
        let mut children = Vec::new();
        let mut replaced = false;

        for child in pair_green.children() {
            let cloned = clone_element(child);
            match &cloned {
                NodeOrToken::Node(node)
                    if HjsonLanguage::kind_from_raw(node.kind()) == SyntaxKind::Value =>
                {
                    children.push(NodeOrToken::Node(ValueCodec::replace_value_node(
                        value,
                        string_style,
                        existing_style,
                    )?));
                    replaced = true;
                }
                NodeOrToken::Token(token)
                    if HjsonLanguage::kind_from_raw(token.kind()) == SyntaxKind::Colon =>
                {
                    children.push(cloned.clone());
                    Self::append_missing_space_after_colon(&mut children, pair_green);
                }
                _ => children.push(cloned),
            }
        }

        if replaced {
            return Ok(SyntaxKind::Pair.create_node(children));
        }

        Self::rebuild_pair(pair_green, value, string_style, existing_style)
    }

    fn append_missing_space_after_colon(children: &mut Vec<GreenElement>, pair_green: &GreenNode) {
        if Self::has_space_after_colon(pair_green) {
            return;
        }

        children.push(NodeOrToken::Token(SyntaxKind::Whitespace.create_token(" ")));
    }

    fn has_space_after_colon(pair_green: &GreenNode) -> bool {
        let mut after_colon = false;
        for child in pair_green.children() {
            match child {
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if kind == SyntaxKind::Colon {
                        after_colon = true;
                        continue;
                    }
                    if after_colon {
                        return kind == SyntaxKind::Whitespace;
                    }
                }
                NodeOrToken::Node(_) if after_colon => return false,
                _ => {}
            }
        }
        false
    }

    fn rebuild_pair(
        pair_green: &GreenNode,
        value: &HjsonValue,
        string_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> Result<GreenNode, HjsonEditError> {
        let key_node = RedNode::new_root(pair_green.clone())
            .child_by_kind(SyntaxKind::Key)
            .ok_or(HjsonEditError::InvalidNodeKind {
                expected: SyntaxKind::Key,
                found: SyntaxKind::Pair,
            })?;
        let key = KeyText::extract(&key_node).ok_or_else(|| HjsonEditError::InvalidPath {
            path: "pair key".to_string(),
        })?;
        ValueCodec::create_pair_node(&key, value, string_style, existing_style)
    }
}

pub(crate) struct StringStyleResolver;

impl StringStyleResolver {
    pub(crate) fn for_red_value_node(node: &RedNode) -> Option<StringStyle> {
        for elem in node.children_with_tokens() {
            let Some(token) = elem.as_token() else {
                continue;
            };
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
        None
    }

    pub(crate) fn for_value_node(node: &GreenElement) -> Option<StringStyle> {
        match node {
            NodeOrToken::Node(green) => {
                let red = RedNode::new_root(green.clone());
                Self::for_red_value_node(&red)
            }
            NodeOrToken::Token(token) => match HjsonLanguage::kind_from_raw(token.kind()) {
                SyntaxKind::String => Some(StringStyle::DoubleQuoted),
                SyntaxKind::BlockString => Some(StringStyle::Block),
                SyntaxKind::UnquotedString => Some(StringStyle::Unquoted),
                _ => None,
            },
        }
    }

    pub(crate) fn resolve(
        value: &str,
        requested_style: StringStyle,
        existing_style: Option<StringStyle>,
    ) -> StringStyle {
        match requested_style {
            StringStyle::Preserve => existing_style.unwrap_or_else(|| Self::auto(value)),
            StringStyle::Auto => Self::auto(value),
            explicit => explicit,
        }
    }

    pub(crate) fn auto(value: &str) -> StringStyle {
        if value.contains('\n') || value.contains('\r') {
            return StringStyle::Block;
        }
        if value.starts_with('[') || value.starts_with('{') {
            return StringStyle::DoubleQuoted;
        }
        if Self::is_safe_unquoted(value) {
            StringStyle::Unquoted
        } else {
            StringStyle::DoubleQuoted
        }
    }

    pub(crate) fn is_safe_unquoted(value: &str) -> bool {
        if value.is_empty() {
            return false;
        }
        if matches!(value, "true" | "false" | "null") {
            return false;
        }
        if HjsonNumber::is_valid(value) {
            return false;
        }
        // disallow starting comment sequences
        if value.starts_with("//") || value.starts_with("/*") || value.starts_with('#') {
            return false;
        }
        // disallow starting structural punctuators
        if matches!(
            value.chars().next(),
            Some('{') | Some('}') | Some('[') | Some(']') | Some(',') | Some(':')
        ) {
            return false;
        }
        // disallow control/newline chars anywhere
        if value
            .chars()
            .any(|ch| matches!(ch, '\n' | '\r' | '\u{0000}'..='\u{001F}'))
        {
            return false;
        }

        // If a comment-start sequence appears inside the value and the
        // fragment immediately before it (trimmed of trailing spaces/tabs)
        // is empty or looks like a literal/number, then leaving the value
        // unquoted would cause the lexer/parser to split it into a
        // literal + comment. Treat that as unsafe.
        let mut search_pos = 0usize;
        while search_pos < value.len() {
            // find next occurrence of any comment starter
            let mut next_found: Option<(usize, usize)> = None; // (idx, len)

            if let Some(i) = value[search_pos..].find("//") {
                let idx = search_pos + i;
                if next_found.is_none() || idx < next_found.unwrap().0 {
                    next_found = Some((idx, 2));
                }
            }
            if let Some(i) = value[search_pos..].find("/*") {
                let idx = search_pos + i;
                if next_found.is_none() || idx < next_found.unwrap().0 {
                    next_found = Some((idx, 2));
                }
            }
            if let Some(i) = value[search_pos..].find('#') {
                let idx = search_pos + i;
                if next_found.is_none() || idx < next_found.unwrap().0 {
                    next_found = Some((idx, 1));
                }
            }

            let (idx, pat_len) = match next_found {
                Some(t) => t,
                None => break,
            };

            let before = &value[..idx];
            let trimmed_before = before.trim_end_matches([' ', '\t']);
            if trimmed_before.is_empty()
                || HjsonNumber::is_valid(trimmed_before)
                || matches!(trimmed_before, "true" | "false" | "null")
            {
                return false;
            }

            search_pos = idx + pat_len;
        }

        true
    }
}

pub(crate) struct StringLiteralCodec;

enum UnescapedFragment {
    Char(char),
    Raw(String),
}

impl StringLiteralCodec {
    pub(crate) fn strip_surrounding(text: &str, delimiter_len: usize) -> String {
        let min_len = delimiter_len * 2;
        if text.len() >= min_len {
            text[delimiter_len..text.len() - delimiter_len].to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn unescape_json_string(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch != '\\' {
                result.push(ch);
                continue;
            }

            match Self::decode_escape(&mut chars) {
                UnescapedFragment::Char(decoded) => result.push(decoded),
                UnescapedFragment::Raw(raw) => result.push_str(&raw),
            }
        }

        result
    }

    fn decode_escape(chars: &mut Peekable<Chars<'_>>) -> UnescapedFragment {
        match chars.next() {
            Some('\\') => UnescapedFragment::Char('\\'),
            Some('"') => UnescapedFragment::Char('"'),
            Some('/') => UnescapedFragment::Char('/'),
            Some('b') => UnescapedFragment::Char('\x08'),
            Some('f') => UnescapedFragment::Char('\x0C'),
            Some('n') => UnescapedFragment::Char('\n'),
            Some('r') => UnescapedFragment::Char('\r'),
            Some('t') => UnescapedFragment::Char('\t'),
            Some('u') => Self::decode_unicode_escape(chars),
            Some(other) => UnescapedFragment::Raw(format!("\\{other}")),
            None => UnescapedFragment::Raw("\\".to_string()),
        }
    }

    fn decode_unicode_escape(chars: &mut Peekable<Chars<'_>>) -> UnescapedFragment {
        let hex: String = chars.by_ref().take(4).collect();
        if hex.len() != 4 {
            return UnescapedFragment::Raw(format!("\\u{hex}"));
        }

        let Ok(codepoint) = u16::from_str_radix(&hex, 16) else {
            return UnescapedFragment::Raw(format!("\\u{hex}"));
        };

        if Self::is_high_surrogate(codepoint) {
            return Self::decode_surrogate_pair(chars, codepoint, &hex);
        }
        if Self::is_low_surrogate(codepoint) {
            return UnescapedFragment::Raw(format!("\\u{hex}"));
        }

        char::from_u32(codepoint as u32)
            .map(UnescapedFragment::Char)
            .unwrap_or_else(|| UnescapedFragment::Raw(format!("\\u{hex}")))
    }

    fn decode_surrogate_pair(
        chars: &mut Peekable<Chars<'_>>,
        high: u16,
        high_hex: &str,
    ) -> UnescapedFragment {
        let mut lookahead = chars.clone();
        if lookahead.next() != Some('\\') || lookahead.next() != Some('u') {
            return UnescapedFragment::Raw(format!("\\u{high_hex}"));
        }

        let low_hex: String = lookahead.by_ref().take(4).collect();
        if low_hex.len() != 4 {
            return UnescapedFragment::Raw(format!("\\u{high_hex}\\u{low_hex}"));
        }

        let Ok(low) = u16::from_str_radix(&low_hex, 16) else {
            return UnescapedFragment::Raw(format!("\\u{high_hex}\\u{low_hex}"));
        };
        if !Self::is_low_surrogate(low) {
            return UnescapedFragment::Raw(format!("\\u{high_hex}\\u{low_hex}"));
        }

        chars.next();
        chars.next();
        for _ in 0..4 {
            chars.next();
        }

        let combined = 0x10000 + (((high as u32) - 0xD800) << 10) + ((low as u32) - 0xDC00);
        char::from_u32(combined)
            .map(UnescapedFragment::Char)
            .unwrap_or_else(|| UnescapedFragment::Raw(format!("\\u{high_hex}\\u{low_hex}")))
    }

    fn is_high_surrogate(codepoint: u16) -> bool {
        (0xD800..=0xDBFF).contains(&codepoint)
    }

    fn is_low_surrogate(codepoint: u16) -> bool {
        (0xDC00..=0xDFFF).contains(&codepoint)
    }

    pub(crate) fn parse_multiline_string(text: &str, head_column: usize) -> String {
        let normalized = Self::strip_surrounding(text, 3).replace("\r\n", "\n");
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

        lines
            .into_iter()
            .map(|line| Self::strip_indent_to_column(line, inferred_column))
            .collect::<Vec<_>>()
            .join("\n")
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

    pub(crate) fn escape_double_quoted(value: &str) -> String {
        let mut result = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                '\x08' => result.push_str("\\b"),
                '\x0C' => result.push_str("\\f"),
                control if control.is_control() || (control as u32) < 0x20 => {
                    result.push_str(&format!("\\u{:04x}", control as u32));
                }
                other => result.push(other),
            }
        }
        result
    }

    pub(crate) fn create_multiline_string_literal(
        value: &str,
        indent: &str,
        newline: &str,
    ) -> String {
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
}

pub(crate) struct KeyText;

impl KeyText {
    pub(crate) fn extract(key_node: &RedNode) -> Option<String> {
        for elem in key_node.children_with_tokens() {
            let Some(token) = elem.as_token() else {
                continue;
            };

            match token.kind() {
                SyntaxKind::UnquotedString => return Some(token.text().to_string()),
                SyntaxKind::String => {
                    let text = token.text();
                    if text.len() >= 2 {
                        let unquoted = &text[1..text.len() - 1];
                        return Some(StringLiteralCodec::unescape_json_string(unquoted));
                    }
                    return Some(text.to_string());
                }
                _ => {}
            }
        }
        None
    }
}

pub(crate) fn value_needs_contextual_formatting(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> bool {
    ValueCodec::needs_contextual_formatting(value, string_style, existing_style)
}

// pub(crate) fn format_target_for_value(
//     document: &HjsonDocument,
//     path: &[PathSegment],
//     prefer_container: bool,
// ) -> Option<FormatTargetUnit> {
//     ValueCodec::format_target_for_value(document, path, prefer_container)
// }

pub(crate) fn node_to_value(node: &RedNode) -> Option<HjsonValue> {
    ValueCodec::node_to_value(node)
}

pub(crate) fn parse_multiline_string(text: &str, head_column: usize) -> String {
    StringLiteralCodec::parse_multiline_string(text, head_column)
}

pub(crate) fn string_style_for_value_node(node: &GreenElement) -> Option<StringStyle> {
    StringStyleResolver::for_value_node(node)
}

pub(crate) fn create_value_node(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    ValueCodec::create_value_node(value, string_style, existing_style)
}

pub(crate) fn value_to_green_node(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    ValueCodec::value_to_green_node(value, string_style, existing_style)
}

pub(crate) fn create_pair_node(
    key: &str,
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    ValueCodec::create_pair_node(key, value, string_style, existing_style)
}

pub(crate) fn replace_pair_value(
    pair_green: &GreenNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    PairValueEditor::replace(pair_green, value, string_style)
}

pub(crate) fn create_multiline_string_literal(value: &str, indent: &str, newline: &str) -> String {
    StringLiteralCodec::create_multiline_string_literal(value, indent, newline)
}

pub(crate) fn extract_key_text(key_node: &RedNode) -> Option<String> {
    KeyText::extract(key_node)
}
