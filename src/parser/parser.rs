/// HJSON構文解析器
/// 
/// Lexerのトークン列をGreen Treeに変換
use super::lexer::Token;
use super::super::syntax::green::GreenTreeBuilder;
use super::super::syntax::kind::SyntaxKind;

/// 構文解析エラー
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl ParseError {
    fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

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
    builder: GreenTreeBuilder,
    errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            builder: GreenTreeBuilder::new(),
            errors: Vec::new(),
        }
    }
    
    /// 構文解析実行
    pub fn parse(mut self) -> ParseResult {
        self.builder.start_node(SyntaxKind::Root);
        
        // 先頭のトリビアを含めて値をパース
        if !self.is_at_end() {
            self.parse_value();
        }
        
        // 残りのトークンがあればエラー
        if !self.is_at_end() {
            let token = self.current().unwrap();
            self.error(
                format!("Unexpected token: {:?}", token.kind),
                token.line,
                token.column,
            );
            // 残りをエラーノードとして回収（トリビアも含めて）
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
            self.builder.token(token.kind, &token.text);
            self.position += 1;
        }
    }
    
    /// 特定の非トリビアトークンを期待して消費（前のトリビアも含めて）
    fn expect(&mut self, kind: SyntaxKind, message: &str) -> bool {
        // 先にトリビアを消費
        self.eat_trivia();
        
        if self.current().map(|t| t.kind == kind).unwrap_or(false) {
            self.bump();
            true
        } else {
            let (line, col) = if let Some(t) = self.current() {
                (t.line, t.column)
            } else {
                (0, 0)
            };
            self.error(message, line, col);
            false
        }
    }
    
    /// 特定の非トリビアトークンかチェック
    fn at(&self, kind: SyntaxKind) -> bool {
        self.current_kind() == Some(kind)
    }

    /// いずれかの非トリビアトークンかチェック
    fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        self.current_kind().map(|k| kinds.contains(&k)).unwrap_or(false)
    }
    
    /// エラーを記録
    fn error(
        &mut self,
        message: impl Into<String>,
        line: usize,
        column: usize,
    ) {
        self.errors.push(ParseError::new(message, line, column));
    }
    
    /// エラーノードを開始
    fn start_error_node(&mut self) {
        self.builder.start_node(SyntaxKind::Error);
    }

    /// エラーノードを終了
    fn finish_error_node(&mut self) {
        self.builder.finish_node();
    }

    /// 回復点までトークンを消費（トリビア含めてすべて保持）
    fn consume_until_recovery(&mut self, recovery_kinds: &[SyntaxKind]) {
        while !self.is_at_end() && !self.at_any(recovery_kinds) {
            self.bump(); // トリビアも含めてすべて追加
        }
    }

    /// トリビア（空白・コメント・改行）を消費して追加
    /// 
    /// トリビアをGreen Treeに追加しつつ、現在位置を進める
    fn eat_trivia(&mut self) {
        while let Some(token) = self.current() {
            if super::super::syntax::kind::is_trivia(token.kind) {
                self.bump();
            } else {
                break;
            }
        }
    }
    
    /// 次の非トリビアトークンの種類を取得
    fn current_kind(&self) -> Option<SyntaxKind> {
        let mut pos = self.position;
        while let Some(token) = self.tokens.get(pos) {
            if super::super::syntax::kind::is_trivia(token.kind) {
                pos += 1;
            } else {
                return Some(token.kind);
            }
        }
        None
    }
    
    /// 次の非トリビアトークンまでスキップ（ eat_trivia() を使うため非推奨）
    fn skip_trivia(&mut self) {
        self.eat_trivia();
    }
    
    /// 値を解析
    fn parse_value(&mut self) {
        self.builder.start_node(SyntaxKind::Value);

        // 先頭のトリビアを消費（インデントなど）
        self.eat_trivia();

        // 終了チェック（トリビアのみの場合）
        if self.is_at_end() {
            self.builder.finish_node();
            return;
        }

        // 現在のトークンを確認（トリビア以外）
        if let Some(token_kind) = self.current_kind() {
            match token_kind {
                SyntaxKind::LBrace => {
                    self.parse_object();
                }
                SyntaxKind::LBracket => {
                    self.parse_array();
                }
                SyntaxKind::String |
                SyntaxKind::SingleQuoteString |
                SyntaxKind::BlockString |
                SyntaxKind::UnquotedString |
                SyntaxKind::Ident => {
                    self.parse_string_or_key();
                }
                SyntaxKind::Integer |
                SyntaxKind::Float |
                SyntaxKind::HexInteger |
                SyntaxKind::NaN |
                SyntaxKind::Infinity |
                SyntaxKind::NegInfinity => {
                    self.eat_trivia();
                    self.bump();
                }
                SyntaxKind::True |
                SyntaxKind::False |
                SyntaxKind::Null => {
                    self.eat_trivia();
                    self.bump();
                }
                _ => {
                    // 予期しないトークン - エラーノードとして回収
                    let (line, col) = if let Some(t) = self.current() {
                        (t.line, t.column)
                    } else {
                        (0, 0)
                    };
                    self.start_error_node();
                    self.error(
                        format!("Unexpected token: {:?}", token_kind),
                        line,
                        col,
                    );
                    self.bump();
                    self.finish_error_node();
                }
            }
        }

        self.builder.finish_node();
    }
    
    /// オブジェクト { ... } を解析
    fn parse_object(
        &mut self,
    ) {
        self.builder.start_node(SyntaxKind::Object);

        // {
        self.bump();

        // 空のオブジェクトかチェック
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
                let (line, col) = if self.position > 0 {
                    let prev = &self.tokens[self.position - 1];
                    (prev.line, prev.column + prev.text.len())
                } else {
                    (1, 1)
                };
                self.error("Unclosed object, expected '}'", line, col);
                break;
            }

            self.parse_pair();

            // Pairの後のトリビアを消費（コメントなど）
            self.eat_trivia();

            // カンマ
            if self.at(SyntaxKind::Comma) {
                self.eat_trivia();
                self.bump();

                // 末尾カンマ対応：}が続く場合は終了
                if self.at(SyntaxKind::RBrace) {
                    break;
                }
            } else if !self.at(SyntaxKind::RBrace) && !self.is_at_end() {
                let token = self.current().unwrap();
                self.error(
                    "Expected ',' or '}'",
                    token.line,
                    token.column,
                );
                // エラー回復：次のペアまたは}までスキップ（トリビア含めて保持）
                self.start_error_node();
                self.consume_until_recovery(&[
                    SyntaxKind::Comma,
                    SyntaxKind::RBrace,
                    SyntaxKind::String,
                    SyntaxKind::SingleQuoteString,
                    SyntaxKind::Ident,
                    SyntaxKind::UnquotedString,
                ]);
                self.finish_error_node();
            }
        }

        // }
        if !self.at(SyntaxKind::RBrace) && !self.is_at_end() {
            let token = self.current().unwrap();
            self.error("Expected '}'", token.line, token.column);
        } else {
            self.bump();
        }

        self.builder.finish_node();
    }
    
    /// 配列 [ ... ] を解析
    fn parse_array(
        &mut self,
    ) {
        self.builder.start_node(SyntaxKind::Array);

        // [
        self.bump();

        // 空の配列かチェック
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
                let (line, col) = if self.position > 0 {
                    let prev = &self.tokens[self.position - 1];
                    (prev.line, prev.column + prev.text.len())
                } else {
                    (1, 1)
                };
                self.error("Unclosed array, expected ']'", line, col);
                break;
            }

            self.parse_value();

            // カンマ
            if self.at(SyntaxKind::Comma) {
                self.bump();

                // 末尾カンマ対応
                if self.at(SyntaxKind::RBracket) {
                    break;
                }
            } else if !self.at(SyntaxKind::RBracket) && !self.is_at_end() {
                let token = self.current().unwrap();
                self.error(
                    "Expected ',' or ']'",
                    token.line,
                    token.column,
                );
                // エラー回復（トリビア含めて保持）
                self.start_error_node();
                self.consume_until_recovery(&[
                    SyntaxKind::Comma,
                    SyntaxKind::RBracket,
                ]);
                self.finish_error_node();
            }
        }

        // ]
        if !self.at(SyntaxKind::RBracket) && !self.is_at_end() {
            let token = self.current().unwrap();
            self.error("Expected ']'", token.line, token.column);
        } else {
            self.bump();
        }

        self.builder.finish_node();
    }
    
    /// キー: 値 のペアを解析
    fn parse_pair(&mut self) {
        self.builder.start_node(SyntaxKind::Pair);

        // 先頭のトリビアを消費（インデントなど）
        self.eat_trivia();

        // キー
        self.builder.start_node(SyntaxKind::Key);

        // トリビアを消費してからキーを確認
        self.eat_trivia();

        let token = self.current();
        match token.map(|t| t.kind) {
            Some(SyntaxKind::String) |
            Some(SyntaxKind::SingleQuoteString) |
            Some(SyntaxKind::Ident) |
            Some(SyntaxKind::UnquotedString) => {
                self.bump();
            }
            _ => {
                if let Some(t) = token {
                    self.error(
                        format!("Expected key, found {:?}", t.kind),
                        t.line,
                        t.column,
                    );
                    // エラートークンを含めて回収
                    self.start_error_node();
                    self.bump();
                    self.finish_error_node();
                } else {
                    self.error("Expected key", 0, 0);
                }
                self.builder.finish_node();
                self.builder.finish_node();
                return;
            }
        }

        self.builder.finish_node();

        // :
        if !self.at(SyntaxKind::Colon) {
            let (line, col) = if let Some(t) = self.current() {
                (t.line, t.column)
            } else {
                (0, 0)
            };
            self.error("Expected ':' after key", line, col);
            // エラー回復：値が始まるまでをエラーノードとして回収
            self.start_error_node();
            self.consume_until_recovery(&[
                SyntaxKind::LBrace,
                SyntaxKind::LBracket,
                SyntaxKind::String,
                SyntaxKind::SingleQuoteString,
                SyntaxKind::BlockString,
                SyntaxKind::UnquotedString,
                SyntaxKind::Ident,
                SyntaxKind::Integer,
                SyntaxKind::Float,
                SyntaxKind::HexInteger,
                SyntaxKind::True,
                SyntaxKind::False,
                SyntaxKind::Null,
                SyntaxKind::NaN,
                SyntaxKind::Infinity,
                SyntaxKind::NegInfinity,
                SyntaxKind::Comma,
                SyntaxKind::RBrace,
            ]);
            self.finish_error_node();
        } else {
            // eat_trivia()はat()の中で既に呼ばれているので、ここでは直接bump()
            self.eat_trivia(); // コロンの前のトリビアを消費
            self.bump(); // コロンを消費
        }

        // 値
        self.parse_value();

        self.builder.finish_node();
    }
    
    /// 文字列またはキーを解析
    fn parse_string_or_key(
        &mut self,
    ) {
        // 単純にトークンを消費
        self.bump();
    }
}

