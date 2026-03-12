/// HJSON字句解析器
///
/// エラー継続型：エラーがあっても解析を続行し、詳細なエラー情報を収集
use crate::syntax::kind::SyntaxKind;

/// 字句解析エラー
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl LexError {
    fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

/// トークン
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: String,
    pub line: usize,
    pub column: usize,
}

/// 字句解析結果
#[derive(Debug, Clone)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

/// 字句解析器
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// 字句解析実行
    pub fn lex(mut self) -> LexResult {
        while !self.is_at_end() {
            self.skip_whitespace_no_newline();

            if self.is_at_end() {
                break;
            }

            let start_line = self.line;
            let start_col = self.column;

            if self.match_char('\n') {
                self.emit(SyntaxKind::Newline, "\n", start_line, start_col);
                self.line += 1;
                self.column = 1;
                continue;
            }

            if self.match_char('\r') {
                // \r\n または \r を \n に正規化（RFC準拠）
                if self.peek() == Some('\n') {
                    self.advance(); // \n も消費
                }
                self.emit(SyntaxKind::Newline, "\n", start_line, start_col);
                self.line += 1;
                self.column = 1;
                continue;
            }

            if self.match_str("//") {
                self.read_line_comment(start_line, start_col);
                continue;
            }

            if self.peek() == Some('#') && self.is_hash_comment_start() {
                self.advance();
                self.read_hash_comment(start_line, start_col);
                continue;
            }

            if self.match_str("/*") {
                self.read_block_comment(start_line, start_col);
                continue;
            }

            if self.match_str("'''") {
                self.read_block_string(start_line, start_col);
                continue;
            }

            if self.match_char('"') {
                self.read_double_quoted_string(start_line, start_col);
                continue;
            }

            if let Some(kind) = self.match_punctuation() {
                let text: String = self.input[self.position - 1..self.position]
                    .iter()
                    .collect();
                self.emit(kind, &text, start_line, start_col);
                continue;
            }

            self.read_quoteless_token(start_line, start_col);
        }

        LexResult {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    fn advance(&mut self) -> Option<char> {
        if let Some(&c) = self.input.get(self.position) {
            self.position += 1;
            self.column += 1;
            Some(c)
        } else {
            None
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_str(&mut self, expected: &str) -> bool {
        let chars: Vec<char> = expected.chars().collect();
        if self.position + chars.len() > self.input.len() {
            return false;
        }

        for (i, &c) in chars.iter().enumerate() {
            if self.input[self.position + i] != c {
                return false;
            }
        }

        for _ in 0..chars.len() {
            self.advance();
        }
        true
    }

    fn skip_whitespace_no_newline(&mut self) {
        let start = self.position;
        let start_col = self.column;
        let start_line = self.line;

        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.advance();
            } else {
                break;
            }
        }

        if self.position > start {
            let text: String = self.input[start..self.position].iter().collect();
            self.tokens.push(Token {
                kind: SyntaxKind::Whitespace,
                text,
                line: start_line,
                column: start_col,
            });
        }
    }

    fn is_hash_comment_start(&self) -> bool {
        if self.position == 0 {
            return true;
        }
        matches!(self.input[self.position - 1], '\n' | '\r' | ' ' | '\t')
    }

    fn emit(&mut self, kind: SyntaxKind, text: impl Into<String>, line: usize, column: usize) {
        self.tokens.push(Token {
            kind,
            text: text.into(),
            line,
            column,
        });
    }

    fn match_punctuation(&mut self) -> Option<SyntaxKind> {
        let c = self.peek()?;
        let kind = match c {
            '{' => SyntaxKind::LBrace,
            '}' => SyntaxKind::RBrace,
            '[' => SyntaxKind::LBracket,
            ']' => SyntaxKind::RBracket,
            ':' => SyntaxKind::Colon,
            ',' => SyntaxKind::Comma,
            _ => return None,
        };
        self.advance();
        Some(kind)
    }

    fn read_line_comment(&mut self, start_line: usize, start_col: usize) {
        let mut text = String::from("//");

        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            text.push(c);
            self.advance();
        }

        self.emit(SyntaxKind::LineComment, text, start_line, start_col);
    }

    fn read_hash_comment(&mut self, start_line: usize, start_col: usize) {
        let mut text = String::from("#");

        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            text.push(c);
            self.advance();
        }

        self.emit(SyntaxKind::HashComment, text, start_line, start_col);
    }

    fn read_block_comment(&mut self, start_line: usize, start_col: usize) {
        let mut text = String::from("/*");

        while !self.is_at_end() {
            if self.match_str("*/") {
                text.push_str("*/");
                self.emit(SyntaxKind::BlockComment, text, start_line, start_col);
                return;
            }

            let c = self.advance().unwrap();
            text.push(c);

            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else if c == '\r' {
                // CRを検出したら行番号をインクリメント（RFC準拠）
                self.line += 1;
                self.column = 1;
                // 次が\nなら消費（CRLFの場合）
                if self.peek() == Some('\n') {
                    self.advance();
                }
            }
        }

        self.errors.push(LexError::new(
            "Unterminated block comment",
            start_line,
            start_col,
        ));
        self.emit(SyntaxKind::BlockComment, text, start_line, start_col);
    }

    fn read_block_string(&mut self, start_line: usize, start_col: usize) {
        let mut text = String::from("'''");

        while !self.is_at_end() {
            if self.match_str("'''") {
                text.push_str("'''");
                self.emit(SyntaxKind::BlockString, text, start_line, start_col);
                return;
            }

            let c = self.advance().unwrap();
            text.push(c);

            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else if c == '\r' {
                // CRを検出したら行番号をインクリメント（RFC準拠）
                self.line += 1;
                self.column = 1;
                // 次が\nなら消費（CRLFの場合）
                if self.peek() == Some('\n') {
                    self.advance();
                }
            }
        }

        self.errors.push(LexError::new(
            "Unterminated block string",
            start_line,
            start_col,
        ));
        self.emit(SyntaxKind::BlockString, text, start_line, start_col);
    }

    fn read_double_quoted_string(&mut self, start_line: usize, start_col: usize) {
        let mut text = String::from("\"");

        while !self.is_at_end() {
            let c = self.peek().unwrap();

            if c == '"' {
                self.advance();
                text.push('"');
                self.emit(SyntaxKind::String, text, start_line, start_col);
                return;
            }

            if c == '\\' {
                self.advance();
                text.push('\\');
                match self.advance() {
                    Some(escaped) => {
                        text.push(escaped);
                        if escaped == 'u' {
                            for _ in 0..4 {
                                match self.advance() {
                                    Some(hex) if hex.is_ascii_hexdigit() => text.push(hex),
                                    Some(other) => {
                                        text.push(other);
                                        self.errors.push(LexError::new(
                                            format!("Invalid unicode escape: \\u...{}", other),
                                            start_line,
                                            start_col,
                                        ));
                                        break;
                                    }
                                    None => {
                                        self.errors.push(LexError::new(
                                            "Unterminated unicode escape",
                                            start_line,
                                            start_col,
                                        ));
                                        self.emit(SyntaxKind::String, text, start_line, start_col);
                                        return;
                                    }
                                }
                            }
                        } else if !matches!(escaped, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't')
                        {
                            self.errors.push(LexError::new(
                                format!("Invalid escape sequence: \\{}", escaped),
                                start_line,
                                start_col,
                            ));
                        }
                    }
                    None => {
                        self.errors.push(LexError::new(
                            "Unterminated string literal",
                            start_line,
                            start_col,
                        ));
                        self.emit(SyntaxKind::String, text, start_line, start_col);
                        return;
                    }
                }
            } else if c == '\n' || c == '\r' {
                self.errors.push(LexError::new(
                    "Unterminated string literal",
                    start_line,
                    start_col,
                ));
                self.emit(SyntaxKind::String, text, start_line, start_col);
                return;
            } else {
                self.advance();
                text.push(c);
            }
        }

        self.errors.push(LexError::new(
            "Unterminated string literal",
            start_line,
            start_col,
        ));
        self.emit(SyntaxKind::String, text, start_line, start_col);
    }

    fn read_quoteless_token(&mut self, start_line: usize, start_col: usize) {
        let first = match self.peek() {
            Some(c) => c,
            None => return,
        };

        if is_punctuator(first) {
            let c = self.advance().unwrap();
            self.errors.push(LexError::new(
                format!("Unexpected character: {}", c),
                start_line,
                start_col,
            ));
            return;
        }

        let mut text = String::new();
        while let Some(c) = self.peek() {
            if c.is_whitespace()
                || c == '{'
                || c == '}'
                || c == '['
                || c == ']'
                || c == ':'
                || c == ','
            {
                break;
            }
            text.push(c);
            self.advance();
        }

        if text.is_empty() {
            let c = self.advance().unwrap();
            self.errors.push(LexError::new(
                format!("Unexpected character: {}", c),
                start_line,
                start_col,
            ));
            return;
        }

        let kind = self.classify_word(&text);
        self.emit(kind, text, start_line, start_col);
    }

    fn classify_word(&self, text: &str) -> SyntaxKind {
        match text {
            "true" => SyntaxKind::True,
            "false" => SyntaxKind::False,
            "null" => SyntaxKind::Null,
            _ if is_valid_number(text) => {
                if text.contains('.') || text.contains('e') || text.contains('E') {
                    SyntaxKind::Float
                } else {
                    SyntaxKind::Integer
                }
            }
            _ if is_ident(text) => SyntaxKind::Ident,
            _ => SyntaxKind::UnquotedString,
        }
    }
}

fn is_punctuator(c: char) -> bool {
    matches!(c, '{' | '}' | '[' | ']' | ',' | ':')
}

fn is_ident(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_non_punctuator_char)
}

fn is_non_punctuator_char(c: char) -> bool {
    !matches!(c, '{' | '}' | '[' | ']' | ',' | ':') && !c.is_whitespace()
}

pub(crate) fn is_valid_number(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let bytes = text.as_bytes();
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

    if idx < bytes.len() && (bytes[idx] == b'e' || bytes[idx] == b'E') {
        idx += 1;
        if idx < bytes.len() && (bytes[idx] == b'+' || bytes[idx] == b'-') {
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

/// 公開API
pub fn lex(input: &str) -> LexResult {
    Lexer::new(input).lex()
}


