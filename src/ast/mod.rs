//! 意味AST（lossless な構文木の射影）。
//!
//! 構文木は空白・改行・コメントをすべて保持する。このモジュールは
//! その木を読み取り専用で包み、メンバー・要素・トリビア・スロット位置を扱いやすくする。
//! 編集は高レベルAPI（[`crate::high_level`]）を使う。

use rowan::NodeOrToken;
use std::ops::Range;

use crate::syntax::{GreenElement, NewlineKind};
use crate::{HjsonEditError, RedNode, RedToken, SyntaxKind};

/// 文字列の書き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuoteStyle {
    /// 二重引用符（`"..."`）。
    DoubleQuoted,
    /// 単一引用符（`'...'`）。
    SingleQuoted,
    /// 複数行文字列（`'''...'''`）。
    Block,
    /// 引用符なし（行末まで）。
    Unquoted,
}

/// 引用符なしの値として書けるか。
///
/// 行頭で別のトークンになる文字を除外し、値の走査で行末まで 1 個の文字列として
/// 読み取れることを確認する。数値・キーワードは文字列として読み戻せない。
fn can_be_quoteless_value(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    // 行頭でトークンに分かれる文字（`{ } [ ] , :`）や、文字列・コメントの開始は
    // 引用符なしの値として読めない
    if text.starts_with(['{', '}', '[', ']', ',', ':', '"', '\'', '#']) {
        return false;
    }
    if text.starts_with("//") || text.starts_with("/*") || text.starts_with("'''") {
        return false;
    }
    // 行頭・行末の空白は値に含まれない
    if text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace) {
        return false;
    }
    if text.contains('\n') || text.contains('\r') {
        return false;
    }
    // 区切り文字の手前が数値・キーワードだと、そこで値が終端する
    let scanned = crate::syntax::scan::scan_quoteless_value(text, 0);
    if scanned.end != text.len() || scanned.consumed_end != text.len() {
        return false;
    }

    crate::syntax::scan::classify_value(text) == crate::syntax::SyntaxKind::UnquotedString
}

/// 複数行文字列（`'''`）として書けるか。
///
/// 本文はエスケープしないため、`'''` を含む場合や、本文の端に `'` がある場合は
/// 書けない。読み戻しでは行頭の字下げが `'''` のカラム分だけ取り除かれるので、
/// 行頭に空白のある本文も書けない。
fn can_be_block_value(text: &str) -> bool {
    if text.is_empty() || text.contains("'''") || text.contains('\r') {
        return false;
    }
    if text.starts_with('\n') || text.ends_with('\n') {
        return false;
    }
    // 本文の端の `'` は区切りの `'''` と繋がって別のトークンになる
    if text.starts_with('\'') || text.ends_with('\'') {
        return false;
    }

    text.split('\n')
        .all(|line| !line.starts_with(char::is_whitespace))
}

/// コメントの書き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    /// `//` の行コメント。
    Line,
    /// `#` の行コメント。
    Hash,
    /// `/* ... */` のブロックコメント。
    Block,
}

impl CommentKind {
    /// コメントの生テキストから本文を取り出す（記号と前後の空白を除く）。
    pub fn decode_body(&self, raw: &str) -> String {
        match self {
            CommentKind::Line => raw
                .strip_prefix("//")
                .unwrap_or(raw)
                .trim_start()
                .to_string(),
            CommentKind::Hash => raw
                .strip_prefix('#')
                .unwrap_or(raw)
                .trim_start()
                .to_string(),
            CommentKind::Block => raw
                .strip_prefix("/*")
                .and_then(|s| s.strip_suffix("*/"))
                .unwrap_or(raw)
                .trim()
                .to_string(),
        }
    }
}