/// 公開API
pub fn parse(input: &str) -> Result<ParseResult, Vec<ParseError>> {
    let lex_result = super::lexer::lex(input);
    
    // 字句解析エラーがある場合
    if !lex_result.errors.is_empty() {
        let parse_errors: Vec<ParseError> = lex_result.errors.iter().map(|e| {
            ParseError::new(&e.message,
                e.line,
                e.column,
            )
        }).collect();
        return Err(parse_errors);
    }
    
    let parser = Parser::new(lex_result.tokens);
    let result = parser.parse();
    
    if result.errors.is_empty() {
        Ok(result)
    } else {
        Err(result.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::syntax::green::GreenNodeExt;
    
    #[test]
    fn test_parse_empty_object() {
        let result = parse("{}").unwrap();
        let text = result.green.text();
        assert_eq!(text, "{}");
    }
    
    #[test]
    fn test_parse_simple_object() {
        let input = r#"{"key": "value"}"#;
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }
    
    #[test]
    fn test_parse_array() {
        let input = "[1, 2, 3]";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }
    
    #[test]
    fn test_parse_with_comments() {
        let input = r#"{
            // This is a comment
            "key": "value" # another comment
        }"#;
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }
    
    #[test]
    fn test_parse_unquoted_keys() {
        let input = "{key: value}";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }
    
    #[test]
    fn test_parse_trailing_comma() {
        let input = "[1, 2,]";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }
    
    #[test]
    fn test_parse_hex_and_special() {
        let input = "{hex: 0xFF, special: NaN}";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }
    
    #[test]
    fn test_parse_error() {
        let input = "{key}";
        let result = parse(input);
        assert!(result.is_err());
    }
}
