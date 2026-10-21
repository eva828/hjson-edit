//! 準拠テスト用の正準形（内部 API）。
//!
//! hjson-cs を基準にした適合テスト（`tests/conformance_cs.rs` など）が、
//! 実装の解析結果と基準の実行結果を比較するために使う。
//! 利用者向けの API ではないため `#[doc(hidden)]` として公開している。

use crate::high_level::value::HjsonValue;
use crate::high_level::{ops::object::HjsonDocumentEdit, path::HjsonObjectPath, HjsonDocument};

/// 文字列を TSV の 1 フィールドに収まるようエスケープする。
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// TSV の入力を元のテキストへ戻す（`escape` の逆。`\uXXXX` にも対応）。
pub fn unescape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        match chars.get(i + 1).copied() {
            Some('n') => {
                out.push('\n');
                i += 2;
            }
            Some('r') => {
                out.push('\r');
                i += 2;
            }
            Some('t') => {
                out.push('\t');
                i += 2;
            }
            Some('\\') => {
                out.push('\\');
                i += 2;
            }
            Some('u') => {
                let hex: String = chars[i + 2..(i + 6).min(chars.len())].iter().collect();
                let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                i += 6;
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
                i += 2;
            }
            None => {
                out.push('\\');
                i += 1;
            }
        }
    }
    out
}

/// 値を正準形にする（整数と浮動小数を区別し、キー順はそのまま）。
pub fn canonicalize(value: &HjsonValue) -> String {
    match value {
        HjsonValue::String(s) => format!("\"{}\"", escape(s)),
        HjsonValue::Integer(i) => i.to_string(),
        HjsonValue::Float(f) => {
            if f.is_nan() {
                "NaN".to_string()
            } else if f.is_infinite() {
                if *f > 0.0 {
                    "inf".to_string()
                } else {
                    "-inf".to_string()
                }
            } else {
                format!("{f:?}")
            }
        }
        HjsonValue::Bool(b) => b.to_string(),
        HjsonValue::Null => "null".to_string(),
        HjsonValue::Array(items) => {
            let parts: Vec<String> = items.iter().map(canonicalize).collect();
            format!("[{}]", parts.join(","))
        }
        HjsonValue::Object(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", escape(k), canonicalize(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// 解析結果を「`状態<TAB>値またはメッセージ`」で返す。
///
/// 状態は `OK`（値）、`ERR`（解析エラー）、`ERRV`（解析後の値変換エラー）。
pub fn evaluate(text: &str) -> String {
    match HjsonDocument::parse(text) {
        Err(e) => {
            let rendered = e.to_string();
            let first = rendered.lines().next().unwrap_or("ERR").to_string();
            format!("ERR\t{first}")
        }
        Ok(document) => {
            let root = match HjsonObjectPath::parse("") {
                Ok(path) => path,
                Err(e) => return format!("ERR\tpath: {e}"),
            };
            match document.get(root) {
                Ok(value) => format!("OK\t{}", canonicalize(&value)),
                Err(e) => {
                    let rendered = e.to_string();
                    let first = rendered.lines().next().unwrap_or("ERR").to_string();
                    format!("ERRV\t{first}")
                }
            }
        }
    }
}