/// 引用符付き文字列の本体をデコードする。
///
/// 基準実装（hjson-cs の ReadStringLiteral）と同じく `\uXXXX` をデコードし、
/// サロゲート対を 1 文字にまとめる。未知のエスケープはそのまま残す。
fn decode_escapes(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let mut code = 0u32;
                let mut ok = true;
                for _ in 0..4 {
                    match chars.next().and_then(|hex| hex.to_digit(16)) {
                        Some(digit) => code = code * 16 + digit,
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    out.push_str("\\u");
                    continue;
                }

                // サロゲート対を 1 文字にまとめる
                if (0xD800..0xDC00).contains(&code) {
                    let saved = chars.clone();
                    let mut low = None;
                    if chars.next() == Some('\\') && chars.next() == Some('u') {
                        let mut value = 0u32;
                        let mut valid = true;
                        for _ in 0..4 {
                            match chars.next().and_then(|hex| hex.to_digit(16)) {
                                Some(digit) => value = value * 16 + digit,
                                None => {
                                    valid = false;
                                    break;
                                }
                            }
                        }
                        if valid && (0xDC00..0xE000).contains(&value) {
                            low = Some(value);
                        }
                    }
                    if let Some(low) = low {
                        let combined = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                        out.push(char::from_u32(combined).unwrap_or('\u{FFFD}'));
                        continue;
                    }
                    chars = saved;
                }

                out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}

/// トークンの行頭からの桁位置（複数行文字列のインデント除去に使う）。
pub fn token_column(token: &RedToken) -> usize {
    let mut node = match token.parent() {
        Some(parent) => parent,
        None => return 0,
    };
    while let Some(parent) = node.parent() {
        node = parent;
    }

    let _ = &node;
    // 直前のトークンから改行までを数える（文書全体を文字列化しない）
    let mut column = 0usize;
    let mut current = token.prev_token();
    while let Some(previous) = current {
        let text = previous.text();
        if let Some(last_line) = text.rsplit_once('\n').map(|(_, rest)| rest) {
            column += last_line.chars().count();
            break;
        }
        column += text.chars().count();
        current = previous.prev_token();
    }
    column
}

impl QuoteStyle {
    /// 生テキストから本文を取り出す。複数行文字列は `indent` 桁分の字下げを除く。
    pub fn decode_body(&self, raw: &str, indent: usize) -> String {
        match self {
            QuoteStyle::DoubleQuoted | QuoteStyle::SingleQuoted => {
                let quote = if matches!(self, QuoteStyle::DoubleQuoted) {
                    '"'
                } else {
                    '\''
                };
                let inner = raw
                    .strip_prefix(quote)
                    .and_then(|text| text.strip_suffix(quote))
                    .unwrap_or(raw);

                decode_escapes(inner)
            }
            QuoteStyle::Block => {
                let inner = raw
                    .strip_prefix("'''")
                    .and_then(|text| text.strip_suffix("'''"))
                    .unwrap_or(raw)
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");

                // 先頭は `'''` の後ろの空白と改行 1 つを読み飛ばす（基準実装と同じ）。
                // 改行が続かない（1 行の）場合も、先頭の空白は値に含めない。
                let after_spaces = inner.trim_start_matches([' ', '\t']);
                let mut body = after_spaces.strip_prefix('\n').unwrap_or(after_spaces);
                if let Some(stripped) = body.strip_suffix('\n') {
                    body = stripped;
                }

                // `'''` のカラム分のインデントを各行から取り除く
                let mut lines: Vec<String> = body
                    .split('\n')
                    .map(|line| {
                        let mut skip = indent;
                        let mut chars = line.char_indices();
                        while skip > 0 {
                            match chars.next() {
                                Some((_, c)) if c == ' ' || c == '\t' => skip -= 1,
                                _ => break,
                            }
                        }
                        let taken = indent - skip;
                        line.chars().skip(taken).collect::<String>()
                    })
                    .collect();

                // 閉じ `'''` の行のインデントを落とした結果できる末尾の空行 1 つを除く
                if lines.last().is_some_and(|line| line.is_empty()) {
                    lines.pop();
                }
                lines.join("\n")
            }
            QuoteStyle::Unquoted => raw.to_string(),
        }
    }

    /// 本文をこのスタイルで書ける形に変換する（引用符の中身だけ）。
    pub fn encode_body(&self, body: &str) -> String {
        match self {
            QuoteStyle::SingleQuoted => {
                let mut out = String::with_capacity(body.len() + 2);
                for c in body.chars() {
                    match c {
                        '\\' => out.push_str("\\\\"),
                        '\'' => out.push_str("\\'"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        '\u{0008}' => out.push_str("\\b"),
                        '\u{000C}' => out.push_str("\\f"),
                        c if c.is_control() => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out
            }
            QuoteStyle::DoubleQuoted => {
                let mut out = String::with_capacity(body.len() + 2);
                for c in body.chars() {
                    match c {
                        '\\' => out.push_str("\\\\"),
                        '"' => out.push_str("\\\""),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        '\u{0008}' => out.push_str("\\b"),
                        '\u{000C}' => out.push_str("\\f"),
                        c if c.is_control() => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out
            }
            QuoteStyle::Block | QuoteStyle::Unquoted => body.to_string(),
        }
    }

    /// キーを引用符なしで書けるか。
    ///
    /// キーは `:` で終端するため値より制限が強い。迷う場合は引用符を付ける側に倒す。
    pub fn can_be_key(&self, text: &str) -> bool {
        match self {
            QuoteStyle::Unquoted => {
                !text.is_empty()
                    && text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    && !matches!(text, "true" | "false" | "null")
                    && !crate::number::HjsonNumber::is_valid(text)
            }
            QuoteStyle::DoubleQuoted | QuoteStyle::SingleQuoted | QuoteStyle::Block => true,
        }
    }

    /// 文字列をこのスタイルで書いて、元の文字列として読み戻せるか。
    ///
    /// 書けない組み合わせ（引用符なしの複数行など）で描画すると文書が壊れるため、
    /// 描画前に必ずこの関数で確認する。
    pub fn can_render(&self, text: &str) -> bool {
        match self {
            QuoteStyle::DoubleQuoted | QuoteStyle::SingleQuoted => true,
            QuoteStyle::Unquoted => can_be_quoteless_value(text),
            QuoteStyle::Block => can_be_block_value(text),
        }
    }

    /// 本文をこのスタイルの文字列として組み立てる（引用符を含む）。
    pub fn render(&self, body: &str) -> String {
        let encoded = self.encode_body(body);
        match self {
            QuoteStyle::DoubleQuoted => format!("\"{}\"", encoded),
            QuoteStyle::SingleQuoted => format!("'{}'", encoded),
            QuoteStyle::Block => format!("'''{}'''", encoded),
            QuoteStyle::Unquoted => encoded,
        }
    }

    /// このスタイルが使うトークン種別。
    pub fn syntax_kind(&self) -> SyntaxKind {
        match self {
            QuoteStyle::DoubleQuoted | QuoteStyle::SingleQuoted => SyntaxKind::String,
            QuoteStyle::Block => SyntaxKind::BlockString,
            QuoteStyle::Unquoted => SyntaxKind::UnquotedString,
        }
    }
}

/// 1 個のコメントの生テキストと本文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentTrivia {
    /// コメントの書き方。
    pub kind: CommentKind,
    /// 記号を含む生テキスト。
    pub text: String,
    /// 記号と前後の空白を除いた本文。
    pub body: String,
}

/// 要素の前後にある意味を持たないテキスト。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trivia {
    /// 空白・タブ。
    Whitespace(String),
    /// 改行。
    Newline {
        /// 改行コードの種類。
        kind: NewlineKind,
        /// 実際の改行テキスト。
        text: String,
    },
    /// コメント。
    Comment(CommentTrivia),
    /// 区切りのカンマ。
    Comma(String),
}

impl Trivia {
    /// コメントか。
    pub fn is_comment(&self) -> bool {
        matches!(self, Self::Comment(_))
    }

    /// 改行ならその種類。
    pub fn newline_kind(&self) -> Option<NewlineKind> {
        match self {
            Self::Newline { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    /// 構文木へ戻すための green 要素。
    pub fn to_green_element(&self) -> GreenElement {
        match self {
            Trivia::Whitespace(text) => SyntaxKind::Whitespace.create_token(text.as_str()).into(),
            Trivia::Newline { text, .. } => SyntaxKind::Newline.create_token(text.as_str()).into(),
            Trivia::Comment(comment) => {
                let kind = match comment.kind {
                    CommentKind::Line => SyntaxKind::LineComment,
                    CommentKind::Hash => SyntaxKind::HashComment,
                    CommentKind::Block => SyntaxKind::BlockComment,
                };
                let token: rowan::GreenToken = kind.create_token(comment.text.as_str());
                let node = rowan::GreenNode::new(SyntaxKind::Comment.into(), vec![token.into()]);
                NodeOrToken::Node(node)
            }
            Trivia::Comma(text) => SyntaxKind::Comma.create_token(text.as_str()).into(),
        }
    }
}

/// 要素の配置（深さ・末尾か・トリビアを持つか）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Layout {
    /// ネストの深さ。
    pub depth: usize,
    /// 兄弟の中で最後の要素か。
    pub is_last: bool,
    /// 前後のトリビアをこの要素が持つか。
    pub owns_trivia: bool,
}

/// 文字列の値（生テキスト・本文・スタイル）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringValue {
    raw: String,
    body: String,
    quote_style: QuoteStyle,
}

impl StringValue {
    /// 引用符を含む生テキスト。
    pub fn raw_text(&self) -> &str {
        &self.raw
    }

    /// 引用符を外した本文。
    pub fn body_text(&self) -> &str {
        &self.body
    }

    /// この値の文字列スタイル。
    pub fn quote_style(&self) -> QuoteStyle {
        self.quote_style
    }
}

/// スカラー値（文字列・整数・浮動小数・真偽値・null）。
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValue {
    /// 文字列。
    String(StringValue),
    /// 整数。
    Integer {
        /// 元のテキスト。
        raw: String,
        /// 解釈した値（i64 の範囲外なら `None`）。
        parsed: Option<i64>,
    },
    /// 浮動小数。
    Float {
        /// 元のテキスト。
        raw: String,
        /// 解釈した値（数値でなければ `None`）。
        parsed: Option<f64>,
    },
    /// 真偽値。
    Bool(bool),
    /// null。
    Null,
}

impl ScalarValue {
    /// 生テキスト（文字列は引用符を含む）。
    pub fn raw_text(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.raw_text()),
            Self::Integer { raw, .. } | Self::Float { raw, .. } => Some(raw),
            Self::Bool(true) => Some("true"),
            Self::Bool(false) => Some("false"),
            Self::Null => Some("null"),
        }
    }

    /// 文字列ならスタイル。
    pub fn quote_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::String(s) => Some(s.quote_style()),
            _ => None,
        }
    }
}

/// `Value` の中身（コンテナまたはスカラー）。
#[derive(Debug, Clone, PartialEq)]
pub enum ValueContent {
    /// オブジェクト。
    Object(Object),
    /// 配列。
    Array(Array),
    /// スカラー。
    Scalar(ScalarValue),
}

impl ValueContent {
    /// スカラーなら生テキスト。
    pub fn raw_text(&self) -> Option<&str> {
        match self {
            Self::Scalar(s) => s.raw_text(),
            _ => None,
        }
    }

    /// スカラー文字列ならスタイル。
    pub fn quote_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::Scalar(s) => s.quote_style(),
            _ => None,
        }
    }
}

