use rowan::{GreenNode, NodeOrToken};

use crate::ast::{AstNode, Key, QuoteStyle, Trivia};
use crate::high_level::error::HjsonEditError;
use crate::syntax::{GreenElement, NewlineKind, SyntaxKind};
use crate::{RedNode, RedToken};

/// HJSON の値。
#[derive(Debug, Clone, PartialEq)]
pub enum HjsonValue {
    /// 文字列。
    String(String),
    /// 整数。
    Integer(i64),
    /// 浮動小数。
    Float(f64),
    /// 真偽値。
    Bool(bool),
    /// null。
    Null,
    /// 配列。
    Array(Vec<HjsonValue>),
    /// オブジェクト（キーと値の並び。順序を保つ）。
    Object(Vec<(String, HjsonValue)>),
}

/// 値の描画設定。
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderConfig {
    /// 文字列のスタイル。`None` なら書ける範囲で引用符なしを選ぶ。
    pub string_style: Option<QuoteStyle>,
    /// 入れ子の値を1行で描けないときに使う改行コード。
    pub newline: NewlineKind,
}

impl From<&str> for HjsonValue {
    fn from(s: &str) -> Self {
        HjsonValue::String(s.to_string())
    }
}

impl From<String> for HjsonValue {
    fn from(s: String) -> Self {
        HjsonValue::String(s)
    }
}

impl From<i64> for HjsonValue {
    fn from(n: i64) -> Self {
        HjsonValue::Integer(n)
    }
}

impl From<i32> for HjsonValue {
    fn from(n: i32) -> Self {
        HjsonValue::Integer(n as i64)
    }
}

impl From<f64> for HjsonValue {
    fn from(n: f64) -> Self {
        HjsonValue::Float(n)
    }
}

impl From<f32> for HjsonValue {
    fn from(n: f32) -> Self {
        HjsonValue::Float(n as f64)
    }
}

impl From<bool> for HjsonValue {
    fn from(b: bool) -> Self {
        HjsonValue::Bool(b)
    }
}

impl From<Vec<HjsonValue>> for HjsonValue {
    fn from(v: Vec<HjsonValue>) -> Self {
        HjsonValue::Array(v)
    }
}

impl From<Vec<(String, HjsonValue)>> for HjsonValue {
    fn from(v: Vec<(String, HjsonValue)>) -> Self {
        HjsonValue::Object(v)
    }
}

impl HjsonValue {
    /// 2 つの値が同じか。整数と浮動小数は数値として比較する。
    pub fn same_value(&self, other: &Self) -> bool {
        crate::high_level::value_ops::values_equal(self, other)
    }

    /// 値に含まれる文字列を順に呼び出す。
    pub(crate) fn for_each_string(&self, visit: &mut impl FnMut(&str)) {
        match self {
            HjsonValue::String(text) => visit(text),
            HjsonValue::Array(items) => {
                for item in items {
                    item.for_each_string(visit);
                }
            }
            HjsonValue::Object(entries) => {
                for (_, value) in entries {
                    value.for_each_string(visit);
                }
            }
            HjsonValue::Integer(_)
            | HjsonValue::Float(_)
            | HjsonValue::Bool(_)
            | HjsonValue::Null => {}
        }
    }

    /// 値の全文字列を `style` で書けるか。
    pub(crate) fn is_representable_with(&self, style: QuoteStyle) -> bool {
        let mut ok = true;
        self.for_each_string(&mut |text| ok &= style.can_render(text));
        ok
    }

    /// HJSON で表現できない数値（NaN と無限大）を含むか。
    pub(crate) fn has_unrepresentable_number(&self) -> bool {
        match self {
            HjsonValue::Float(number) => !number.is_finite(),
            HjsonValue::Array(items) => items.iter().any(Self::has_unrepresentable_number),
            HjsonValue::Object(entries) => entries
                .iter()
                .any(|(_, value)| value.has_unrepresentable_number()),
            HjsonValue::String(_)
            | HjsonValue::Integer(_)
            | HjsonValue::Bool(_)
            | HjsonValue::Null => false,
        }
    }

    /// 数値が HJSON で表現できるか（NaN・無限大は不可）。
    pub(crate) fn ensure_representable(&self) -> Result<(), HjsonEditError> {
        if self.has_unrepresentable_number() {
            return Err(HjsonEditError::ValueNotRepresentable {
                reason: "NaN and infinity have no HJSON representation".to_string(),
            });
        }

        Ok(())
    }

