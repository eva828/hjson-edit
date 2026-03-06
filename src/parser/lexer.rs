/// HJSON字句解析器
/// 
/// エラー継続型：エラーがあっても解析を続行し、詳細なエラー情報を収集
use super::super::syntax::kind::SyntaxKind;

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
            
            // 改行の処理
            if self.match_char('\n') {
                self.emit(SyntaxKind::Newline, "\n", start_line, start_col);
                self.line += 1;
                self.column = 1;
                continue;
            }
            
            if self.match_char('\r') {
                let text = if self.peek() == Some('\n') {
                    self.advance();
                    "\r\n"
                } else {
                    "\r"
                };
                self.emit(SyntaxKind::Newline, text, start_line, start_col);
                self.line += 1;
                self.column = 1;
                continue;
            }
            
            // コメントの処理
            if self.match_str("//") {
                self.read_line_comment(start_line, start_col);
                continue;
            }
            
            if self.match_char('#') {
                self.read_hash_comment(start_line, start_col);
                continue;
            }
            
            if self.match_str("/*") {
                self.read_block_comment(start_line, start_col);
                continue;
            }
            
            // ブロック文字列 '''...'''
            if self.match_str("'''") {
                self.read_block_string(start_line, start_col);
                continue;
            }
            
            // 文字列リテラル
            if self.match_char('"') {
                self.read_double_quoted_string(start_line, start_col);
                continue;
            }
            
            if self.match_char('\'') {
                self.read_single_quoted_string(start_line, start_col);
                continue;
            }
            
            // 構文記号
            if let Some(kind) = self.match_punctuation() {
                let text: String = self.input[self.position - 1..self.position]
                    .iter()
                    .collect();
                self.emit(kind, &text, start_line, start_col);
                continue;
            }
            
            // 数値・特殊値・識別子・引用符なし文字列
            self.read_word_or_number(start_line, start_col);
        }
        
        LexResult {
            tokens: self.tokens,
            errors: self.errors,
        }
    }
    
    /// 1文字先読み（消費しない）
    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }
    
    /// 2文字先読み
    fn peek_next(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }
    
    /// 終了判定
    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
    
    /// 1文字消費
    fn advance(&mut self) -> Option<char> {
        if let Some(&c) = self.input.get(self.position) {
            self.position += 1;
            self.column += 1;
            Some(c)
        } else {
            None
        }
    }
    
    /// 特定文字にマッチしたら消費
    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }
    
    /// 特定文字列にマッチしたら消費
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
    
    /// 空白（改行以外）を消費してWhitespaceトークンとして記録
    fn skip_whitespace_no_newline(&mut self) {
        let start = self.position;
        let start_col = self.column;
        let start_line = self.line;
        
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }
        
        // 空白があればWhitespaceトークンとして記録
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
    
    /// トークン生成
    fn emit(&mut self,
        kind: SyntaxKind,
        text: impl Into<String>,
        line: usize,
        column: usize,
    ) {
        self.tokens.push(Token {
            kind,
            text: text.into(),
            line,
            column,
        });
    }
    
    /// 構文記号を判定
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
    
    /// 行コメント //... を読み込み
    fn read_line_comment(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
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
    
    /// #コメントを読み込み
    fn read_hash_comment(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
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
    
    /// ブロックコメント /* ... */ を読み込み
    fn read_block_comment(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
        let mut text = String::from("/*");
        let _start_pos = self.position;
        
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
            }
        }
        
        // 終了しないブロックコメント
        self.errors.push(LexError::new(
            "Unterminated block comment",
            start_line,
            start_col,
        ));
        self.emit(SyntaxKind::BlockComment, text, start_line, start_col);
    }
    
    /// ブロック文字列 '''...''' を読み込み
    fn read_block_string(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
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
            }
        }
        
        // 終了しないブロック文字列
        self.errors.push(LexError::new(
            "Unterminated block string",
            start_line,
            start_col,
        ));
        self.emit(SyntaxKind::BlockString, text, start_line, start_col);
    }
    
    /// ダブルクォート文字列 "..." を読み込み
    fn read_double_quoted_string(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
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
                // エスケープシーケンス
                self.advance();
                text.push('\\');
                if let Some(escaped) = self.advance() {
                    text.push(escaped);
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
    
    /// シングルクォート文字列 '...' を読み込み
    fn read_single_quoted_string(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
        let mut text = String::from("'");
        
        while !self.is_at_end() {
            let c = self.peek().unwrap();
            
            if c == '\'' {
                // ''' はブロック文字列として処理済みなのでここでは通常文字列
                // 次の2文字も ' かチェック
                if self.peek_next() == Some('\'') && 
                   self.input.get(self.position + 2) == Some(&'\'') {
                    // ''' で始まる場合は既にブロック文字列として処理されているはず
                    // ここに来るのは ''' の2文字目以降の ' のみ
                }
                
                self.advance();
                text.push('\'');
                self.emit(SyntaxKind::SingleQuoteString, text, start_line, start_col);
                return;
            }
            
            if c == '\\' {
                self.advance();
                text.push('\\');
                if let Some(escaped) = self.advance() {
                    text.push(escaped);
                }
            } else if c == '\n' || c == '\r' {
                self.errors.push(LexError::new(
                    "Unterminated string literal",
                    start_line,
                    start_col,
                ));
                self.emit(SyntaxKind::SingleQuoteString, text, start_line, start_col);
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
        self.emit(SyntaxKind::SingleQuoteString, text, start_line, start_col);
    }
    
    /// 単語（識別子、数値、特殊値、引用符なし文字列）を読み込み
    fn read_word_or_number(&mut self,
        start_line: usize,
        start_col: usize,
    ) {
        let mut text = String::new();
        let _start_pos = self.position;
        
        // マイナス記号の処理（-Infinity, 負の数値）
        if self.peek() == Some('-') && self.peek_next().map(|c| c.is_ascii_digit() || c == 'I').unwrap_or(false) {
            text.push(self.advance().unwrap());
        }
        
        // 単語文字を収集
        while let Some(c) = self.peek() {
            // 区切り文字で終了
            if c.is_whitespace() 
                || c == '{' || c == '}' || c == '[' || c == ']' 
                || c == ':' || c == ',' || c == '"' || c == '\'' 
                || c == '#' || c == '/' {
                break;
            }
            text.push(c);
            self.advance();
        }
        
        if text.is_empty() {
            // 不明な文字
            let c = self.advance().unwrap();
            self.errors.push(LexError::new(
                format!("Unexpected character: {}", c),
                start_line,
                start_col,
            ));
            return;
        }
        
        // 型判定
        let kind = self.classify_word(&text);
        self.emit(kind, text, start_line, start_col);
    }
    
    /// 単語を分類
    fn classify_word(&self, text: &str) -> SyntaxKind {
        match text {
            "true" => SyntaxKind::True,
            "false" => SyntaxKind::False,
            "null" => SyntaxKind::Null,
            "NaN" => SyntaxKind::NaN,
            "Infinity" => SyntaxKind::Infinity,
            "-Infinity" => SyntaxKind::NegInfinity,
            _ => {
                // 16進数かチェック
                if text.starts_with("0x") || text.starts_with("0X") {
                    if text[2..].chars().all(|c| c.is_ascii_hexdigit()) {
                        return SyntaxKind::HexInteger;
                    }
                }
                
                // 数値かチェック
                if text.chars().next().map(|c| c.is_ascii_digit() || c == '-' || c == '+').unwrap_or(false) {
                    if text.parse::<f64>().is_ok() || text.parse::<i64>().is_ok() {
                        if text.contains('.') || text.contains('e') || text.contains('E') {
                            return SyntaxKind::Float;
                        } else {
                            return SyntaxKind::Integer;
                        }
                    }
                }
                
                // 識別子か引用符なし文字列か判定
                // HJSONではキーも値も引用符なし可
                if text.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    SyntaxKind::Ident
                } else {
                    SyntaxKind::UnquotedString
                }
            }
        }
    }
}

/// 公開API
pub fn lex(input: &str) -> LexResult {
    Lexer::new(input).lex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::kind::is_trivia;

    #[test]
    fn test_basic_tokens() {
        let input = "{ } [ ] : ,";
        let result = lex(input);

        assert!(result.errors.is_empty());
        // 空白もトークンとして出力される
        assert_eq!(result.tokens.len(), 11);

        let non_trivia_kinds: Vec<_> = result.tokens.iter()
            .filter(|t| !is_trivia(t.kind))
            .map(|t| t.kind)
            .collect();
        assert_eq!(non_trivia_kinds, vec![
            SyntaxKind::LBrace,
            SyntaxKind::RBrace,
            SyntaxKind::LBracket,
            SyntaxKind::RBracket,
            SyntaxKind::Colon,
            SyntaxKind::Comma,
        ]);
    }
    
    #[test]
    fn test_comments() {
        let input = "// line comment\n# hash comment\n/* block */";
        let result = lex(input);
        
        assert!(result.errors.is_empty());
        let comment_kinds: Vec<_> = result.tokens.iter()
            .filter(|t| matches!(t.kind, SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment))
            .map(|t| t.kind)
            .collect();
        
        assert_eq!(comment_kinds, vec![
            SyntaxKind::LineComment,
            SyntaxKind::HashComment,
            SyntaxKind::BlockComment,
        ]);
    }
    
    #[test]
    fn test_special_values() {
        let input = "true false null NaN Infinity -Infinity";
        let result = lex(input);
        
        assert!(result.errors.is_empty());
        let kinds: Vec<_> = result.tokens.iter()
            .filter(|t| !matches!(t.kind, SyntaxKind::Whitespace))
            .map(|t| t.kind)
            .collect();
        
        assert_eq!(kinds, vec![
            SyntaxKind::True,
            SyntaxKind::False,
            SyntaxKind::Null,
            SyntaxKind::NaN,
            SyntaxKind::Infinity,
            SyntaxKind::NegInfinity,
        ]);
    }
    
    #[test]
    fn test_hex_number() {
        let input = "0xFF 0x1a3b";
        let result = lex(input);
        
        assert!(result.errors.is_empty());
        let hex_tokens: Vec<_> = result.tokens.iter()
            .filter(|t| t.kind == SyntaxKind::HexInteger)
            .collect();
        
        assert_eq!(hex_tokens.len(), 2);
    }
    
    #[test]
    fn test_block_string() {
        let input = "'''multi\nline\ntext'''";
        let result = lex(input);
        
        assert!(result.errors.is_empty());
        let block_strings: Vec<_> = result.tokens.iter()
            .filter(|t| t.kind == SyntaxKind::BlockString)
            .collect();
        
        assert_eq!(block_strings.len(), 1);
    }
    
    #[test]
    fn test_whitespace() {
        let input = "{ }";
        let result = lex(input);
        
        assert!(result.errors.is_empty());
        let kinds: Vec<_> = result.tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds, vec![
            SyntaxKind::LBrace,
            SyntaxKind::Whitespace,
            SyntaxKind::RBrace,
        ]);
    }
}