/// 構文木のノードを意味ASTの型へ射影する。
pub trait AstNode: Sized {
    /// この種別をその型として扱えるか。
    fn can_cast(kind: SyntaxKind) -> bool;
    /// ノードをその型として読む。
    fn cast(node: RedNode) -> Result<Self, HjsonEditError>;
    /// 元の構文ノード。
    fn syntax(&self) -> &RedNode;

    /// 元の構文ノード。
    fn node(&self) -> &RedNode {
        self.syntax()
    }
}

/// 文書のルート。
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    syntax: RedNode,
    /// 要素の直前のトリビア。
    pub leading_trivia: Vec<Trivia>,
    /// 要素の直後のトリビア。
    pub trailing_trivia: Vec<Trivia>,
    /// 配置情報。
    pub layout: Layout,
    /// 本文が空のときの挿入位置。
    pub body_insert_index: usize,
}

/// オブジェクト（`{ ... }` またはブレースレス）。
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    syntax: RedNode,
    /// 要素の直前のトリビア。
    pub leading_trivia: Vec<Trivia>,
    /// 要素の直後のトリビア。
    pub trailing_trivia: Vec<Trivia>,
    /// 区切りの書き方。
    pub delimiter_style: DelimiterStyle,
    /// 配置情報。
    pub layout: Layout,
    empty_body_insert_index: usize,
    body_end_index: usize,
}

/// コンテナの区切りの書き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterStyle {
    /// `{ ... }` のオブジェクト。
    Braced,
    /// ブレースレス（文書直下のキー行）。
    Braceless,
}

/// コンテナ内の要素（メンバーまたは配列要素）1 つの位置。
///
/// 位置は「コンテナの子要素の並び」での index 範囲で表す。挿入・削除・コメント操作が
/// 同じ位置計算を共有できるよう、要素本体だけでなく前後のトリビアと、
/// 要素の行を終わらせる改行の位置も保持する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSpan {
    lead: Range<usize>,
    elem: Range<usize>,
    trail: Range<usize>,
    at_line_start: bool,
    line_break: Option<usize>,
}

impl SlotSpan {
    /// 要素本体（`Member` または `Value`）の範囲。
    pub fn elem(&self) -> Range<usize> {
        self.elem.clone()
    }

    /// 要素の直前にあるトリビア（インデント・改行・コメント）の範囲。
    pub fn lead(&self) -> Range<usize> {
        self.lead.clone()
    }

    /// 要素の直後にあるトリビア（行末コメントと、行を終わらせる改行まで）の範囲。
    pub fn trail(&self) -> Range<usize> {
        self.trail.clone()
    }

    /// この要素の直前へ挿入する位置が行頭か。
    ///
    /// 真のとき、挿入する要素はそれだけで 1 行を占める（行末の改行が必要になる）。
    pub fn at_line_start(&self) -> bool {
        self.at_line_start
    }

    /// 要素の行を終わらせる改行の位置。要素の後に閉じ区切りが続く場合は `None`。
    pub fn line_break(&self) -> Option<usize> {
        self.line_break
    }

    /// 直前のトリビアを含む要素全体の範囲。要素の削除に使う。
    pub fn span(&self) -> Range<usize> {
        self.lead.start..self.trail.end
    }

    /// この要素の直前へ挿入する位置。
    pub fn insert_before(&self) -> usize {
        self.lead.start
    }
}

/// 本文が空のコンテナへ最初の要素を挿入する位置と、前後の改行の要否。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyInsertion {
    index: usize,
    terminate_line: bool,
    start_new_line: bool,
}

impl BodyInsertion {
    /// 挿入位置。
    pub fn index(&self) -> usize {
        self.index
    }

    /// 挿入する要素の後ろに改行が必要か（閉じ区切りが独立した行にある）。
    pub fn terminate_line(&self) -> bool {
        self.terminate_line
    }

    /// 挿入する要素の前に改行が必要か（同じ行に先行する内容がある）。
    pub fn start_new_line(&self) -> bool {
        self.start_new_line
    }
}

/// 配列（`[ ... ]`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    syntax: RedNode,
    /// 要素の直前のトリビア。
    pub leading_trivia: Vec<Trivia>,
    /// 要素の直後のトリビア。
    pub trailing_trivia: Vec<Trivia>,
    /// 区切りの書き方。
    pub delimiter_style: DelimiterStyle,
    /// 配置情報。
    pub layout: Layout,
    empty_body_insert_index: usize,
    body_end_index: usize,
}

/// オブジェクトのメンバー（キー・区切り・値とその前後のトリビア）。
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    syntax: RedNode,
    /// 要素の直前のトリビア。
    pub leading_trivia: Vec<Trivia>,
    /// キーと `:` の間のトリビア。
    pub before_colon_trivia: Vec<Trivia>,
    /// `:` と値の間のトリビア。
    pub after_colon_trivia: Vec<Trivia>,
    /// 要素の直後のトリビア。
    pub trailing_trivia: Vec<Trivia>,
    /// 配置情報。
    pub layout: Layout,
}

/// コメントのノード。
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    syntax: RedNode,
    /// コメントの内容。
    pub trivia: CommentTrivia,
}

/// メンバーのキー。
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    syntax: RedNode,
}

/// 値（コンテナまたはスカラーを包む）。
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    syntax: RedNode,
    /// 配置情報。
    pub layout: Layout,
}

/// 配列の要素。
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayItem {
    syntax: RedNode,
    /// 要素の直前のトリビア。
    pub leading_trivia: Vec<Trivia>,
    /// 要素の直後のトリビア。
    pub trailing_trivia: Vec<Trivia>,
    /// 配置情報。
    pub layout: Layout,
}

/// 文書直下の項目。
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentItem {
    /// オブジェクト。
    Object(Object),
    /// 配列。
    Array(Array),
    /// 値。
    Value(Value),
}

impl DocumentItem {
    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        match self {
            Self::Object(object) => object.detected_string_style(),
            Self::Array(array) => array.detected_string_style(),
            Self::Value(value) => value.detected_string_style(),
        }
    }
}