    pub(crate) fn render_value_node(&self, config: &RenderConfig) -> GreenNode {
        GreenNode::new(SyntaxKind::Value.into(), self.render_body(config))
    }

    pub(crate) fn render_member_node(
        &self,
        key: &str,
        key_style: Option<QuoteStyle>,
        config: &RenderConfig,
        before_colon_trivia: &[Trivia],
        after_colon_trivia: &[Trivia],
    ) -> GreenNode {
        let mut children: Vec<GreenElement> =
            Vec::with_capacity(3 + before_colon_trivia.len() + after_colon_trivia.len());
        children.push(render_key_node(key, key_style).into());
        for trivia in before_colon_trivia {
            children.push(trivia.to_green_element());
        }
        children.push(SyntaxKind::Colon.create_token(":").into());
        for trivia in after_colon_trivia {
            children.push(trivia.to_green_element());
        }
        children.push(self.render_value_node(config).into());
        GreenNode::new(SyntaxKind::Member.into(), children)
    }

    fn render_body(&self, config: &RenderConfig) -> Vec<GreenElement> {
        match self {
            HjsonValue::String(s) => {
                let style = config.string_style.unwrap_or_else(|| {
                    if QuoteStyle::Unquoted.can_render(s) {
                        QuoteStyle::Unquoted
                    } else {
                        QuoteStyle::DoubleQuoted
                    }
                });
                let (kind, text) = render_string(s, style);
                vec![kind.create_token(text.as_str()).into()]
            }
            HjsonValue::Integer(n) => {
                let text = n.to_string();
                vec![SyntaxKind::Integer.create_token(text.as_str()).into()]
            }
            HjsonValue::Float(n) => {
                let text = n.to_string();
                vec![SyntaxKind::Float.create_token(text.as_str()).into()]
            }
            HjsonValue::Bool(true) => {
                vec![SyntaxKind::True.create_token("true").into()]
            }
            HjsonValue::Bool(false) => {
                vec![SyntaxKind::False.create_token("false").into()]
            }
            HjsonValue::Null => {
                vec![SyntaxKind::Null.create_token("null").into()]
            }
            HjsonValue::Array(items) => {
                let mut children = vec![SyntaxKind::LBracket.create_token("[").into()];
                for (idx, item) in items.iter().enumerate() {
                    children.push(item.render_value_node(config).into());
                    if idx + 1 != items.len() {
                        let separator = render_separator(children.last().unwrap(), config);
                        children.push(separator);
                    }
                }
                push_terminating_newline(&mut children, config);
                children.push(SyntaxKind::RBracket.create_token("]").into());
                vec![GreenNode::new(SyntaxKind::Array.into(), children).into()]
            }
            HjsonValue::Object(entries) => {
                let mut children = vec![SyntaxKind::LBrace.create_token("{").into()];
                for (idx, (key, value)) in entries.iter().enumerate() {
                    children.push(
                        value
                            .render_member_node(
                                key,
                                None,
                                config,
                                &[],
                                &[Trivia::Whitespace(" ".to_string())],
                            )
                            .into(),
                    );
                    if idx + 1 != entries.len() {
                        let separator = render_separator(children.last().unwrap(), config);
                        children.push(separator);
                    }
                }
                push_terminating_newline(&mut children, config);
                children.push(SyntaxKind::RBrace.create_token("}").into());
                vec![GreenNode::new(SyntaxKind::Object.into(), children).into()]
            }
        }
    }
}

/// 要素の末尾が引用符なし文字列かどうか。
///
/// 引用符なし文字列は行末でしか終端しないため、直後に `,` や `}`/`]` を置くと
/// それらが値に吸収されて文書が壊れる。挿入位置と改行の判断に使う。
pub(crate) fn ends_with_unquoted_string(element: &GreenElement) -> bool {
    match element {
        NodeOrToken::Token(token) => token.kind() == SyntaxKind::UnquotedString.into(),
        NodeOrToken::Node(node) => RedNode::new_root(node.clone())
            .last_token()
            .is_some_and(|token| token.kind() == SyntaxKind::UnquotedString),
    }
}

