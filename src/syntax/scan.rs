//! 生テキストの走査（キーと値の範囲決定）。
//!
//! キーや quoteless 値の範囲は文法位置に依存するため、字句解析器ではなく
//! 構文解析器がこの走査を使って単位を確定する（基準実装 hjson-cs と同じ構造）。

use crate::number::{HjsonNumber, NumberKind};
use crate::syntax::SyntaxKind;

/// 走査で見つかった問題。
#[derive(Debug, Clone, PartialEq)]
pub struct ScanError {
    pub offset: usize,
    pub message: String,
}

impl ScanError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

fn is_punctuator(c: char) -> bool {
    matches!(c, '{' | '}' | '[' | ']' | ',' | ':')
}

/// キーの走査結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// キー名の範囲（前後の空白を含めない）。
    pub name_start: usize,
    pub name_end: usize,
    /// キーの次の位置（引用符なしキーでは `:` の位置、引用キーでは閉じ引用符の次）。
    pub end: usize,
}

/// キーを走査する。引用キーは閉じ引用符まで、引用符なしキーは `:` までをキー名とする。
pub fn scan_key(source: &str, start: usize) -> Result<Key, ScanError> {
    let rest = &source[start..];
    let first = rest.chars().next();

    if let Some(quote @ ('"' | '\'')) = first {
        // 引用符付きキー。複数行は不可。
        let mut escaped = false;
        for (offset, c) in rest.char_indices().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            match c {
                '\\' => escaped = true,
                '\n' | '\r' => {
                    return Err(ScanError::new(
                        start + offset,
                        "Unterminated string literal",
                    ))
                }
                c if c == quote => {
                    let end = start + offset + c.len_utf8();
                    return Ok(Key {
                        name_start: start,
                        name_end: end,
                        end,
                    });
                }
                _ => {}
            }
        }
        return Err(ScanError::new(start, "Unterminated string literal"));
    }

    // 引用符なしキー。`:` までをキーとし、`#` `//` `/*` も文字として含める。
    let mut name_end = None;
    let mut first_space = None;
    for (offset, c) in rest.char_indices() {
        let position = start + offset;
        if c == ':' {
            let Some(name_end) = name_end else {
                return Err(ScanError::new(position, "Found ':' but no key name"));
            };
            // 末尾以外の空白はキー名に含められない
            if let Some(space) = first_space {
                if space < name_end {
                    return Err(ScanError::new(
                        start,
                        "Found whitespace in your key name (use quotes to include)",
                    ));
                }
            }
            return Ok(Key {
                name_start: start,
                name_end,
                end: position,
            });
        }
        // キーと `:` の間の空白・改行は許される（内容が続く場合のみ誤り）
        if is_punctuator(c) {
            return Err(ScanError::new(
                position,
                format!("Found '{c}' where a key name was expected"),
            ));
        }
        if c.is_whitespace() {
            first_space.get_or_insert(position);
            continue;
        }
        name_end = Some(position + c.len_utf8());
    }

    Err(ScanError::new(start, "Name is not closed"))
}

/// quoteless 値の終端を返す（基準実装 hjson-cs の readTfnns と同じ規則）。
///
/// 行末または `,` `}` `]` `#` `//` `/*` で終端する。ただし手前が数値または
/// キーワード（true/false/null）のときだけ、その位置で終端する。
pub fn scan_quoteless_value(source: &str, start: usize) -> usize {
    let mut last_good = start;
    let mut chars = source[start..].char_indices().peekable();

    while let Some((offset, c)) = chars.next() {
        if c == '\n' || c == '\r' {
            break;
        }

        let position = start + offset;
        let next = chars.peek().map(|(_, c)| *c);
        let is_delimiter = matches!(c, ',' | '}' | ']' | '#')
            || (c == '/' && matches!(next, Some('/') | Some('*')));

        if is_delimiter && is_keyword_or_number(&source[start..position]) {
            let text = source[start..position].trim_end();
            return start + text.len();
        }

        last_good = position + c.len_utf8();
    }

    // 行末では末尾の空白を値に含めない
    let text = source[start..last_good].trim_end();
    start + text.len()
}

/// quoteless 値のテキストからトークン種別を決める。
pub fn classify_value(text: &str) -> SyntaxKind {
    match text {
        "true" => SyntaxKind::True,
        "false" => SyntaxKind::False,
        "null" => SyntaxKind::Null,
        _ => match HjsonNumber::classify(text) {
            Some(NumberKind::Integer) => SyntaxKind::Integer,
            Some(NumberKind::Float) => SyntaxKind::Float,
            None => SyntaxKind::UnquotedString,
        },
    }
}

fn is_keyword_or_number(text: &str) -> bool {
    matches!(text.trim(), "true" | "false" | "null") || HjsonNumber::is_valid(text.trim())
}