impl AstNode for Document {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Root
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Root,
                found: node.kind(),
            });
        }

        let layout = collect_layout(&node, true);
        let leading_trivia = collect_root_leading_trivia(&node);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia: leading_trivia.clone(),
            trailing_trivia: collect_root_trailing_trivia(&node),
            layout,
            body_insert_index: leading_trivia.len(),
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Document {
    /// 文書直下の項目（空の文書なら `None`）。
    pub fn item(&self) -> Option<DocumentItem> {
        match self.syntax.kind() {
            SyntaxKind::Root => {
                let child = self
                    .syntax
                    .children_with_tokens()
                    .nth(self.body_insert_index)?;
                match child {
                    NodeOrToken::Node(node) => match node.kind() {
                        SyntaxKind::Object => Some(DocumentItem::Object(Object::cast(node).ok()?)),
                        SyntaxKind::Array => Some(DocumentItem::Array(Array::cast(node).ok()?)),
                        SyntaxKind::Value => Some(DocumentItem::Value(Value::cast(node).ok()?)),
                        _ => None,
                    },
                    NodeOrToken::Token(_) => None,
                }
            }
            SyntaxKind::Object => Some(DocumentItem::Object(
                Object::cast(self.syntax.clone()).ok()?,
            )),
            SyntaxKind::Array => Some(DocumentItem::Array(Array::cast(self.syntax.clone()).ok()?)),
            SyntaxKind::Value => Some(DocumentItem::Value(Value::cast(self.syntax.clone()).ok()?)),
            _ => None,
        }
    }

    /// 本文が空のときの挿入位置。
    pub fn body_insert_index(&self) -> usize {
        self.body_insert_index
    }

    /// 中身がオブジェクトならその射影。
    pub fn object(&self) -> Option<Object> {
        match self.item()? {
            DocumentItem::Object(object) => Some(object),
            DocumentItem::Value(value) => match value.content().ok()? {
                ValueContent::Object(object) => Some(object),
                _ => None,
            },
            _ => None,
        }
    }

    /// 中身が配列ならその射影。
    pub fn array(&self) -> Option<Array> {
        match self.item()? {
            DocumentItem::Array(array) => Some(array),
            DocumentItem::Value(value) => match value.content().ok()? {
                ValueContent::Array(array) => Some(array),
                _ => None,
            },
            _ => None,
        }
    }

    /// 中身がスカラーならその射影。
    pub fn value(&self) -> Option<Value> {
        match self.item()? {
            DocumentItem::Value(value) => Some(value),
            _ => None,
        }
    }

    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.item().and_then(|item| item.detected_string_style())
    }
}

impl AstNode for Object {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Object
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: node.kind(),
            });
        }

        let owns_trivia = !matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let is_root_child = matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);
        let delimiter_style = detect_delimiter_style(&node, SyntaxKind::LBrace);
        let leading_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_leading_trivia(
                &node,
                SyntaxKind::Member,
                SyntaxKind::LBrace,
                SyntaxKind::RBrace,
            )
        };
        let trailing_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_trailing_trivia(
                &node,
                SyntaxKind::Member,
                SyntaxKind::LBrace,
                SyntaxKind::RBrace,
            )
        };
        let empty_body_insert_index = node
            .children_with_tokens()
            .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBrace))
            .unwrap_or_else(|| node.children_with_tokens().count());
        let body_end_index = empty_body_insert_index;

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            trailing_trivia,
            delimiter_style,
            layout,
            empty_body_insert_index,
            body_end_index,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Object {
    /// メンバーの一覧。
    pub fn members(&self) -> Vec<Member> {
        self.syntax
            .children()
            .filter(|child| child.kind() == SyntaxKind::Member)
            .filter_map(|child| Member::cast(child).ok())
            .collect()
    }

    /// メンバーの位置を、コンテナ内の並び順に返す。
    pub fn slots(&self) -> Vec<SlotSpan> {
        container_slots(&self.syntax, SyntaxKind::Member)
    }

    /// 本文が空のときに、最初のメンバーを挿入する位置。
    pub fn body_insertion(&self) -> BodyInsertion {
        body_insertion(&self.syntax, self.delimiter_style, SyntaxKind::RBrace)
    }

    /// 末尾コメントの手前へ追加するときの挿入位置。
    pub fn append_member_index_before_footer(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
            return children
                .iter()
                .rposition(|child| matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Member))
                .map(|idx| idx + 1)
                .unwrap_or(self.empty_body_insert_index);
        }

        self.body_end_index
    }

    /// コメントも含めた本文の末尾へ追加するときの挿入位置。
    pub fn append_member_index_after_comments(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            return self.syntax.children_with_tokens().count();
        }

        self.body_end_index
    }

    /// キーが一致する最初のメンバー。
    pub fn member_by_key(&self, key: &str) -> Option<Member> {
        self.members()
            .into_iter()
            .find(|member| member.member_key().as_deref() == Some(key))
    }

    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.members()
            .into_iter()
            .find_map(|member| member.detected_string_style())
    }
}

impl AstNode for Array {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Array
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: node.kind(),
            });
        }

        let owns_trivia = !matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let is_root_child = matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);
        let delimiter_style = detect_delimiter_style(&node, SyntaxKind::LBracket);
        let leading_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_leading_trivia(
                &node,
                SyntaxKind::Value,
                SyntaxKind::LBracket,
                SyntaxKind::RBracket,
            )
        };
        let trailing_trivia = if is_root_child {
            Vec::new()
        } else {
            collect_container_trailing_trivia(
                &node,
                SyntaxKind::Value,
                SyntaxKind::LBracket,
                SyntaxKind::RBracket,
            )
        };
        let empty_body_insert_index = node
            .children_with_tokens()
            .position(|child| matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::RBracket))
            .unwrap_or_else(|| node.children_with_tokens().count());
        let body_end_index = empty_body_insert_index;

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            trailing_trivia,
            delimiter_style,
            layout,
            empty_body_insert_index,
            body_end_index,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Array {
    /// 要素の一覧。
    pub fn items(&self) -> Vec<ArrayItem> {
        self.syntax
            .children()
            .filter(|child| child.kind() == SyntaxKind::Value)
            .filter_map(|child| ArrayItem::cast(child).ok())
            .collect()
    }

    /// 要素の値の一覧。
    pub fn values(&self) -> Vec<Value> {
        self.items()
            .into_iter()
            .filter_map(|item| item.value().ok())
            .collect()
    }

    /// 配列要素の位置を、コンテナ内の並び順に返す。
    pub fn slots(&self) -> Vec<SlotSpan> {
        container_slots(&self.syntax, SyntaxKind::Value)
    }

    /// 本文が空のときに、最初の要素を挿入する位置。
    pub fn body_insertion(&self) -> BodyInsertion {
        body_insertion(&self.syntax, self.delimiter_style, SyntaxKind::RBracket)
    }

    /// 末尾コメントの手前へ追加するときの挿入位置。
    pub fn append_value_index_before_footer(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            let children = self.syntax.children_with_tokens().collect::<Vec<_>>();
            return children
                .iter()
                .rposition(|child| matches!(child, NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value))
                .map(|idx| idx + 1)
                .unwrap_or(self.empty_body_insert_index);
        }

        self.body_end_index
    }

    /// コメントも含めた本文の末尾へ追加するときの挿入位置。
    pub fn append_value_index_after_comments(&self) -> usize {
        if self.delimiter_style == DelimiterStyle::Braceless {
            return self.syntax.children_with_tokens().count();
        }

        self.body_end_index
    }

    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.values()
            .into_iter()
            .find_map(|item| item.detected_string_style())
    }
}