/// 描画した要素の後ろに置く区切り（カンマまたは改行）を返す。
///
/// 直前の要素が引用符なし文字列で終わる場合、カンマはその文字列に吸収されるため改行を使う。
fn render_separator(element: &GreenElement, config: &RenderConfig) -> GreenElement {
    if ends_with_unquoted_string(element) {
        newline_element(config)
    } else {
        SyntaxKind::Comma.create_token(",").into()
    }
}

fn newline_element(config: &RenderConfig) -> GreenElement {
    SyntaxKind::Newline
        .create_token(config.newline.as_str())
        .into()
}

/// 閉じ区切りの直前に引用符なし文字列が来るとき、値の終端として改行を補う。
///
/// オブジェクト/配列を1行で描画する際、最後の要素が引用符なし文字列だと
/// 閉じ区切りがその文字列に吸収されるため、区切りの前で行を終わらせる。
fn push_terminating_newline(children: &mut Vec<GreenElement>, config: &RenderConfig) {
    if children.last().is_some_and(ends_with_unquoted_string) {
        children.push(newline_element(config));
    }
}

impl HjsonValue {
    fn from_token(token: &RedToken) -> Result<Self, HjsonEditError> {
        let text = token.text();
        match token.kind() {
            SyntaxKind::String => {
                let style = if text.starts_with('\'') {
                    QuoteStyle::SingleQuoted
                } else {
                    QuoteStyle::DoubleQuoted
                };
                Ok(HjsonValue::String(style.decode_body(text, 0)))
            }
            SyntaxKind::BlockString => Ok(HjsonValue::String(
                QuoteStyle::Block.decode_body(text, crate::ast::token_column(token)),
            )),
            SyntaxKind::UnquotedString => Ok(HjsonValue::String(
                QuoteStyle::Unquoted.decode_body(text, 0),
            )),
            // 整数として解釈されたテキストでも i64 に収まらない場合がある。
            // 基準実装（hjson-cs の TryParseNumericLiteral）は long で読めなければ
            // double として扱うため、同じ挙動にする。
            SyntaxKind::Integer => match text.parse::<i64>() {
                Ok(value) => Ok(HjsonValue::Integer(value)),
                Err(_) => text.parse::<f64>().map(HjsonValue::Float).map_err(|_| {
                    HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Float,
                        found: token.kind(),
                    }
                }),
            },
            SyntaxKind::Float => text.parse::<f64>().map(HjsonValue::Float).map_err(|_| {
                HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Float,
                    found: token.kind(),
                }
            }),
            SyntaxKind::True => Ok(HjsonValue::Bool(true)),
            SyntaxKind::False => Ok(HjsonValue::Bool(false)),
            SyntaxKind::Null => Ok(HjsonValue::Null),
            _ => Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: token.kind(),
            }),
        }
    }

    pub(crate) fn from_node(node: &RedNode) -> Result<Self, HjsonEditError> {
        match node.kind() {
            SyntaxKind::Root => {
                let child = node
                    .children()
                    .find(|child| {
                        matches!(
                            child.kind(),
                            SyntaxKind::Value | SyntaxKind::Object | SyntaxKind::Array
                        )
                    })
                    .ok_or(HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Value,
                        found: node.kind(),
                    })?;
                Self::from_node(&child)
            }
            SyntaxKind::Value => node
                .children_with_tokens()
                .find(|child| match child {
                    NodeOrToken::Node(child) => !child.kind().is_trivia(),
                    NodeOrToken::Token(token) => !token.kind().is_trivia(),
                })
                .ok_or(HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Value,
                    found: node.kind(),
                })
                .and_then(|child| match child {
                    NodeOrToken::Node(child) => Self::from_node(&child),
                    NodeOrToken::Token(token) => Self::from_token(&token),
                }),
            SyntaxKind::Object => {
                // 重複キーは後勝ち（基準実装の JsonObject と同じ）。位置は最初の出現を保つ。
                let mut entries: Vec<(String, HjsonValue)> = Vec::new();
                // メンバーごとに線形走査すると要素数に対して二次関数的になる
                let mut positions: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for member in node
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Member)
                {
                    let key = member
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Key)
                        .and_then(|key_node| Key::cast(key_node).ok())
                        .and_then(|k| k.body_text())
                        .ok_or(HjsonEditError::UnexpectedStructure {
                            expected: SyntaxKind::Value,
                            found: node.kind(),
                        })?;
                    let value = member
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Value)
                        .ok_or(HjsonEditError::UnexpectedStructure {
                            expected: SyntaxKind::Value,
                            found: node.kind(),
                        })?;
                    let parsed = Self::from_node(&value)?;
                    match positions.get(&key) {
                        Some(index) => entries[*index].1 = parsed,
                        None => {
                            positions.insert(key.clone(), entries.len());
                            entries.push((key, parsed));
                        }
                    }
                }
                Ok(HjsonValue::Object(entries))
            }
            SyntaxKind::Array => {
                let mut items = Vec::new();
                for value in node
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Value)
                {
                    items.push(Self::from_node(&value)?);
                }
                Ok(HjsonValue::Array(items))
            }
            SyntaxKind::String
            | SyntaxKind::BlockString
            | SyntaxKind::UnquotedString
            | SyntaxKind::Integer
            | SyntaxKind::Float
            | SyntaxKind::True
            | SyntaxKind::False
            | SyntaxKind::Null => node
                .children_with_tokens()
                .find_map(|child| match child {
                    NodeOrToken::Token(token) if !token.kind().is_trivia() => Some(token),
                    _ => None,
                })
                .ok_or(HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Value,
                    found: node.kind(),
                })
                .and_then(|token| Self::from_token(&token)),
            _ => Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            }),
        }
    }
}

