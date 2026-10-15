/// HJSON構文解析器
///
/// Lexerのトークン列をGreen Treeに変換
use super::lexer::{Lexer, Token};
use super::scan;
use super::SyntaxKind;
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

/// コンテナ（オブジェクト・配列）のネスト上限。
///
/// 再帰下降の深さを制限し、スタックオーバーフローと深さに由来する
/// 二次関数的な処理時間を防ぐ。一般的な設定ファイルの深さ（実運用の
/// 設定ファイル群 542 件での最大は 7）に対して十分な余裕がある。
pub const MAX_NESTING_DEPTH: usize = 128;

/// 構文解析の結果と、ルートをブレースレスとして解釈したかどうか。
struct ParseOutcome {
    result: ParseResult,
    braceless: bool,
}

/// 構文解析器
pub struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    position: usize,
    builder: rowan::GreenNodeBuilder<'static>,
    errors: Vec<ParseError>,
    depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueBoundary {
    Closed,
    Open,
}

/// ルートの解釈方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootMode {
    /// ブレースレスオブジェクトを試し、成立しなければ単一値として解釈する。
    Auto,
    /// 常に単一値として解釈する。
    SingleValue,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            position: 0,
            builder: rowan::GreenNodeBuilder::new(),
            errors: Vec::new(),
            depth: 0,
        }
    }

    fn error(&mut self, offset: usize, message: impl Into<String>) {
        self.errors.push(ParseError::new(offset, message));
    }

    /// ネスト上限の範囲内なら深さを進めて true を返す。
    fn enter_container(&mut self) -> bool {
        if self.depth >= MAX_NESTING_DEPTH {
            let offset = self.current().map_or(0, |token| token.offset);
            let limit = MAX_NESTING_DEPTH;
            self.error(offset, format!("Nesting too deep: limit is {limit}"));
            return false;
        }

        self.depth += 1;
        true
    }

    /// 上限を超えたコンテナの中身を再帰せずに読み飛ばす。
    ///
    /// 開き括弧は呼び出し元が消費済みであることを前提に、対応する閉じ括弧まで
    /// 入れ子を数えながら消費する。
    fn skip_container(&mut self, open: SyntaxKind, close: SyntaxKind) {
        self.start_error_node();
        self.bump();

        let mut level = 0usize;
        while !self.is_at_end() {
            match self.current().map(|token| token.kind) {
                Some(kind) if kind == open => level += 1,
                Some(kind) if kind == close => {
                    if level == 0 {
                        self.bump();
                        break;
                    }
                    level -= 1;
                }
                _ => {}
            }

            if self.current().is_some_and(|token| token.kind.is_trivia()) {
                self.bump_trivia();
            } else {
                self.bump();
            }
        }

        self.finish_error_node();
    }

    /// 値・キーの先頭に現れた未終端の文字列を検出する。
    ///
    /// 字句解析器は quoteless 値の途中に現れる `"` / `'''` を文字列開始とみなさない。
    /// したがって、それらが値・キーの先頭に現れた場合は未終端の文字列である。
    fn reject_unterminated_string(&mut self) {
        let Some(token) = self
            .tokens
            .get(self.position..)
            .and_then(|rest| rest.iter().find(|t| !t.kind.is_trivia()))
        else {
            return;
        };

        if matches!(token.kind, SyntaxKind::String | SyntaxKind::BlockString) {
            return;
        }

        let message = if token.text.starts_with('"') {
            "Unterminated string literal"
        } else if token.text.starts_with("'''") {
            "Unterminated block string"
        } else {
            return;
        };

        self.error(token.offset, message);
    }

    /// 構文解析実行。
    ///
    /// ルートの解釈は基準実装（hjson-cs）と同じ規則にする。まずブレースレス
    /// オブジェクトとして解釈し、成立しなければ単一値として解釈し直す。
    pub fn parse(self) -> ParseResult {
        let source = self.source;
        let tokens = self.tokens.clone();

        let first = Parser::new(source, tokens.clone()).parse_inner(RootMode::Auto);
        if !first.braceless || first.result.errors.is_empty() {
            return first.result;
        }

        // ブレースレス解釈が失敗した場合のみ、単一値として解釈し直す
        let retry = Parser::new(source, tokens).parse_inner(RootMode::SingleValue);
        if retry.result.errors.is_empty() {
            return retry.result;
        }
        first.result
    }

    fn parse_inner(mut self, mode: RootMode) -> ParseOutcome {
        self.builder.start_node(SyntaxKind::Root.into());

        let mut has_body = false;
        let mut braceless = false;

        if !self.is_at_end() {
            if matches!(mode, RootMode::Auto) && self.is_braceless_object_start() {
                braceless = true;
                self.eat_root_header_trivia_for_braceless_object();
                self.depth += 1;
                self.parse_braceless_object();
                self.depth -= 1;
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

        ParseOutcome {
            result: ParseResult {
                green: self.builder.finish(),
                errors: self.errors,
            },
            braceless,
        }
    }

    /// 生テキストの範囲 `[.., end)` に含まれるトークンを消費する。
    ///
    /// 範囲の末端でトークンが切れる場合（例: `{a//b: 1}` のキー）は、残りを
    /// 再字句解析してトークン列を置き換える。トークン境界と文法上の単位は
    /// 一致しないため、消費はオフセットで行う。
    fn consume_raw_until(&mut self, end: usize) {
        while let Some(token) = self.tokens.get(self.position) {
            if token.offset >= end {
                break;
            }
            let token_end = token.offset + token.text.len();
            if token_end <= end {
                self.position += 1;
                continue;
            }

            // 残り全体を再字句解析する。単位がトークンをまたぐ場合（例: キーの途中から
            // 始まる複数行文字列）に、分割後のトークン列が不整合にならないようにする。
            let tail = &self.source[end..];
            let replacement: Vec<Token> = Lexer::new(tail)
                .lex()
                .tokens
                .into_iter()
                .map(|mut token| {
                    token.offset += end;
                    token
                })
                .collect();
            self.tokens.splice(self.position.., replacement);
            break;
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
        self.reject_unterminated_string();

        // 値ノード自体には先頭トリビアを含めない（呼び出し側が先に消費する）
        if self.is_at_end() {
            let offset = self.source.len();
            self.error(offset, "Incomplete input");
            self.builder.finish_node();
            return ValueBoundary::Open;
        }

        // 現在のトークンを確認（トリビア以外）
        if let Some(token_kind) = self.current_kind() {
            match token_kind {
                SyntaxKind::LBrace => {
                    if self.enter_container() {
                        self.parse_object();
                        self.depth -= 1;
                    } else {
                        self.skip_container(SyntaxKind::LBrace, SyntaxKind::RBrace);
                    }
                    self.builder.finish_node();
                    return ValueBoundary::Closed;
                }
                SyntaxKind::LBracket => {
                    if self.enter_container() {
                        self.parse_array();
                        self.depth -= 1;
                    } else {
                        self.skip_container(SyntaxKind::LBracket, SyntaxKind::RBracket);
                    }
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
        self.reject_unterminated_string();
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
                let start = self.current().map_or(0, |token| token.offset);
                match scan::scan_key(self.source, start) {
                    Ok(key) => {
                        let text = &self.source[key.name_start..key.name_end];
                        let kind = if text.starts_with('"') || text.starts_with('\'') {
                            SyntaxKind::String
                        } else {
                            SyntaxKind::UnquotedString
                        };
                        self.consume_raw_until(key.end);
                        self.builder.token(kind.into(), text);
                    }
                    Err(error) => {
                        self.error(error.offset, error.message);
                        self.bump();
                    }
                }
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
                    let start_position = self.position;
                    let end = scan::scan_quoteless_value(self.source, start_offset);

                    if end <= start_offset {
                        self.bump();
                        return ValueBoundary::Closed;
                    }

                    self.consume_raw_until(end);
                    let text = &self.source[start_offset..end];
                    let kind = scan::classify_value(text);
                    self.builder.token(kind.into(), text);

                    if self.position - start_position > 1 {
                        ValueBoundary::Open
                    } else {
                        ValueBoundary::Closed
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
        while let Some(token) = self.tokens.get(pos) {
            if token.kind.is_trivia() {
                pos += 1;
                continue;
            }
            return scan::scan_key(self.source, token.offset).is_ok();
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

    fn nested_arrays(depth: usize) -> String {
        format!("{}{}{}", "[".repeat(depth), "1", "]".repeat(depth))
    }

    #[test]
    fn test_nesting_at_limit_is_accepted() {
        let src = nested_arrays(MAX_NESTING_DEPTH);
        let parsed = parse_text(&src);
        assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    }

    #[test]
    fn test_nesting_beyond_limit_is_rejected() {
        let parsed = parse_text(&nested_arrays(MAX_NESTING_DEPTH + 1));
        assert!(parsed
            .errors
            .iter()
            .any(|e| e.message.contains("Nesting too deep")));
    }

    #[test]
    fn test_object_nesting_beyond_limit_is_rejected() {
        let src = format!(
            "{}1{}",
            "{a:".repeat(MAX_NESTING_DEPTH + 1),
            "}".repeat(MAX_NESTING_DEPTH + 1)
        );
        let parsed = parse_text(&src);
        assert!(parsed
            .errors
            .iter()
            .any(|e| e.message.contains("Nesting too deep")));
    }

    #[test]
    fn test_deep_nesting_does_not_panic_and_keeps_all_text() {
        let src = nested_arrays(50_000);
        let parsed = parse_text(&src);
        assert!(parsed
            .errors
            .iter()
            .any(|e| e.message.contains("Nesting too deep")));

        // 読み飛ばした分も含めて入力をすべて保持する
        let text_len: usize = crate::syntax::RedNode::new_root(parsed.green)
            .text()
            .len()
            .into();
        assert_eq!(text_len, src.len());
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