impl AstNode for Member {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Member
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Member,
                found: node.kind(),
            });
        }

        let layout = collect_layout(&node, true);
        let (before_colon_trivia, after_colon_trivia) = collect_member_middle_trivia(&node);
        let leading_trivia = collect_leading_trivia(&node, layout.owns_trivia);
        let trailing_trivia = collect_trailing_trivia(&node, layout.owns_trivia);

        Ok(Self {
            syntax: node.clone(),
            leading_trivia,
            before_colon_trivia,
            after_colon_trivia,
            trailing_trivia,
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Member {
    /// キー。
    pub fn key(&self) -> Option<Key> {
        self.syntax
            .children()
            .find(|child| child.kind() == SyntaxKind::Key)
            .and_then(|child| Key::cast(child).ok())
    }

    /// 値。
    pub fn value(&self) -> Option<Value> {
        self.syntax
            .children()
            .find(|child| child.kind() == SyntaxKind::Value)
            .and_then(|child| Value::cast(child).ok())
    }

    /// キーの文字列（引用符を外した本文）。
    pub fn member_key(&self) -> Option<String> {
        self.key().and_then(|key| key.body_text())
    }

    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.value().and_then(|value| value.detected_string_style())
    }

    /// 要素全体（前後のトリビアを含む）の範囲。
    pub fn span_range(&self) -> Range<usize> {
        let start = self
            .syntax
            .index()
            .saturating_sub(self.leading_trivia.len());
        let end = self.syntax.index() + 1 + self.trailing_trivia.len();
        start..end
    }

    /// この要素の直前へ挿入する位置。
    pub fn insert_before_index(&self) -> usize {
        self.span_range().start
    }
}

impl AstNode for Comment {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Comment
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Comment,
                found: node.kind(),
            });
        }

        let trivia = collect_comment_trivia(&node).ok_or(HjsonEditError::UnexpectedStructure {
            expected: SyntaxKind::Comment,
            found: node.kind(),
        })?;

        Ok(Self {
            syntax: node,
            trivia,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Comment {
    /// コメントの書き方。
    pub fn kind(&self) -> CommentKind {
        self.trivia.kind
    }

    /// 記号を含む生テキスト。
    pub fn raw_text(&self) -> &str {
        &self.trivia.text
    }

    /// 記号を除いた本文。
    pub fn body_text(&self) -> &str {
        &self.trivia.body
    }
}

impl AstNode for Key {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Key
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Key,
                found: node.kind(),
            });
        }

        Ok(Self { syntax: node })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Key {
    /// 文字列トークンの種別とテキスト。
    pub fn token_text_and_kind(&self) -> Option<(SyntaxKind, String)> {
        self.syntax
            .children_with_tokens()
            .find_map(|elem| match elem {
                NodeOrToken::Token(token)
                    if matches!(
                        token.kind(),
                        SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString
                    ) =>
                {
                    Some((token.kind(), token.text().to_string()))
                }
                _ => None,
            })
    }

    /// 文字列の生テキスト（引用符を含む）。
    pub fn raw_text(&self) -> Option<String> {
        self.token_text_and_kind().map(|(_, text)| text)
    }

    /// 引用符を外した本文。
    pub fn body_text(&self) -> Option<String> {
        self.syntax
            .children_with_tokens()
            .find_map(|elem| match elem {
                NodeOrToken::Token(token) => {
                    let text = token.text().to_string();
                    let style = match token.kind() {
                        SyntaxKind::String => {
                            if text.starts_with('\'') {
                                QuoteStyle::SingleQuoted
                            } else {
                                QuoteStyle::DoubleQuoted
                            }
                        }
                        SyntaxKind::BlockString => QuoteStyle::Block,
                        SyntaxKind::UnquotedString => QuoteStyle::Unquoted,
                        // HJSONではキーは常に文字列扱いのため、パーサがキーとして
                        // 許容する数値・真偽値・nullトークンもそのまま文字列として返す
                        SyntaxKind::Integer
                        | SyntaxKind::Float
                        | SyntaxKind::True
                        | SyntaxKind::False
                        | SyntaxKind::Null => return Some(text),
                        _ => return None,
                    };
                    Some(style.decode_body(&text, token_column(&token)))
                }
                _ => None,
            })
    }
}

impl AstNode for Value {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Value
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            });
        }

        let owns_trivia = !matches!(
            node.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::Root)
        );
        let layout = collect_layout(&node, owns_trivia);

        Ok(Self {
            syntax: node.clone(),
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl Value {
    /// 中身（コンテナまたはスカラー）。
    pub fn content(&self) -> Result<ValueContent, HjsonEditError> {
        let child = self
            .syntax
            .children_with_tokens()
            .find(|child| match child {
                NodeOrToken::Node(node) => !node.kind().is_trivia(),
                NodeOrToken::Token(token) => !token.kind().is_trivia(),
            })
            .ok_or(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: self.syntax.kind(),
            })?;

        Ok(match child {
            NodeOrToken::Node(node) => match node.kind() {
                SyntaxKind::Object => ValueContent::Object(Object::cast(node)?),
                SyntaxKind::Array => ValueContent::Array(Array::cast(node)?),
                other => {
                    return Err(HjsonEditError::UnexpectedStructure {
                        expected: SyntaxKind::Value,
                        found: other,
                    });
                }
            },
            NodeOrToken::Token(token) => {
                let raw = token.text().to_string();
                match token.kind() {
                    SyntaxKind::String => {
                        let style = if raw.starts_with('\'') {
                            QuoteStyle::SingleQuoted
                        } else {
                            QuoteStyle::DoubleQuoted
                        };
                        ValueContent::Scalar(ScalarValue::String(StringValue {
                            raw: raw.clone(),
                            body: style.decode_body(&raw, token_column(&token)),
                            quote_style: style,
                        }))
                    }
                    SyntaxKind::BlockString => {
                        let style = QuoteStyle::Block;
                        ValueContent::Scalar(ScalarValue::String(StringValue {
                            raw: raw.clone(),
                            body: style.decode_body(&raw, token_column(&token)),
                            quote_style: style,
                        }))
                    }
                    SyntaxKind::UnquotedString => {
                        let style = QuoteStyle::Unquoted;
                        ValueContent::Scalar(ScalarValue::String(StringValue {
                            raw: raw.clone(),
                            body: style.decode_body(&raw, 0),
                            quote_style: style,
                        }))
                    }
                    SyntaxKind::Integer => ValueContent::Scalar(ScalarValue::Integer {
                        raw: raw.clone(),
                        parsed: raw.parse::<i64>().ok(),
                    }),
                    SyntaxKind::Float => ValueContent::Scalar(ScalarValue::Float {
                        raw: raw.clone(),
                        parsed: raw.parse::<f64>().ok(),
                    }),
                    SyntaxKind::True => ValueContent::Scalar(ScalarValue::Bool(true)),
                    SyntaxKind::False => ValueContent::Scalar(ScalarValue::Bool(false)),
                    SyntaxKind::Null => ValueContent::Scalar(ScalarValue::Null),
                    other => {
                        return Err(HjsonEditError::UnexpectedStructure {
                            expected: SyntaxKind::Value,
                            found: other,
                        });
                    }
                }
            }
        })
    }

    /// 文字列ならそのスタイル。
    pub fn quote_style(&self) -> Option<QuoteStyle> {
        self.content().ok()?.quote_style()
    }

    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        match self.content().ok()? {
            ValueContent::Scalar(ScalarValue::String(string)) => Some(string.quote_style()),
            ValueContent::Object(object) => object.detected_string_style(),
            ValueContent::Array(array) => array.detected_string_style(),
            ValueContent::Scalar(_) => None,
        }
    }
}

impl AstNode for ArrayItem {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Value
    }

    fn cast(node: RedNode) -> Result<Self, HjsonEditError> {
        if !Self::can_cast(node.kind()) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            });
        }

        let parent_kind = node.parent().map(|parent| parent.kind());
        if !matches!(parent_kind, Some(SyntaxKind::Array)) {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: parent_kind.unwrap_or(SyntaxKind::Root),
            });
        }

        let layout = collect_layout(&node, true);
        let leading_trivia = collect_leading_trivia(&node, true);
        let trailing_trivia = collect_trailing_trivia(&node, true);

        Ok(Self {
            syntax: node,
            leading_trivia,
            trailing_trivia,
            layout,
        })
    }

    fn syntax(&self) -> &RedNode {
        &self.syntax
    }
}

impl ArrayItem {
    /// 値。
    pub fn value(&self) -> Result<Value, HjsonEditError> {
        Value::cast(self.syntax.clone())
    }

    /// 使われている文字列スタイル。
    pub fn detected_string_style(&self) -> Option<QuoteStyle> {
        self.value().ok()?.detected_string_style()
    }

