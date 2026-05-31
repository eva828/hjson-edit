/// HJSON構文解析器
///
/// Lexerのトークン列をGreen Treeに変換
use super::lexer::Token;
use super::SyntaxKind;
use crate::number::HjsonNumber;
use line_index::{LineIndex, TextSize};

/// 構文解析エラー（オフセットのみ保持）
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl ParseError {
    pub(crate) fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }

    pub fn format_with_line_index(&self, line_index: &LineIndex) -> String {
        let lc = line_index.line_col(TextSize::from(self.offset as u32));
        format!(
            "Error at line {}, column {}: {}",
            lc.line, lc.col, self.message
        )
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at offset {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

/// 構文解析結果
#[derive(Debug)]
pub struct ParseResult {
    pub green: rowan::GreenNode,
    pub errors: Vec<ParseError>,
}

/// 構文解析器
pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    builder: rowan::GreenNodeBuilder<'static>,
    errors: Vec<ParseError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarContext {
    Root,
    ObjectValue,
    ArrayItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueBoundary {
    Closed,
    Open,
}

struct ScalarScanner<'a> {
    tokens: &'a [Token],
    start: usize,
    context: ScalarContext,
}

impl<'a> ScalarScanner<'a> {
    fn new(tokens: &'a [Token], start: usize, context: ScalarContext) -> Self {
        Self {
            tokens,
            start,
            context,
        }
    }

    fn collect(&self) -> (String, usize) {
        let mut pos = self.start;
        let mut text = String::new();
        let mut last_non_ws_len = 0usize;
        let mut last_non_ws_pos = self.start;

        while let Some(token) = self.tokens.get(pos) {
            if self.is_terminator(token.kind, pos, &text) {
                break;
            }

            if self.is_collectable_token(token.kind) {
                text.push_str(&token.text);
                pos += 1;
                if token.kind != SyntaxKind::Whitespace {
                    last_non_ws_len = text.len();
                    last_non_ws_pos = pos;
                }
            } else {
                break;
            }
        }

        text.truncate(last_non_ws_len);
        (text, last_non_ws_pos - self.start)
    }

    fn is_terminator(&self, kind: SyntaxKind, pos: usize, text: &str) -> bool {
        match kind {
            SyntaxKind::Newline => true,
            SyntaxKind::RBrace if self.is_closing_rbrace(pos) => true,
            SyntaxKind::RBracket if self.is_closing_rbracket(pos) => true,
            SyntaxKind::Comma if self.is_separator_comma(pos) => true,
            SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                if self.starts_inline_comment_after_scalar(text, kind) =>
            {
                true
            }
            _ => false,
        }
    }

    fn is_collectable_token(&self, kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::Whitespace
                | SyntaxKind::UnquotedString
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null
                | SyntaxKind::String
                | SyntaxKind::BlockString
                | SyntaxKind::LineComment
                | SyntaxKind::HashComment
                | SyntaxKind::BlockComment
                | SyntaxKind::Comma
                | SyntaxKind::Colon
                | SyntaxKind::LBrace
                | SyntaxKind::LBracket
        )
    }

    fn is_closing_rbrace(&self, pos: usize) -> bool {
        matches!(self.context, ScalarContext::ObjectValue) && self.followed_by_terminator(pos + 1)
    }

    fn is_closing_rbracket(&self, pos: usize) -> bool {
        matches!(self.context, ScalarContext::ArrayItem) && self.followed_by_terminator(pos + 1)
    }

    fn followed_by_terminator(&self, mut pos: usize) -> bool {
        while let Some(token) = self.tokens.get(pos) {
            match token.kind {
                SyntaxKind::Whitespace => pos += 1,
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
                    return true;
                }
                SyntaxKind::Newline
                | SyntaxKind::Comma
                | SyntaxKind::RBrace
                | SyntaxKind::RBracket => return true,
                _ => return false,
            }
        }
        true
    }

    fn starts_inline_comment_after_scalar(&self, current_text: &str, kind: SyntaxKind) -> bool {
        if current_text.is_empty() {
            return true;
        }

        let trimmed = current_text.trim_end_matches([' ', '\t']);
        if trimmed.is_empty() {
            return true;
        }

        // コメントの前のテキストがリテラル（数値、true/false/null）の場合、
        // その後のコメントトークンはインラインコメント終了子として扱う。
        // それ以外（例：アンクオート文字列）の場合、コメントトークンはスカラー値のテキストの一部として扱う。
        match kind {
            SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
                HjsonNumber::is_valid(trimmed) || matches!(trimmed, "true" | "false" | "null")
            }
            _ => false,
        }
    }

    fn is_separator_comma(&self, comma_pos: usize) -> bool {
        match self.context {
            ScalarContext::Root => false,
            ScalarContext::ArrayItem => self.array_has_next_item_after_comma(comma_pos + 1),
            ScalarContext::ObjectValue => self.object_has_next_member_after_comma(comma_pos + 1),
        }
    }

    fn array_has_next_item_after_comma(&self, mut pos: usize) -> bool {
        while let Some(token) = self.tokens.get(pos) {
            match token.kind {
                SyntaxKind::Whitespace => pos += 1,
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
                    return true;
                }
                SyntaxKind::Newline | SyntaxKind::RBracket => return true,
                SyntaxKind::LBrace
                | SyntaxKind::LBracket
                | SyntaxKind::String
                | SyntaxKind::BlockString
                | SyntaxKind::UnquotedString
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null => return true,
                _ => return false,
            }
        }
        true
    }

    fn object_has_next_member_after_comma(&self, mut pos: usize) -> bool {
        while let Some(token) = self.tokens.get(pos) {
            match token.kind {
                SyntaxKind::Whitespace => pos += 1,
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
                    return true;
                }
                SyntaxKind::Newline | SyntaxKind::RBrace => return true,
                SyntaxKind::UnquotedString | SyntaxKind::String => {
                    pos += 1;
                    while let Some(next) = self.tokens.get(pos) {
                        if next.kind.is_trivia() {
                            pos += 1;
                        } else {
                            return next.kind == SyntaxKind::Colon;
                        }
                    }
                    return false;
                }
                _ => return false,
            }
        }
        true
    }
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            builder: rowan::GreenNodeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn error(&mut self, offset: usize, message: impl Into<String>) {
        self.errors.push(ParseError::new(offset, message));
    }

    /// 構文解析実行
    pub fn parse(mut self) -> ParseResult {
        self.builder.start_node(SyntaxKind::Root.into());

        let mut has_body = false;

        if !self.is_at_end() {
            if self.is_braceless_object_start() {
                self.eat_root_header_trivia_for_braceless_object();
                self.parse_braceless_object();
                has_body = true;
            } else {
                self.eat_trivia();
                if !self.is_at_end() {
                    let _ = self.parse_value(ScalarContext::Root);
                    has_body = true;
                }
            }
        }

        if !has_body {
            self.builder.start_node(SyntaxKind::Object.into());
            self.builder.finish_node();
        }

        self.eat_trivia();

        if !self.is_at_end() {
            let token = self.current().unwrap();
            self.error(token.offset, format!("Unexpected token: {:?}", token.kind));
            self.start_error_node();
            while !self.is_at_end() {
                self.bump();
            }
            self.finish_error_node();
        }

        self.builder.finish_node();

        ParseResult {
            green: self.builder.finish(),
            errors: self.errors,
        }
    }

    /// 現在のトークン
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    /// 終了判定
    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    /// 次のトークンを消費
    fn bump(&mut self) {
        if let Some(token) = self.tokens.get(self.position) {
            self.builder.token(token.kind.into(), &token.text);
            self.position += 1;
        }
    }

    fn bump_trivia(&mut self) {
        if matches!(self.current().map(|token| token.kind), Some(kind) if kind.is_comment()) {
            self.builder.start_node(SyntaxKind::Comment.into());
            self.bump();
            self.builder.finish_node();
        } else {
            self.bump();
        }
    }

    /// 特定の非トリビアトークンかチェック
    fn at(&self, kind: SyntaxKind) -> bool {
        self.current_kind() == Some(kind)
    }

    /// いずれかの非トリビアトークンかチェック
    fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        self.current_kind()
            .map(|k| kinds.contains(&k))
            .unwrap_or(false)
    }

    /// エラーノードを開始
    fn start_error_node(&mut self) {
        self.builder.start_node(SyntaxKind::Error.into());
    }

    /// エラーノードを終了
    fn finish_error_node(&mut self) {
        self.builder.finish_node();
    }

    /// 回復点までトークンを消費（トリビア含めてすべて保持）
    fn consume_until_recovery(&mut self, recovery_kinds: &[SyntaxKind]) {
        while !self.is_at_end() && !self.at_any(recovery_kinds) {
            if self.current().is_some_and(|token| token.kind.is_trivia()) {
                self.bump_trivia();
            } else {
                self.bump();
            }
        }
    }

    /// トリビア（空白・コメント・改行）を消費して追加
    ///
    /// トリビアをGreen Treeに追加しつつ、現在位置を進める
    fn eat_trivia(&mut self) {
        while let Some(token) = self.current() {
            if token.kind.is_trivia() {
                self.bump_trivia();
            } else {
                break;
            }
        }
    }

    fn eat_trivia_and_report_newline(&mut self) -> bool {
        let mut had_newline = false;
        while let Some(token) = self.current() {
            if token.kind.is_trivia() {
                if token.kind == SyntaxKind::Newline {
                    had_newline = true;
                }
                self.bump_trivia();
            } else {
                break;
            }
        }
        had_newline
    }

    fn eat_root_header_trivia_for_braceless_object(&mut self) {
        let start = self.position;
        let mut cursor = self.position;
        let mut last_newline = None;

        while let Some(token) = self.tokens.get(cursor) {
            if !token.kind.is_trivia() {
                break;
            }
            if token.kind == SyntaxKind::Newline {
                last_newline = Some(cursor);
            }
            cursor += 1;
        }

        if cursor == start {
            return;
        }

        let mut consume_until = cursor;

        if let Some(newline_index) = last_newline {
            let trailing_start = newline_index + 1;
            let trailing = &self.tokens[trailing_start..cursor];
            if !trailing.is_empty()
                && trailing
                    .iter()
                    .all(|token| token.kind == SyntaxKind::Whitespace)
            {
                consume_until = trailing_start;
            }
        } else if self.tokens[start..cursor]
            .iter()
            .all(|token| token.kind == SyntaxKind::Whitespace)
        {
            consume_until = start;
        }

        while self.position < consume_until {
            self.bump_trivia();
        }
    }

    /// 次の非トリビアトークンの種類を取得
    fn current_kind(&self) -> Option<SyntaxKind> {
        let mut pos = self.position;
        while let Some(token) = self.tokens.get(pos) {
            if token.kind.is_trivia() {
                pos += 1;
            } else {
                return Some(token.kind);
            }
        }
        None
    }

    /// 値を解析
    fn parse_value(&mut self, context: ScalarContext) -> ValueBoundary {
        self.builder.start_node(SyntaxKind::Value.into());

        // 値ノード自体には先頭トリビアを含めない（呼び出し側が先に消費する）
        // 終了チェック（ストリーム終端）
        if self.is_at_end() {
            self.builder.finish_node();
            return ValueBoundary::Open;
        }

        // 現在のトークンを確認（トリビア以外）
        if let Some(token_kind) = self.current_kind() {
            match token_kind {
                SyntaxKind::LBrace => {
                    self.parse_object();
                    self.builder.finish_node();
                    return ValueBoundary::Closed;
                }
                SyntaxKind::LBracket => {
                    self.parse_array();
                    self.builder.finish_node();
                    return ValueBoundary::Closed;
                }
                SyntaxKind::String
                | SyntaxKind::BlockString
                | SyntaxKind::UnquotedString
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null => {
                    let boundary = self.parse_scalar_value(context);
                    self.builder.finish_node();
                    return boundary;
                }
                _ => {
                    let offset = if let Some(t) = self.current() {
                        t.offset
                    } else {
                        0
                    };
                    self.start_error_node();
                    self.error(offset, format!("Unexpected token: {:?}", token_kind));
                    self.bump();
                    self.finish_error_node();
                }
            }
        }

        self.builder.finish_node();
        ValueBoundary::Closed
    }

    /// オブジェクト { ... } を解析
    fn parse_object(&mut self) {
        self.builder.start_node(SyntaxKind::Object.into());

        // {
        self.bump();

        // 空のオブジェクトかチェック
        if self.at(SyntaxKind::RBrace) {
            self.eat_trivia();
            self.bump();
            self.builder.finish_node();
            return;
        }

        self.eat_trivia();
        if self.at(SyntaxKind::RBrace) {
            self.bump();
            self.builder.finish_node();
            return;
        }

        // キー・バリューのペアを解析
        loop {
            if self.at(SyntaxKind::RBrace) {
                break;
            }

            if self.is_at_end() {
                let offset = if self.position > 0 {
                    let prev = &self.tokens[self.position - 1];
                    prev.offset + prev.text.len()
                } else {
                    0
                };
                self.error(offset, "Unclosed object, expected '}'");
                break;
            }

            // トリビアがあっても at() はトリビアをスキップして確認するため、
            // ここでは parse_member の直前に先行するトリビアを消費しておく。
            if self.at(SyntaxKind::RBrace) || self.is_at_end() {
                break;
            }

            // consume trivia so it becomes sibling tokens of the upcoming Member
            self.eat_trivia();
            self.parse_member();

            // Memberの後のトリビアを消費（コメントなど）
            let had_newline = self.eat_trivia_and_report_newline();

            // 閉じ値の後は同一行でも次の member を許可する
            if self.at(SyntaxKind::Comma) {
                self.bump();

                // カンマ後のトリビア（改行・コメントなど）を消費
                self.eat_trivia();

                // 末尾カンマ対応：}が続く場合は終了
                if self.at(SyntaxKind::RBrace) {
                    break;
                }
            } else if self.at(SyntaxKind::RBrace) {
                break;
            } else if !self.at(SyntaxKind::RBrace) && !self.is_at_end() {
                // 改行（トリビア）が消費されていればHJSONの改行区切りとして有効
                if had_newline {
                    // カンマなし・改行ありは有効な区切り → ループ継続
                    continue;
                }
                // 改行もなくカンマもない → エラー
                self.recover_from_separator_error(
                    "Expected ',' or '}'",
                    &[
                        SyntaxKind::Comma,
                        SyntaxKind::RBrace,
                        SyntaxKind::String,
                        SyntaxKind::UnquotedString,
                    ],
                );
            }
        }

        // }
        if !self.at(SyntaxKind::RBrace) && !self.is_at_end() {
            let token = self.current().unwrap();
            self.error(token.offset, "Expected '}'");
        } else {
            self.bump();
        }

        self.builder.finish_node();
    }

    /// セパレーターエラーからの回復
    fn recover_from_separator_error(&mut self, message: &str, recovery_kinds: &[SyntaxKind]) {
        let token = self.current().unwrap();
        self.error(token.offset, message);
        self.start_error_node();
        self.consume_until_recovery(recovery_kinds);
        self.finish_error_node();
    }

    /// 配列 [ ... ] を解析
    fn parse_array(&mut self) {
        self.builder.start_node(SyntaxKind::Array.into());

        // [
        self.bump();

        // 空の配列かチェック
        if self.at(SyntaxKind::RBracket) {
            self.eat_trivia();
            self.bump();
            self.builder.finish_node();
            return;
        }

        self.eat_trivia();
        if self.at(SyntaxKind::RBracket) {
            self.bump();
            self.builder.finish_node();
            return;
        }

        // 値を解析
        loop {
            if self.at(SyntaxKind::RBracket) {
                break;
            }

            if self.is_at_end() {
                let offset = if self.position > 0 {
                    let prev = &self.tokens[self.position - 1];
                    prev.offset + prev.text.len()
                } else {
                    0
                };
                self.error(offset, "Unclosed array, expected ']'");
                break;
            }

            // 値ノードの兄弟としてトリビアを消費する
            self.eat_trivia();
            let _ = self.parse_value(ScalarContext::ArrayItem);

            // 同一行に複数の値がある場合はエラー（カンマまたは改行が必要）

            // 値の後のトリビア（改行・コメントなど）を消費
            let had_newline = self.eat_trivia_and_report_newline();

            // カンマ
            if self.at(SyntaxKind::Comma) {
                self.bump();

                // カンマ後のトリビア（改行・コメントなど）を消費（複数行末尾カンマ対応）
                self.eat_trivia();

                // 末尾カンマ対応
                if self.at(SyntaxKind::RBracket) {
                    break;
                }
            } else if !self.at(SyntaxKind::RBracket) && !self.is_at_end() {
                // 改行（トリビア）が消費されていればHJSONの改行区切りとして有効
                if had_newline {
                    // カンマなし・改行ありは有効な区切り → ループ継続
                    continue;
                }
                // 改行もなくカンマもない → エラー
                self.recover_from_separator_error(
                    "Expected ',' or ']'",
                    &[SyntaxKind::Comma, SyntaxKind::RBracket],
                );
            }
        }

        if !self.at(SyntaxKind::RBracket) && !self.is_at_end() {
            let token = self.current().unwrap();
            self.error(token.offset, "Expected ']'");
        } else {
            self.bump();
        }

        self.builder.finish_node();
    }

    /// キー: 値 のペアを解析
    fn parse_member(&mut self) -> ValueBoundary {
        self.builder.start_node(SyntaxKind::Member.into());
        // Memberノード自身は先頭のtriviaを含めない。呼び出し側で先に消費される。

        // キー
        self.builder.start_node(SyntaxKind::Key.into());

        let token = self.current();
        match token.map(|t| t.kind) {
            Some(SyntaxKind::String) | Some(SyntaxKind::UnquotedString) => {
                self.bump();
            }
            _ => {
                if let Some(t) = token {
                    self.error(t.offset, format!("Expected key, found {:?}", t.kind));
                    self.start_error_node();
                    self.bump();
                    self.finish_error_node();
                } else {
                    self.error(0, "Expected key");
                }
                self.builder.finish_node();
                self.builder.finish_node();
                return ValueBoundary::Closed;
            }
        }

        self.builder.finish_node();

        // :
        if !self.at(SyntaxKind::Colon) {
            let offset = if let Some(t) = self.current() {
                t.offset
            } else {
                0
            };
            self.error(offset, "Expected ':' after key");
            self.start_error_node();
            self.consume_until_recovery(&[
                SyntaxKind::LBrace,
                SyntaxKind::LBracket,
                SyntaxKind::String,
                SyntaxKind::BlockString,
                SyntaxKind::UnquotedString,
                SyntaxKind::Integer,
                SyntaxKind::Float,
                SyntaxKind::True,
                SyntaxKind::False,
                SyntaxKind::Null,
                SyntaxKind::Comma,
                SyntaxKind::RBrace,
            ]);
            self.finish_error_node();
        } else {
            self.eat_trivia();
            self.bump();
        }

        // 値
        // ':'と値の間のトリビアを消費して、Valueノードをトリビアフリーにする
        self.eat_trivia();
        let boundary = self.parse_value(ScalarContext::ObjectValue);

        self.builder.finish_node();
        boundary
    }

    fn parse_scalar_value(&mut self, context: ScalarContext) -> ValueBoundary {
        if let Some(token) = self.current() {
            let kind = token.kind;
            return match kind {
                SyntaxKind::String
                | SyntaxKind::BlockString
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null => {
                    self.bump();
                    ValueBoundary::Closed
                }
                SyntaxKind::UnquotedString => {
                    let (text, consumed) =
                        ScalarScanner::new(&self.tokens, self.position, context).collect();
                    if consumed == 1 {
                        self.bump();
                        ValueBoundary::Open
                    } else {
                        self.builder.token(SyntaxKind::UnquotedString.into(), &text);
                        self.position += consumed;
                        ValueBoundary::Open
                    }
                }
                _ => {
                    self.bump();
                    ValueBoundary::Closed
                }
            };
        }

        ValueBoundary::Closed
    }

    fn has_colon_after_key(&self) -> bool {
        let mut pos = self.position;
        let mut consumed_key = false;

        while let Some(token) = self.tokens.get(pos) {
            if token.kind.is_trivia() {
                pos += 1;
                continue;
            }

            if !consumed_key {
                if matches!(token.kind, SyntaxKind::UnquotedString | SyntaxKind::String) {
                    consumed_key = true;
                    pos += 1;
                    continue;
                }
                return false;
            }

            return token.kind == SyntaxKind::Colon;
        }

        false
    }

    /// オブジェクトの member 開始かチェック
    /// 引用符なしキー（UnquotedString）または引用符付きキー（String）で開始される
    fn is_braceless_object_start(&self) -> bool {
        self.is_object_member_start()
    }

    fn is_object_member_start(&self) -> bool {
        matches!(
            self.current_kind(),
            Some(SyntaxKind::UnquotedString) | Some(SyntaxKind::String)
        ) && self.has_colon_after_key()
    }

    /// ブレースレスルートオブジェクトを解析（中括弧なし）
    fn parse_braceless_object(&mut self) {
        self.builder.start_node(SyntaxKind::Object.into());
        loop {
            if self.is_at_end() {
                break;
            }

            // トリビアをチェック（改行のみの場合は終了）
            self.eat_trivia();
            if self.is_at_end() {
                break;
            }

            if !self.is_braceless_object_start() {
                break;
            }

            // Memberが先行トリビアを吸収しないように、トリビアを消費する
            self.eat_trivia();
            self.parse_member();

            let had_separator_newline = self.eat_trivia_and_report_newline();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.eat_trivia();
            } else if self.at(SyntaxKind::RBrace) {
                break;
            } else if !self.is_at_end() && !had_separator_newline {
                break;
            }
        }

        self.builder.finish_node();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::lexer::Lexer;
    use crate::ast::Document;
    use crate::ast::AstNode;

    fn parse_text(src: &str) -> ParseResult {
        let tokens = Lexer::new(src).lex();
        Parser::new(tokens.tokens).parse()
    }

    #[test]
    fn test_parser_recover_from_missing_colon_reports_error_but_produces_ast() {
        let parsed = parse_text("{\n  \"key\" \"value\"\n}");
        assert!(!parsed.errors.is_empty());
        
        let doc = Document::cast(crate::syntax::RedNode::new_root(parsed.green)).unwrap();
        let root = doc.object().unwrap();
        assert_eq!(root.members().len(), 1);
    }

    #[test]
    fn test_parser_recover_unclosed_object_reports_error_but_produces_ast() {
        let parsed = parse_text("{ \"a\": 1");
        assert!(!parsed.errors.is_empty());
        
        let doc = Document::cast(crate::syntax::RedNode::new_root(parsed.green)).unwrap();
        let root = doc.object().unwrap();
        assert_eq!(root.members().len(), 1);
    }

    #[test]
    fn test_parser_recover_unclosed_array_reports_error_but_produces_ast() {
        let parsed = parse_text("[ 1, 2");
        assert!(!parsed.errors.is_empty());
        
        let doc = Document::cast(crate::syntax::RedNode::new_root(parsed.green)).unwrap();
        let root = doc.array().unwrap();
        assert_eq!(root.items().len(), 2);
    }
}
