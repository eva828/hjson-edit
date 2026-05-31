use line_index::LineIndex;
use rowan::TextSize;

use super::SyntaxKind;
/// HJSON字句解析器
///
/// エラー継続型：エラーがあっても解析を続行し、詳細なエラー情報を収集
use crate::number::{HjsonNumber, NumberKind};

/// 字句解析エラー（発生したバイトオフセットのみを記録）
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub offset: usize,
}

impl LexError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
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

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LexError at offset {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for LexError {}

pub struct Cursor<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn remaining_str(&self) -> &'a str {
        &self.input[self.position..]
    }

    pub fn peek(&self) -> Option<char> {
        self.remaining_str().chars().next()
    }

    pub fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    pub fn advance(&mut self) -> Option<char> {
        if let Some(c) = self.peek() {
            self.position += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    pub fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn match_str(&mut self, expected: &str) -> bool {
        if self.remaining_str().starts_with(expected) {
            for _ in 0..expected.chars().count() {
                self.advance();
            }
            true
        } else {
            false
        }
    }

    /// 開始位置から現在の位置までをスライスとして切り出す（ゼロアロケーション）
    pub fn slice(&self, start: usize, end: usize) -> &'a str {
        &self.input[start..end]
    }

    pub fn peek_n(&self, n: usize) -> Option<char> {
        self.remaining_str().chars().nth(n)
    }

    pub fn peek_prev(&self) -> Option<char> {
        if self.position == 0 {
            None
        } else {
            let prev_str = &self.input[..self.position];
            prev_str.chars().last()
        }
    }

    /// 前の文字を取得（peek_prevのエイリアス）
    pub fn prev(&self) -> Option<char> {
        self.peek_prev()
    }

    pub fn pos(&self) -> usize {
        self.position
    }
}

/// トークン（メタデータを削ぎ落とした純粋なデータ）
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: String,
    pub offset: usize,
}

/// 字句解析結果
#[derive(Debug, Clone)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