fn render_key_node(key: &str, key_style: Option<QuoteStyle>) -> GreenNode {
    let (key_kind, key_text) = render_string(
        key,
        key_style.unwrap_or_else(|| {
            if QuoteStyle::Unquoted.can_be_key(key) {
                QuoteStyle::Unquoted
            } else {
                QuoteStyle::DoubleQuoted
            }
        }),
    );
    GreenNode::new(
        SyntaxKind::Key.into(),
        vec![key_kind.create_token(key_text.as_str()).into()],
    )
}

fn render_string(text: &str, style: QuoteStyle) -> (SyntaxKind, String) {
    (style.syntax_kind(), style.render(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_be_key_accepts_alphanumeric_underscore_dash_dot() {
        assert!(QuoteStyle::Unquoted.can_be_key("hello_world"));
        assert!(QuoteStyle::Unquoted.can_be_key("foo"));
        assert!(QuoteStyle::Unquoted.can_be_key("a-b.c_d-e0"));
    }

    #[test]
    fn test_can_be_key_rejects_space_and_punctuation() {
        assert!(!QuoteStyle::Unquoted.can_be_key("hello world"));
        assert!(!QuoteStyle::Unquoted.can_be_key("foo!"));
        assert!(!QuoteStyle::Unquoted.can_be_key(""));
    }

    #[test]
    fn test_can_be_key_rejects_reserved_words() {
        assert!(!QuoteStyle::Unquoted.can_be_key("true"));
        assert!(!QuoteStyle::Unquoted.can_be_key("false"));
        assert!(!QuoteStyle::Unquoted.can_be_key("null"));
    }

    #[test]
    fn test_can_be_key_rejects_numeric_tokens() {
        assert!(!QuoteStyle::Unquoted.can_be_key("123"));
        assert!(!QuoteStyle::Unquoted.can_be_key("1.23"));
    }

    #[test]
    fn test_render_body_string_default_style() {
        let config = RenderConfig::default();
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body.len(), 1);
        assert_eq!(body[0].kind(), SyntaxKind::UnquotedString.into());
    }

    #[test]
    fn test_render_body_string_double_quoted_style() {
        let config = RenderConfig {
            string_style: Some(QuoteStyle::DoubleQuoted),
            ..RenderConfig::default()
        };
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body[0].kind(), SyntaxKind::String.into());
    }

    #[test]
    fn test_render_body_string_block_style() {
        let config = RenderConfig {
            string_style: Some(QuoteStyle::Block),
            ..RenderConfig::default()
        };
        let s = HjsonValue::String("foo".to_string());
        let body = s.render_body(&config);
        assert_eq!(body[0].kind(), SyntaxKind::BlockString.into());
    }

    #[test]
    fn test_render_body_array_empty() {
        let config = RenderConfig::default();
        let arr = HjsonValue::Array(vec![]);
        let body = arr.render_body(&config);
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn test_render_body_object_empty() {
        let config = RenderConfig::default();
        let obj = HjsonValue::Object(vec![]);
        let body = obj.render_body(&config);
        assert_eq!(body.len(), 1);
    }
}