    /// 要素全体（前後のトリビアを含む）の範囲。
    pub fn span_range(&self) -> Range<usize> {
        let start = self
            .syntax
            .index()
            .saturating_sub(self.leading_trivia.len());
        let end = self.syntax.index() + 1 + self.trailing_trivia.len();
        start..end
    }

    /// この要素の直前へ挿入する位置。
    pub fn insert_before_index(&self) -> usize {
        self.span_range().start
    }
}

fn collect_comment_trivia(node: &RedNode) -> Option<CommentTrivia> {
    node.children_with_tokens().find_map(|elem| match elem {
        NodeOrToken::Token(token) => {
            let text = token.text().to_string();
            let (kind, body) = match token.kind() {
                SyntaxKind::LineComment => {
                    (CommentKind::Line, CommentKind::Line.decode_body(&text))
                }
                SyntaxKind::HashComment => {
                    (CommentKind::Hash, CommentKind::Hash.decode_body(&text))
                }
                SyntaxKind::BlockComment => {
                    (CommentKind::Block, CommentKind::Block.decode_body(&text))
                }
                _ => return None,
            };
            Some(CommentTrivia { kind, text, body })
        }
        NodeOrToken::Node(_) => None,
    })
}

fn collect_container_leading_trivia(
    node: &RedNode,
    item_kind: SyntaxKind,
    opening_token_kind: SyntaxKind,
    closing_token_kind: SyntaxKind,
) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let open_index = children.iter().position(
        |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == opening_token_kind),
    );

    let start = open_index.map_or(0, |idx| idx + 1);
    let end = children
        .iter()
        .rposition(
            |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == closing_token_kind),
        )
        .unwrap_or(children.len());

    let first_item_index = children[start..end]
        .iter()
        .position(|elem| matches!(elem, NodeOrToken::Node(node) if node.kind() == item_kind))
        .map(|relative| start + relative);

    let leading_end = first_item_index.unwrap_or(end);
    collect_trivia_slice(&children[start..leading_end])
}

/// コンテナの子要素（メンバーまたは配列要素）の位置を、子の並び順に返す。
fn container_slots(node: &RedNode, element_kind: SyntaxKind) -> Vec<SlotSpan> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let mut slots = Vec::new();

    for (index, child) in children.iter().enumerate() {
        if !matches!(child, NodeOrToken::Node(element) if element.kind() == element_kind) {
            continue;
        }

        let lead = leading_trivia_start(&children, index);
        let trail_end = trailing_trivia_end(&children, index + 1);
        slots.push(SlotSpan {
            lead: lead..index,
            elem: index..index + 1,
            trail: index + 1..trail_end,
            at_line_start: follows_line_break(&children, lead),
            line_break: first_newline_index(&children, index + 1..trail_end),
        });
    }

    slots
}

/// 本文が空のコンテナへ最初の要素を挿入する位置を返す。
fn body_insertion(
    node: &RedNode,
    delimiter_style: DelimiterStyle,
    closing_kind: SyntaxKind,
) -> BodyInsertion {
    let children = node.children_with_tokens().collect::<Vec<_>>();

    if delimiter_style == DelimiterStyle::Braceless {
        // 閉じ区切りが無いので、本文の末尾（末尾トリビアの先頭）へ入れる
        let index = children
            .iter()
            .rposition(|child| match child {
                NodeOrToken::Token(token) => !token.kind().is_trivia(),
                NodeOrToken::Node(node) => node.kind() != SyntaxKind::Comment,
            })
            .map_or(0, |position| position + 1);

        // 本文が空のときは、文書先頭のコメントなどが同じ行に残っている場合がある
        let start_new_line = index == 0 && has_same_line_content_before(node);

        return BodyInsertion {
            index,
            terminate_line: true,
            start_new_line,
        };
    }

    let index = children
        .iter()
        .position(
            |child| matches!(child, NodeOrToken::Token(token) if token.kind() == closing_kind),
        )
        .unwrap_or(children.len());

    // 閉じ区切りの直前（空白を飛ばして）が改行なら、要素は独立した行を占める
    let terminate_line = follows_line_break(&children, index);
    let start_new_line = !terminate_line && children[..index].iter().any(is_comment_element);

    BodyInsertion {
        index,
        terminate_line,
        start_new_line,
    }
}

/// コメントの要素か。
fn is_comment_element(element: &NodeOrToken<RedNode, RedToken>) -> bool {
    match element {
        NodeOrToken::Node(node) => node.kind() == SyntaxKind::Comment,
        NodeOrToken::Token(token) => token.kind().is_comment(),
    }
}

/// ノードの直前（親の子要素の並び）に、同じ行の内容があるか。
fn has_same_line_content_before(node: &RedNode) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let children = parent.children_with_tokens().collect::<Vec<_>>();
    let Some(position) = children
        .iter()
        .position(|child| matches!(child, NodeOrToken::Node(child) if child == node))
    else {
        return false;
    };

    children[..position]
        .iter()
        .rev()
        .find(|child| {
            !matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace)
        })
        .is_some_and(|child| {
            !matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline)
        })
}

/// `index` の直前（空白を飛ばして）に改行があるか。
///
/// 真なら、その位置へ挿入する要素は行頭から始まり、行末の改行が必要になる。
fn follows_line_break(children: &[NodeOrToken<RedNode, RedToken>], index: usize) -> bool {
    children[..index]
        .iter()
        .rev()
        .find(|child| {
            !matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace)
        })
        .is_some_and(|child| {
            matches!(child, NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline)
        })
}

/// 範囲内で最初に現れる改行トークンの位置。
fn first_newline_index(
    children: &[NodeOrToken<RedNode, RedToken>],
    range: Range<usize>,
) -> Option<usize> {
    range.into_iter().find(|index| {
        matches!(&children[*index], NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline)
    })
}

fn trailing_trivia_end(children: &[NodeOrToken<RedNode, RedToken>], start: usize) -> usize {
    let mut cursor = start;
    while cursor < children.len() {
        let Some(item) = trivia_from_element(&children[cursor]) else {
            break;
        };
        cursor += 1;
        if matches!(item, Trivia::Newline { .. }) {
            break;
        }
    }
    cursor
}

fn collect_container_trailing_trivia(
    node: &RedNode,
    item_kind: SyntaxKind,
    opening_token_kind: SyntaxKind,
    closing_token_kind: SyntaxKind,
) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let start = children
        .iter()
        .position(
            |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == opening_token_kind),
        )
        .map_or(0, |idx| idx + 1);
    let end = children
        .iter()
        .rposition(
            |elem| matches!(elem, NodeOrToken::Token(token) if token.kind() == closing_token_kind),
        )
        .unwrap_or(children.len());

    let last_item_index = children[start..end]
        .iter()
        .rposition(|elem| matches!(elem, NodeOrToken::Node(node) if node.kind() == item_kind))
        .map(|relative| start + relative);

    let trailing_start = match last_item_index {
        Some(idx) => trailing_trivia_end(&children, idx + 1),
        // 要素が 1 つも無いときは、間に挟まるコメントは前側に属する
        // （そうしないと leading と trailing が同じコメントを指し、整形で重複する）
        None => end,
    };
    collect_trivia_slice(&children[trailing_start..end])
}

fn collect_trivia_slice(slice: &[NodeOrToken<RedNode, RedToken>]) -> Vec<Trivia> {
    let mut trivia = Vec::new();
    for elem in slice {
        if let Some(item) = trivia_from_element(elem) {
            trivia.push(item);
        }
    }
    trivia
}