/// 字句解析器本体
pub struct Lexer<'a> {
    cursor: Cursor<'a>,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            cursor: Cursor::new(input),
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// 字句解析実行
    pub fn lex(mut self) -> LexResult {
        while !self.cursor.is_at_end() {
            if self.cursor.pos() == 0 && self.cursor.peek() == Some('\u{FEFF}') {
                self.cursor.advance();
                continue;
            }

            self.skip_whitespace_no_newline();

            if self.cursor.is_at_end() {
                break;
            }

            let start_offset = self.cursor.pos();

            if self.cursor.match_char('\n') {
                self.emit(SyntaxKind::Newline, "\n", start_offset);
                continue;
            }

            if self.cursor.match_char('\r') {
                if self.cursor.match_char('\n') {
                    self.emit(SyntaxKind::Newline, "\r\n", start_offset);
                } else {
                    self.emit(SyntaxKind::Whitespace, "\r", start_offset);
                }
                continue;
            }

            if self.cursor.match_str("//") {
                self.read_line_comment(start_offset);
                continue;
            }

            if self.cursor.peek() == Some('#') && self.is_hash_comment_start() {
                self.cursor.advance();
                self.read_hash_comment(start_offset);
                continue;
            }

            if self.cursor.match_str("/*") {
                self.read_block_comment(start_offset);
                continue;
            }

            if self.cursor.match_str("'''") {
                self.read_block_string(start_offset);
                continue;
            }

            if self.cursor.match_char('"') {
                self.read_double_quoted_string(start_offset);
                continue;
            }

            if let Some(kind) = self.match_punctuation() {
                let end_offset = self.cursor.pos();
                let text = self.cursor.slice(start_offset, end_offset);
                self.emit(kind, text, start_offset);
                continue;
            }

            self.read_quoteless_token(start_offset);
        }

        LexResult {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    fn skip_whitespace_no_newline(&mut self) {
        let start_offset = self.cursor.pos();

        while let Some(c) = self.cursor.peek() {
            if c == ' ' || c == '\t' {
                self.cursor.advance();
            } else {
                break;
            }
        }

        let end_offset = self.cursor.pos();
        if end_offset > start_offset {
            let text = self.cursor.slice(start_offset, end_offset);
            self.emit(SyntaxKind::Whitespace, text, start_offset);
        }
    }

    fn is_hash_comment_start(&self) -> bool {
        if self.cursor.pos() == 0 {
            return true;
        }
        match self.cursor.prev() {
            Some(ch) => matches!(ch, '\n' | '\r' | ' ' | '\t'),
            None => true,
        }
    }

    fn emit(&mut self, kind: SyntaxKind, text: &str, offset: usize) {
        self.tokens.push(Token {
            kind,
            text: text.to_string(),
            offset,
        });
    }

    fn match_punctuation(&mut self) -> Option<SyntaxKind> {
        let c = self.cursor.peek()?;
        let kind = match c {
            '{' => SyntaxKind::LBrace,
            '}' => SyntaxKind::RBrace,
            '[' => SyntaxKind::LBracket,
            ']' => SyntaxKind::RBracket,
            ':' => SyntaxKind::Colon,
            ',' => SyntaxKind::Comma,
            _ => return None,
        };
        self.cursor.advance();
        Some(kind)
    }

    fn read_line_comment(&mut self, start_offset: usize) {
        while let Some(c) = self.cursor.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.cursor.advance();
        }
        let text = self.cursor.slice(start_offset, self.cursor.pos());
        self.emit(SyntaxKind::LineComment, text, start_offset);
    }

    fn read_hash_comment(&mut self, start_offset: usize) {
        while let Some(c) = self.cursor.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.cursor.advance();
        }
        let text = self.cursor.slice(start_offset, self.cursor.pos());
        self.emit(SyntaxKind::HashComment, text, start_offset);
    }

    fn read_block_comment(&mut self, start_offset: usize) {
        let mut terminated = false;
        while !self.cursor.is_at_end() {
            if self.cursor.match_str("*/") {
                terminated = true;
                break;
            }
            self.cursor.advance();
        }

        if !terminated {
            self.errors
                .push(LexError::new(start_offset, "Unterminated block comment"));
        }

        let text = self.cursor.slice(start_offset, self.cursor.pos());
        self.emit(SyntaxKind::BlockComment, text, start_offset);
    }

    fn read_block_string(&mut self, start_offset: usize) {
        let mut terminated = false;
        while !self.cursor.is_at_end() {
            if self.cursor.match_str("'''") {
                terminated = true;
                break;
            }
            // \r\n の正規化は行わず、そのまま進める（Lossless性を保持）
            self.cursor.advance();
        }

        if !terminated {
            self.errors
                .push(LexError::new(start_offset, "Unterminated block string"));
        }

        let text = self.cursor.slice(start_offset, self.cursor.pos());
        self.emit(SyntaxKind::BlockString, text, start_offset);
    }

    fn read_double_quoted_string(&mut self, start_offset: usize) {
    let mut terminated = false;
    // 開きの " は lex() の match_char('"') で既に消費済み

        while let Some(c) = self.cursor.peek() {
            if c == '"' {
                self.cursor.advance();
                terminated = true;
                break;
            }

            if c == '\\' {
                self.read_escape_sequence(start_offset);
            } else if c == '\n' || c == '\r' {
                self.errors
                    .push(LexError::new(start_offset, "Unterminated string literal"));
                break;
            } else {
                self.cursor.advance();
            }
        }

        if !terminated
            && self.cursor.is_at_end()
            && !self
                .errors
                .iter()
                .any(|e| e.message.contains("Unterminated"))
        {
            self.errors
                .push(LexError::new(start_offset, "Unterminated string literal"));
        }

        let text = self.cursor.slice(start_offset, self.cursor.pos());
        self.emit(SyntaxKind::String, text, start_offset);
    }

    fn read_escape_sequence(&mut self, start_offset: usize) {
        // '\'を消費
        self.cursor.advance();
        match self.cursor.advance() {
            Some('u') => {
                self.validate_unicode_escape(start_offset);
            }
            Some(escaped) if matches!(escaped, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') => {}
            Some(escaped) => {
                self.errors.push(LexError::new(
                    start_offset,
                    format!("Invalid escape sequence: \\{}", escaped),
                ));
            }
            None => {
                self.errors
                    .push(LexError::new(start_offset, "Unterminated string literal"));
            }
        }
    }

    fn validate_unicode_escape(&mut self, start_offset: usize) {
        for _ in 0..4 {
            match self.cursor.advance() {
                Some(hex) if hex.is_ascii_hexdigit() => {}
                Some(other) => {
                    self.errors.push(LexError::new(
                        start_offset,
                        format!("Invalid unicode escape: \\u...{}", other),
                    ));
                    break;
                }
                None => {
                    self.errors
                        .push(LexError::new(start_offset, "Unterminated unicode escape"));
                    break;
                }
            }
        }
    }

    fn read_quoteless_token(&mut self, start_offset: usize) {
        // 先頭が無効な文字でないかチェック
        let first = match self.cursor.peek() {
            Some(c) => c,
            None => return,
        };

        if is_punctuator(first) {
            let c = self.cursor.advance().unwrap();
            self.errors.push(LexError::new(
                start_offset,
                format!("Unexpected character: {}", c),
            ));
            return;
        }

        // HJSONの仕様に基づいて、トークンが終了する条件まで進める
        while let Some(c) = self.cursor.peek() {
            if self.is_quoteless_token_end(c) {
                break;
            }
            self.cursor.advance();
        }

        let end_offset = self.cursor.pos();
        if start_offset == end_offset {
            let c = self.cursor.advance().unwrap();
            self.errors.push(LexError::new(
                start_offset,
                format!("Unexpected character: {}", c),
            ));
            return;
        }

        let text = self.cursor.slice(start_offset, end_offset);
        let kind = self.classify_word(text);

        self.emit(kind, text, start_offset);
    }

    fn is_quoteless_token_end(&self, c: char) -> bool {
        // コメント開始でストップ
        if c == '/' {
            if let Some(next_char) = self.cursor.peek_n(1) {
                if next_char == '/' || next_char == '*' {
                    return true;
                }
            }
        }
        if c == '#' && self.is_hash_comment_start() {
            return true;
        }
        // 空白やカンマ・括弧などの記号でストップ
        if c.is_whitespace() || is_punctuator(c) {
            return true;
        }
        false
    }

    fn classify_word(&self, text: &str) -> SyntaxKind {
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
}

fn is_punctuator(c: char) -> bool {
    matches!(c, '{' | '}' | '[' | ']' | ',' | ':')
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_basics() {
        let mut cursor = Cursor::new("aあb"); // 'a' (1 byte), 'あ' (3 bytes), 'b' (1 byte)
        
        assert_eq!(cursor.peek(), Some('a'));
        assert_eq!(cursor.peek_n(1), Some('あ'));
        assert_eq!(cursor.peek_prev(), None);

        cursor.advance();
        assert_eq!(cursor.peek(), Some('あ'));
        assert_eq!(cursor.peek_prev(), Some('a'));
        
        cursor.advance();
        assert_eq!(cursor.peek(), Some('b'));
        assert_eq!(cursor.peek_prev(), Some('あ'));
        
        cursor.advance();
        assert_eq!(cursor.peek(), None);
        assert!(cursor.is_at_end());
        
        assert_eq!(cursor.advance(), None);
    }

    #[test]
    fn test_cursor_match() {
        let mut cursor = Cursor::new("foo bar");
        
        assert!(cursor.match_str("foo"));
        assert_eq!(cursor.pos(), 3);
        
        assert!(!cursor.match_str("baz"));
        assert_eq!(cursor.pos(), 3);
        
        assert!(cursor.match_char(' '));
        assert_eq!(cursor.pos(), 4);
    }

    #[test]
    fn test_lexer_hash_comment_start() {
        let lexer = Lexer::new("# foo\n");
        assert!(lexer.is_hash_comment_start());

        let mut lexer = Lexer::new("  # foo\n");
        lexer.cursor.advance();
        lexer.cursor.advance();
        assert!(lexer.is_hash_comment_start());

        let mut lexer = Lexer::new("a# foo\n");
        lexer.cursor.advance();
        assert!(!lexer.is_hash_comment_start());
    }

    #[test]
    fn test_lexer_invalid_escape_sequence_produces_error() {
        let result = Lexer::new(r#""\x""#).lex();
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("Invalid escape sequence: \\x"));
    }

    #[test]
    fn test_lexer_valid_escape_sequence_produces_no_error() {
        let result = Lexer::new(r#""\"""#).lex();
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_lexer_eof_after_backslash_produces_error() {
        let result = Lexer::new(r#""\"#).lex();
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("Unterminated string literal"));
    }
}
