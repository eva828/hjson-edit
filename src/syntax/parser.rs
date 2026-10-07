/// HJSON構文解析器
///
/// Lexerのトークン列をGreen Treeに変換
use super::lexer::{Cursor, Token};
use super::SyntaxKind;
use crate::number::HjsonNumber;
use line_index::{LineIndex, TextSize};

/// 構文解析エラー（エラーメッセージと発生したバイトオフセットを保持）
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
pub struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    position: usize,
    builder: rowan::GreenNodeBuilder<'static>,
    errors: Vec<ParseError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueBoundary {
    Closed,
    Open,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            position: 0,
            builder: rowan::GreenNodeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn error(&mut self, offset: usize, message: impl Into<String>) {
        self.errors.push(ParseError::new(offset, message));
    }

    fn is_hjson_delimiter(&self, cursor: &Cursor) -> bool {
        let c = cursor.peek().unwrap();
        c == ','
            || c == '}'
            || c == ']'
            || c == '#'
            || (c == '/' && cursor.peek_n(1).is_some_and(|n| n == '/' || n == '*'))
    }

    fn is_keyword_or_number(text: &str) -> bool {
        matches!(text.trim(), "true" | "false" | "null") || HjsonNumber::is_valid(text.trim())
    }

    /// 生のソーステキストから quoteless value を文字単位で読み取る。
    fn read_raw_value(&self, start_offset: usize) -> (String, usize) {
        let sub = &self.source[start_offset..];
        let mut cursor = Cursor::new(sub);
        let mut last_good_pos = 0usize;

        loop {
            if cursor.is_at_end() {
                break;
            }
            let c = cursor.peek().unwrap();

            if c == '\n' {
                break;
            }
            if c == '\r' {
                cursor.advance();
                continue;
            }

            if self.is_hjson_delimiter(&cursor) {
                if Self::is_keyword_or_number(&sub[..cursor.pos()]) {
                    break;
                }
                cursor.advance();
                last_good_pos = cursor.pos();
                continue;
            }

            cursor.advance();
            last_good_pos = cursor.pos();
        }

        let end = if last_good_pos > 0 {
            last_good_pos
        } else {
            cursor.pos()
        };
        let raw = &sub[..end];
        let trimmed = raw.trim_end();
        (trimmed.to_string(), trimmed.len())
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
                    let _ = self.parse_value();
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

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

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

    fn at(&self, kind: SyntaxKind) -> bool {
        self.current_kind() == Some(kind)
    }

    fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        self.current_kind()
            .map(|k| kinds.contains(&k))
            .unwrap_or(false)
    }

    fn start_error_node(&mut self) {
        self.builder.start_node(SyntaxKind::Error.into());
    }

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

    fn can_start_value(&self) -> bool {
        matches!(
            self.current_kind(),
            Some(
                SyntaxKind::String
                    | SyntaxKind::BlockString
                    | SyntaxKind::LBrace
                    | SyntaxKind::LBracket
                    | SyntaxKind::UnquotedString
                    | SyntaxKind::Integer
                    | SyntaxKind::Float
                    | SyntaxKind::True
                    | SyntaxKind::False
                    | SyntaxKind::Null,
            )
        )
    }

    fn parse_value(&mut self) -> ValueBoundary {
        self.builder.start_node(SyntaxKind::Value.into());

        // 値ノード自体には先頭トリビアを含めない（呼び出し側が先に消費する）
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
                    let boundary = self.parse_scalar_value();
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

    fn parse_object(&mut self) {
        self.builder.start_node(SyntaxKind::Object.into());

        // {
        self.bump();

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

            let had_newline = self.eat_trivia_and_report_newline();

            // 閉じ値の後は同一行でも次の member を許可する
            if self.at(SyntaxKind::Comma) {
                self.bump();

                self.eat_trivia();

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

    fn parse_array(&mut self) {
        self.builder.start_node(SyntaxKind::Array.into());

        // [
        self.bump();

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

            let is_brace_delimited = matches!(
                self.current_kind(),
                Some(
                    SyntaxKind::String
                        | SyntaxKind::BlockString
                        | SyntaxKind::LBrace
                        | SyntaxKind::LBracket
                )
            );

            let _ = self.parse_value();

            let had_newline = self.eat_trivia_and_report_newline();

            // カンマ
            if self.at(SyntaxKind::Comma) {
                self.bump();

                self.eat_trivia();

                if self.at(SyntaxKind::RBracket) {
                    break;
                }
            } else if !self.at(SyntaxKind::RBracket) && !self.is_at_end() {
                // 改行（トリビア）が消費されていればHJSONの改行区切りとして有効
                if had_newline {
                    // カンマなし・改行ありは有効な区切り → ループ継続
                    continue;
                }
                let whitespace_separator_allowed = is_brace_delimited && self.can_start_value();
                if whitespace_separator_allowed {
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

    fn parse_member(&mut self) -> ValueBoundary {
        self.builder.start_node(SyntaxKind::Member.into());
        // Memberノード自身は先頭のtriviaを含めない。呼び出し側で先に消費される。

        self.builder.start_node(SyntaxKind::Key.into());

        let token = self.current();
        match token.map(|t| t.kind) {
            Some(
                SyntaxKind::String
                | SyntaxKind::UnquotedString
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null,
            ) => {
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
        let boundary = self.parse_value();

        self.builder.finish_node();
        boundary
    }

    fn parse_scalar_value(&mut self) -> ValueBoundary {
        if let Some(token) = self.current() {
            let kind = token.kind;
            return match kind {
                SyntaxKind::String | SyntaxKind::BlockString => {
                    self.bump();
                    ValueBoundary::Closed
                }
                SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null
                | SyntaxKind::UnquotedString => {
                    let start_offset = token.offset;
                    let (_, byte_len) = self.read_raw_value(start_offset);
                    let raw_end = start_offset + byte_len;

                    // span の外にはみ出すトークンは取り込まない。取り込むと
                    // そのトークンが覆う span 外のテキストが欠落する。
                    let mut consumed = 0;
                    while self.position + consumed < self.tokens.len() {
                        let candidate = &self.tokens[self.position + consumed];
                        if candidate.offset + candidate.text.len() > raw_end {
                            break;
                        }
                        consumed += 1;
                    }

                    // 末尾の空白・改行は値に含めず、後続のトリビアとして残す。
                    // コメントは quoteless 値の一部として扱う（参照実装と同じ）。
                    while consumed > 0 {
                        let last = &self.tokens[self.position + consumed - 1];
                        if matches!(last.kind, SyntaxKind::Whitespace | SyntaxKind::Newline) {
                            consumed -= 1;
                        } else {
                            break;
                        }
                    }

                    if consumed <= 1 {
                        self.bump();
                        ValueBoundary::Closed
                    } else {
                        // 値の文字列は実際に消費したトークンの範囲から取り出す。
                        let last = &self.tokens[self.position + consumed - 1];
                        let covered_end = last.offset + last.text.len();
                        let text = &self.source[start_offset..covered_end];

                        self.builder.token(SyntaxKind::UnquotedString.into(), text);
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
                if matches!(
                    token.kind,
                    SyntaxKind::UnquotedString
                        | SyntaxKind::String
                        | SyntaxKind::Integer
                        | SyntaxKind::Float
                        | SyntaxKind::True
                        | SyntaxKind::False
                        | SyntaxKind::Null,
                ) {
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
            Some(SyntaxKind::UnquotedString)
                | Some(SyntaxKind::String)
                | Some(SyntaxKind::Integer)
                | Some(SyntaxKind::Float)
                | Some(SyntaxKind::True)
                | Some(SyntaxKind::False)
                | Some(SyntaxKind::Null),
        ) && self.has_colon_after_key()
    }

    /// ブレースレスルートオブジェクトを解析（中括弧なし）
    fn parse_braceless_object(&mut self) {
        self.builder.start_node(SyntaxKind::Object.into());
        loop {
            if self.is_at_end() {
                break;
            }

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
            } else if self.at(SyntaxKind::RBrace) || (!self.is_at_end() && !had_separator_newline) {
                break;
            }
        }

        self.builder.finish_node();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::AstNode;
    use crate::ast::Document;
    use crate::syntax::lexer::Lexer;

    fn parse_text(src: &str) -> ParseResult {
        let tokens = Lexer::new(src).lex();
        Parser::new(src, tokens.tokens).parse()
    }

    #[test]
    fn test_multiline_block_string_in_quoteless_value_keeps_all_text() {
        // 値の span を越えて伸びるブロック文字列トークンを取り込むと、
        // そのトークンが覆うテキストが欠落する。
        let src = "a: x{0} '''y\nz''' \nb: 2\n";
        let parsed = parse_text(src);
        assert!(!parsed.errors.is_empty());
        let text_len: usize = crate::syntax::RedNode::new_root(parsed.green)
            .text()
            .len()
            .into();
        assert_eq!(text_len, src.len());
    }

    #[test]
    fn test_multiline_block_string_in_quoteless_value_is_not_accepted_silently() {
        let parsed = parse_text("a: x{0} '''y\nz''' \nb: 2\n");
        assert!(parsed
            .errors
            .iter()
            .any(|e| e.message.contains("Unexpected token")));
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