fn detect_delimiter_style(node: &RedNode, opening_token_kind: SyntaxKind) -> DelimiterStyle {
    let is_braced = node
        .children_with_tokens()
        .find_map(|elem| match elem {
            NodeOrToken::Token(token) if token.kind().is_trivia() => None,
            NodeOrToken::Token(token) => Some(token.kind() == opening_token_kind),
            NodeOrToken::Node(_) => Some(false),
        })
        .unwrap_or(false);

    if is_braced {
        DelimiterStyle::Braced
    } else {
        DelimiterStyle::Braceless
    }
}

fn collect_leading_trivia(node: &RedNode, enabled: bool) -> Vec<Trivia> {
    if !enabled {
        return Vec::new();
    }

    // 親の子をすべて並べると要素数に対して二次関数的になるため、兄弟を後方へたどる
    let mut items: Vec<Trivia> = Vec::new();
    let mut cursor = node.prev_sibling_or_token();
    while let Some(element) = cursor {
        // コメントは Comment ノードなので `is_trivia` では判定できない
        let Some(item) = trivia_from_element(&element) else {
            break;
        };
        if matches!(item, Trivia::Comma(_)) {
            break;
        }
        items.push(item);
        cursor = element.prev_sibling_or_token();
    }
    items.reverse();

    // 直前の要素の行末の改行は、その要素の trailing に属する
    if let Some(position) = items
        .iter()
        .position(|item| matches!(item, Trivia::Newline { .. }))
    {
        items.drain(..=position);
    }

    items
}

/// 指定 index の要素の直前にあるトリビアの開始 index。
///
/// カンマ・行を区切る改行・非トリビア要素で区切られた「最後の連続したトリビア列」の
/// 先頭を返す。`collect_leading_trivia` と同じ規則を共有するための唯一の実装。
fn leading_trivia_start(children: &[NodeOrToken<RedNode, RedToken>], index: usize) -> usize {
    let mut start = 0usize;
    let mut saw_boundary_newline = false;

    for (position, elem) in children.iter().enumerate().take(index) {
        let Some(item) = trivia_from_element(elem) else {
            start = position + 1;
            saw_boundary_newline = false;
            continue;
        };

        match item {
            Trivia::Comma(_) => {
                start = position + 1;
                saw_boundary_newline = false;
            }
            Trivia::Newline { .. } if !saw_boundary_newline => {
                start = position + 1;
                saw_boundary_newline = true;
            }
            _ => {}
        }
    }

    start
}

fn collect_root_leading_trivia(node: &RedNode) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let mut trivia = Vec::new();

    for elem in children.iter() {
        let Some(item) = trivia_from_element(elem) else {
            break;
        };

        match item {
            Trivia::Whitespace(text) => trivia.push(Trivia::Whitespace(text)),
            Trivia::Newline { kind, text } => trivia.push(Trivia::Newline { kind, text }),
            Trivia::Comment(comment) => trivia.push(Trivia::Comment(comment)),
            Trivia::Comma(_) => break,
        }
    }

    trivia
}

fn collect_member_middle_trivia(node: &RedNode) -> (Vec<Trivia>, Vec<Trivia>) {
    let mut before_colon = Vec::new();
    let mut after_colon = Vec::new();
    let mut seen_colon = false;

    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Key => continue,
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value => break,
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Colon => {
                seen_colon = true;
                continue;
            }
            other => {
                if let Some(item) = trivia_from_element(&other) {
                    if seen_colon {
                        after_colon.push(item);
                    } else {
                        before_colon.push(item);
                    }
                }
            }
        }
    }

    (before_colon, after_colon)
}

fn collect_trailing_trivia(node: &RedNode, enabled: bool) -> Vec<Trivia> {
    if !enabled {
        return Vec::new();
    }

    // 親の子をすべて並べると要素数に対して二次関数的になるため、兄弟を前方へたどる
    let mut items: Vec<Trivia> = Vec::new();
    let mut cursor = node.next_sibling_or_token();
    while let Some(element) = cursor {
        let Some(item) = trivia_from_element(&element) else {
            break;
        };
        let is_newline = matches!(item, Trivia::Newline { .. });
        items.push(item);
        if is_newline {
            break;
        }
        cursor = element.next_sibling_or_token();
    }

    items
}

fn collect_root_trailing_trivia(node: &RedNode) -> Vec<Trivia> {
    let children = node.children_with_tokens().collect::<Vec<_>>();
    let mut trivia = Vec::new();

    for elem in children.iter().rev() {
        let Some(item) = trivia_from_element(elem) else {
            break;
        };

        match item {
            Trivia::Whitespace(text) => trivia.push(Trivia::Whitespace(text)),
            Trivia::Newline { kind, text } => trivia.push(Trivia::Newline { kind, text }),
            Trivia::Comment(comment) => trivia.push(Trivia::Comment(comment)),
            Trivia::Comma(_) => break,
        }
    }

    trivia.reverse();
    trivia
}

fn collect_layout(node: &RedNode, owns_trivia: bool) -> Layout {
    let mut depth = 1usize;
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), SyntaxKind::Object | SyntaxKind::Array) {
            depth += 1;
        }
        current = parent.parent();
    }

    // 親の子をすべて走査すると要素数に対して二次関数的になるため、後続の兄弟だけを見る
    let mut next = node.next_sibling();
    let mut is_last = true;
    while let Some(sibling) = next {
        if trivia_from_element(&NodeOrToken::Node(sibling.clone())).is_none() {
            is_last = false;
            break;
        }
        next = sibling.next_sibling();
    }

    Layout {
        depth,
        is_last,
        owns_trivia,
    }
}

pub(crate) fn trivia_from_element(element: &NodeOrToken<RedNode, RedToken>) -> Option<Trivia> {
    match element {
        NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => {
            collect_comment_trivia(node).map(Trivia::Comment)
        }
        NodeOrToken::Token(token) => match token.kind() {
            SyntaxKind::Whitespace => Some(Trivia::Whitespace(token.text().to_string())),
            SyntaxKind::Newline => Some(Trivia::Newline {
                kind: NewlineKind::from_text(token.text()).unwrap_or_default(),
                text: token.text().to_string(),
            }),
            SyntaxKind::LineComment => {
                let text = token.text().to_string();
                Some(Trivia::Comment(CommentTrivia {
                    kind: CommentKind::Line,
                    text: text.clone(),
                    body: CommentKind::Line.decode_body(&text),
                }))
            }
            SyntaxKind::HashComment => {
                let text = token.text().to_string();
                Some(Trivia::Comment(CommentTrivia {
                    kind: CommentKind::Hash,
                    text: text.clone(),
                    body: CommentKind::Hash.decode_body(&text),
                }))
            }
            SyntaxKind::BlockComment => {
                let text = token.text().to_string();
                Some(Trivia::Comment(CommentTrivia {
                    kind: CommentKind::Block,
                    text: text.clone(),
                    body: CommentKind::Block.decode_body(&text),
                }))
            }
            SyntaxKind::Comma => Some(Trivia::Comma(token.text().to_string())),
            _ => None,
        },
        NodeOrToken::Node(_) => None,
    }
}

