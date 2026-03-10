use super::super::syntax::green::GreenTreeBuilder;
use super::super::syntax::kind::SyntaxKind;
/// HJSON構文解析器
///
/// Lexerのトークン列をGreen Treeに変換
use super::lexer::Token;

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
            self.eat_trivia();
            // ブレースレスルートオブジェクトのチェック
            if self.is_braceless_object_start() {
                self.parse_braceless_object();
            } else {
                self.parse_value();
            }
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
    #[allow(dead_code)]
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
        self.current_kind()
            .map(|k| kinds.contains(&k))
            .unwrap_or(false)
    }

    /// エラーを記録
    fn error(&mut self, message: impl Into<String>, line: usize, column: usize) {
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

    fn eat_trivia_and_report_newline(&mut self) -> bool {
        let mut had_newline = false;
        while let Some(token) = self.current() {
            if super::super::syntax::kind::is_trivia(token.kind) {
                if token.kind == SyntaxKind::Newline {
                    had_newline = true;
                }
                self.bump();
            } else {
                break;
            }
        }
        had_newline
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

    /// 次の非トリビアトークンまでスキップ（eat_trivia()のエイリアス）
    #[allow(dead_code)]
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
                SyntaxKind::String
                | SyntaxKind::BlockString
                | SyntaxKind::UnquotedString
                | SyntaxKind::Ident
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null => self.parse_scalar_value(),
                _ => {
                    // 予期しないトークン - エラーノードとして回収
                    let (line, col) = if let Some(t) = self.current() {
                        (t.line, t.column)
                    } else {
                        (0, 0)
                    };
                    self.start_error_node();
                    self.error(format!("Unexpected token: {:?}", token_kind), line, col);
                    self.bump();
                    self.finish_error_node();
                }
            }
        }

        self.builder.finish_node();
    }

    /// オブジェクト { ... } を解析
    fn parse_object(&mut self) {
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

            // Pair前のトリビアを消費（改行・インデントなど）
            self.eat_trivia();
            // トリビア消費後に } が来たら終了、またはEOFの場合もループ先頭に戻って適切なエラーを出力
            if self.at(SyntaxKind::RBrace) || self.is_at_end() {
                break;
            }

            self.parse_pair();

            // Pairの後のトリビアを消費（コメントなど）
            let had_newline = self.eat_trivia_and_report_newline();

            // カンマ
            if self.at(SyntaxKind::Comma) {
                self.bump();

                // カンマ後のトリビア（改行・コメントなど）を消費
                self.eat_trivia();

                // 末尾カンマ対応：}が続く場合は終了
                if self.at(SyntaxKind::RBrace) {
                    break;
                }
            } else if !self.at(SyntaxKind::RBrace) && !self.is_at_end() {
                // 改行（トリビア）が消費されていればHJSONの改行区切りとして有効
                if had_newline {
                    // カンマなし・改行ありは有効な区切り → ループ継続
                    continue;
                }
                // 改行もなくカンマもない → エラー
                let token = self.current().unwrap();
                self.error("Expected ',' or '}'", token.line, token.column);
                // エラー回復：次のペアまたは}までスキップ（トリビア含めて保持）
                self.start_error_node();
                self.consume_until_recovery(&[
                    SyntaxKind::Comma,
                    SyntaxKind::RBrace,
                    SyntaxKind::String,
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
    fn parse_array(&mut self) {
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
                let token = self.current().unwrap();
                self.error("Expected ',' or ']'", token.line, token.column);
                // エラー回復（トリビア含めて保持）
                self.start_error_node();
                self.consume_until_recovery(&[SyntaxKind::Comma, SyntaxKind::RBracket]);
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

        // キー
        self.builder.start_node(SyntaxKind::Key);

        let token = self.current();
        match token.map(|t| t.kind) {
            Some(SyntaxKind::String)
            | Some(SyntaxKind::Ident)
            | Some(SyntaxKind::UnquotedString) => {
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
                SyntaxKind::BlockString,
                SyntaxKind::UnquotedString,
                SyntaxKind::Ident,
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
            // eat_trivia()はat()の中で既に呼ばれているので、ここでは直接bump()
            self.eat_trivia(); // コロンの前のトリビアを消費
            self.bump(); // コロンを消費
        }

        // 値
        self.parse_value();

        self.builder.finish_node();
    }

    fn parse_scalar_value(&mut self) {
        self.eat_trivia();
        if let Some(token) = self.current() {
            match token.kind {
                SyntaxKind::String | SyntaxKind::BlockString => self.bump(),
                SyntaxKind::Ident | SyntaxKind::UnquotedString => {
                    let (text, consumed) = self.collect_quoteless_scalar();
                    if consumed == 1 {
                        self.bump();
                    } else {
                        self.builder.token(SyntaxKind::UnquotedString, &text);
                        self.position += consumed;
                    }
                }
                SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null => {
                    let (text, consumed) = self.collect_quoteless_scalar();
                    if consumed == 1 {
                        self.bump();
                    } else {
                        self.builder.token(SyntaxKind::UnquotedString, &text);
                        self.position += consumed;
                    }
                }
                _ => self.bump(),
            }
        }
    }

    fn collect_quoteless_scalar(&self) -> (String, usize) {
        let mut pos = self.position;
        let mut text = String::new();
        let mut last_non_ws_len = 0usize;
        let mut last_non_ws_pos = self.position;

        while let Some(token) = self.tokens.get(pos) {
            match token.kind {
                SyntaxKind::Newline | SyntaxKind::RBrace | SyntaxKind::RBracket => break,
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                    if text.ends_with(' ') || text.ends_with('\t') =>
                {
                    break;
                }
                SyntaxKind::Whitespace
                | SyntaxKind::LineComment
                | SyntaxKind::HashComment
                | SyntaxKind::BlockComment
                | SyntaxKind::Ident
                | SyntaxKind::UnquotedString
                | SyntaxKind::Integer
                | SyntaxKind::Float
                | SyntaxKind::True
                | SyntaxKind::False
                | SyntaxKind::Null
                | SyntaxKind::String
                | SyntaxKind::BlockString
                | SyntaxKind::Comma
                | SyntaxKind::Colon
                | SyntaxKind::LBrace
                | SyntaxKind::LBracket => {
                    text.push_str(&token.text);
                    pos += 1;
                    if token.kind != SyntaxKind::Whitespace {
                        last_non_ws_len = text.len();
                        last_non_ws_pos = pos;
                    }
                    if matches!(
                        token.kind,
                        SyntaxKind::LineComment | SyntaxKind::HashComment
                    ) {
                        break;
                    }
                }
                _ => break,
            }
        }

        text.truncate(last_non_ws_len);
        (text, last_non_ws_pos - self.position)
    }

    fn has_colon_after_key(&self) -> bool {
        let mut pos = self.position;
        let mut consumed_key = false;

        while let Some(token) = self.tokens.get(pos) {
            if super::super::syntax::kind::is_trivia(token.kind) {
                pos += 1;
                continue;
            }

            if !consumed_key {
                if matches!(token.kind, SyntaxKind::Ident | SyntaxKind::String) {
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

    /// ブレースレスルートオブジェクトの開始かチェック
    /// 引用符なしキー（Ident/UnquotedString）のみで開始される
    fn is_braceless_object_start(&self) -> bool {
        matches!(
            self.current_kind(),
            Some(SyntaxKind::Ident) | Some(SyntaxKind::String)
        ) && self.has_colon_after_key()
    }

    /// ブレースレスルートオブジェクトを解析（中括弧なし）
    fn parse_braceless_object(&mut self) {
        self.builder.start_node(SyntaxKind::Object);
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

            self.parse_pair();

            let had_separator_newline = self.eat_trivia_and_report_newline();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.eat_trivia();
            } else if !self.is_at_end() && !had_separator_newline {
                break;
            }
        }

        self.builder.finish_node();
    }
}

/// 公開API
pub fn parse(input: &str) -> Result<ParseResult, Vec<ParseError>> {
    let lex_result = super::lexer::lex(input);

    // 字句解析エラーがある場合
    if !lex_result.errors.is_empty() {
        let parse_errors: Vec<ParseError> = lex_result
            .errors
            .iter()
            .map(|e| ParseError::new(&e.message, e.line, e.column))
            .collect();
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
    use super::super::super::syntax::green::GreenNodeExt;
    use super::*;

    /// 空オブジェクト "{}" のパースとラウンドトリップを検証
    #[test]
    fn test_parse_empty_object() {
        let result = parse("{}").unwrap();
        let text = result.green.text();
        assert_eq!(text, "{}");
    }

    /// 1つのキー・値ペアを持つオブジェクトのパースとラウンドトリップを検証
    #[test]
    fn test_parse_simple_object() {
        let input = r#"{"key": "value"}"#;
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }

    /// 整数3要素の配列のパースとラウンドトリップを検証
    #[test]
    fn test_parse_array() {
        let input = "[1, 2, 3]";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }

    /// 行コメント（// と #）を含むオブジェクトのパースとラウンドトリップを検証
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

    /// 引用符なしキー・値のパースとラウンドトリップを検証
    #[test]
    fn test_parse_unquoted_keys() {
        let input = "{key: value}";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }

    /// 末尾カンマ付き配列 "[1, 2,]" のパースとラウンドトリップを検証
    #[test]
    fn test_parse_trailing_comma() {
        let input = "[1, 2,]";
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }

    #[test]
    fn test_parse_quoted_key_in_object() {
        let input = r#"{"quoted key": 1, plain: two}"#;
        let result = parse(input).unwrap();
        let text = result.green.text();
        assert_eq!(text, input);
    }

    /// コロンなしの不正構文 "{key}" がErrになることを検証
    #[test]
    fn test_parse_error() {
        let input = "{key}";
        let result = parse(input);
        assert!(result.is_err());
    }
}