/// ノードの値から文字列スタイルを推定する。
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parser::Parser;
    use crate::syntax::{lexer::Lexer, RedNode, SyntaxKind};

    fn parse_and_find_node(src: &str, kind: SyntaxKind) -> RedNode {
        let tokens = Lexer::new(src).lex();
        let parsed = Parser::new(src, tokens.tokens).parse();
        let root = RedNode::new_root(parsed.green);
        root.descendants().find(|node| node.kind() == kind).unwrap()
    }

    fn element_text(element: &NodeOrToken<RedNode, RedToken>) -> String {
        match element {
            NodeOrToken::Node(node) => node.text().to_string(),
            NodeOrToken::Token(token) => token.text().to_string(),
        }
    }

    fn slice(node: &RedNode, range: std::ops::Range<usize>) -> String {
        node.children_with_tokens().collect::<Vec<_>>()[range]
            .iter()
            .map(element_text)
            .collect()
    }

    #[test]
    fn slots_split_members_into_lead_elem_trail() {
        let src = "{\n  a: 1,\n  // c\n  b: 2\n}\n";
        let node = parse_and_find_node(src, SyntaxKind::Object);
        let object = Object::cast(node.clone()).unwrap();
        let slots = object.slots();

        assert_eq!(slots.len(), 2);
        assert_eq!(slice(&node, slots[0].lead()), "  ");
        assert_eq!(slice(&node, slots[0].elem()), "a: 1");
        assert_eq!(slice(&node, slots[0].trail()), ",\n");
        assert_eq!(slice(&node, slots[1].lead()), "  // c\n  ");
        assert_eq!(slice(&node, slots[1].elem()), "b: 2");
        assert_eq!(slice(&node, slots[1].trail()), "\n");
        assert!(slots[0].at_line_start());
        assert!(slots[1].at_line_start());
        assert_eq!(slots[0].line_break(), Some(5));
        assert!(slots[1].line_break().is_some());
        assert_eq!(slice(&node, slots[0].span()), "  a: 1,\n");
        assert_eq!(slice(&node, slots[1].span()), "  // c\n  b: 2\n");
    }

    #[test]
    fn slots_split_array_items_into_lead_elem_trail() {
        let src = "[1, 2]";
        let node = parse_and_find_node(src, SyntaxKind::Array);
        let array = Array::cast(node.clone()).unwrap();
        let slots = array.slots();

        assert_eq!(slots.len(), 2);
        assert_eq!(slice(&node, slots[0].lead()), "");
        assert_eq!(slice(&node, slots[0].elem()), "1");
        assert_eq!(slice(&node, slots[0].trail()), ", ");
        assert_eq!(slice(&node, slots[1].lead()), " ");
        assert_eq!(slice(&node, slots[1].elem()), "2");
        assert_eq!(slice(&node, slots[1].trail()), "");
        assert!(!slots[0].at_line_start());
        assert!(!slots[1].at_line_start());
        assert_eq!(slots[1].line_break(), None);
    }

    #[test]
    fn slots_report_braceless_members_and_line_breaks() {
        let src = "a: 1\nb: 2\n";
        let node = parse_and_find_node(src, SyntaxKind::Object);
        let object = Object::cast(node.clone()).unwrap();
        let slots = object.slots();

        assert_eq!(slots.len(), 2);
        assert_eq!(slice(&node, slots[0].elem()), "a: 1");
        assert_eq!(slice(&node, slots[0].trail()), "\n");
        assert_eq!(slice(&node, slots[1].elem()), "b: 2");
        assert_eq!(slots[0].line_break(), Some(1));
        assert_eq!(slots[1].line_break(), Some(3));
        assert_eq!(slots[1].insert_before(), 2);
    }

    #[test]
    fn body_insertion_points_before_the_closing_delimiter() {
        let cases = [
            ("{}", 1, false),
            ("{  }", 2, false),
            ("{\n}", 2, true),
            ("{\n  }", 3, true),
        ];
        for (src, index, at_line_start) in cases {
            let object = Object::cast(parse_and_find_node(src, SyntaxKind::Object)).unwrap();
            let insertion = object.body_insertion();
            assert_eq!(
                (insertion.index(), insertion.terminate_line()),
                (index, at_line_start),
                "{src}"
            );
        }

        let cases = [("[]", 1, false), ("[\n]", 2, true)];
        for (src, index, at_line_start) in cases {
            let array = Array::cast(parse_and_find_node(src, SyntaxKind::Array)).unwrap();
            let insertion = array.body_insertion();
            assert_eq!(
                (insertion.index(), insertion.terminate_line()),
                (index, at_line_start),
                "{src}"
            );
        }
    }

    #[test]
    fn test_cast_value_node_to_mismatched_kinds_rejected() {
        let value_node = parse_and_find_node("123", SyntaxKind::Value);

        assert!(Object::cast(value_node.clone()).is_err());
        assert!(Array::cast(value_node.clone()).is_err());
        assert!(Document::cast(value_node.clone()).is_err());
        assert!(Member::cast(value_node.clone()).is_err());
        assert!(Key::cast(value_node.clone()).is_err());
    }

    #[test]
    fn test_cast_object_node_to_mismatched_kinds_rejected() {
        let object_node = parse_and_find_node("{}", SyntaxKind::Object);

        assert!(Value::cast(object_node.clone()).is_err());
        assert!(Array::cast(object_node.clone()).is_err());
    }

    #[test]
    fn test_cast_comment_node_to_value_rejected() {
        let comment_node = parse_and_find_node("// c", SyntaxKind::Comment);

        assert!(Value::cast(comment_node.clone()).is_err());
    }

    #[test]
    fn test_key_body_text_unquoted_string() {
        let key = parse_and_find_node("{ foo: \"a\" }", SyntaxKind::Key);
        assert_eq!(Key::cast(key).unwrap().body_text(), Some("foo".to_string()));
    }

    #[test]
    fn test_key_body_text_quoted_string() {
        let key = parse_and_find_node("{ \"quoted key\": \"a\" }", SyntaxKind::Key);
        assert_eq!(
            Key::cast(key).unwrap().body_text(),
            Some("quoted key".to_string())
        );
    }

    #[test]
    fn test_key_body_text_integer_is_treated_as_string() {
        let key = parse_and_find_node("{ 0: \"a\" }", SyntaxKind::Key);
        assert_eq!(Key::cast(key).unwrap().body_text(), Some("0".to_string()));
    }

    #[test]
    fn test_key_body_text_float_is_treated_as_string() {
        let key = parse_and_find_node("{ 1.5: \"a\" }", SyntaxKind::Key);
        assert_eq!(Key::cast(key).unwrap().body_text(), Some("1.5".to_string()));
    }

    #[test]
    fn test_key_body_text_boolean_is_treated_as_string() {
        let true_key = parse_and_find_node("{ true: \"a\" }", SyntaxKind::Key);
        assert_eq!(
            Key::cast(true_key).unwrap().body_text(),
            Some("true".to_string())
        );
        let false_key = parse_and_find_node("{ false: \"a\" }", SyntaxKind::Key);
        assert_eq!(
            Key::cast(false_key).unwrap().body_text(),
            Some("false".to_string())
        );
    }

    #[test]
    fn test_key_body_text_null_is_treated_as_string() {
        let key = parse_and_find_node("{ null: \"a\" }", SyntaxKind::Key);
        assert_eq!(
            Key::cast(key).unwrap().body_text(),
            Some("null".to_string())
        );
    }

    #[test]
    fn test_object_with_numeric_keys_is_addressable() {
        let root = parse_and_find_node("{ 0: \"a\", 1: \"b\" }", SyntaxKind::Object);
        let object = Object::cast(root).unwrap();
        assert_eq!(
            object.member_by_key("0").unwrap().member_key(),
            Some("0".to_string())
        );
        assert_eq!(
            object.member_by_key("1").unwrap().member_key(),
            Some("1".to_string())
        );
    }
}
