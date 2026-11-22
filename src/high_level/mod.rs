//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

/// 編集APIのエラー型。
pub mod error;
mod format;
pub(crate) mod indent;
/// 文書を編集する操作（値・コメント・マージ）。
pub mod ops;
/// 値とコメントの位置を指すパス。
pub mod path;
pub(crate) mod resolve;
/// 文書の体裁とスタイルの推定。
pub mod style;
/// 値の型と描画。
pub mod value;
mod value_ops;
pub mod verify;

use crate::ast as semantic_ast;
use crate::ast::{AstNode, CommentKind, QuoteStyle};
use crate::format::FormatConfig;
use crate::high_level::error::HjsonEditError;
use crate::high_level::path::HjsonObjectPath;
use crate::high_level::style::detect_newline_from_ast;
use crate::syntax::lexer::Lexer;
use crate::syntax::parser::Parser;
use crate::syntax::MAX_INPUT_BYTES;
use crate::syntax::{NewlineKind, SyntaxKind};
use crate::RedNode;
use line_index::LineIndex;
use rowan::GreenNode;

/// スタイルの選び方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Prefer<T> {
    /// 元のスタイルを保つ。書けない値なら二重引用符へ切り替える。
    #[default]
    Preserve,
    /// 希望のスタイル。書けない値なら二重引用符へ切り替える。
    Preferred(T),
    /// 指定したスタイル。書けない値はエラーにする。
    Explicit(T),
}

/// パースした HJSON 文書。
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    root: RedNode,
    string_style: Prefer<QuoteStyle>,
    comment_style: Prefer<CommentKind>,
    verify: bool,
    format: Option<FormatConfig>,
    touched: Vec<HjsonObjectPath>,
    newline: String,
    /// 字下げ 1 段分（文書から推定する。編集では変わらない）。
    indent_unit: String,
    warnings: Vec<String>,
}

/// 入力長が上限（`MAX_INPUT_BYTES`）以内かを検査する。
///
/// 位置情報が u32 のため、それを超える入力は正しい位置を報告できない。
pub(crate) fn check_input_len(len: usize) -> Result<(), HjsonEditError> {
    if len > MAX_INPUT_BYTES {
        return Err(HjsonEditError::InputTooLong {
            len,
            limit: MAX_INPUT_BYTES,
        });
    }

    Ok(())
}

impl HjsonDocument {
    /// HJSON 文字列をパースして文書を作成する。
    ///
    /// 先頭の BOM は通常の文字として扱う。CLI はファイルと標準入力のどちらも、
    /// 読み込み時に先頭の BOM を除去してからこの関数に渡す。
    ///
    /// CLI の `edit --set` は値テキストもこの関数で解釈する。そのため `hello` は引用符
    /// なし文字列、`8080` は数値、`"true"` は文字列になる。値テキストにコメントが含まれる
    /// 場合は CLI がエラーにする（この関数はコメントを警告として読み飛ばすだけ）。
    ///
    /// # エラー
    ///
    /// - [`HjsonEditError::ParseError`] / [`HjsonEditError::LexError`]（行・列付き）
    /// - [`HjsonEditError::InputTooLong`]（[`MAX_INPUT_BYTES`] を超える入力）
    pub fn parse(input: &str) -> Result<Self, HjsonEditError> {
        check_input_len(input.len())?;

        let lex_result = Lexer::new(input).lex();
        let parser = Parser::new(input, lex_result.tokens);
        let result = parser.parse();

        // 再字句解析を始めた位置以降は、初回の字句解析結果が捨てられている。
        // そこで見つかった初回のエラーは報告しない（再字句解析側で検出し直す）。
        let relexed_from = result.relexed_from.unwrap_or(usize::MAX);

        // 閉じないブロックコメントは基準実装（hjson-cs）と同じく読み飛ばし、
        // 警告として保持する（切断の検出は CLI の役割）。
        let lex_errors: Vec<_> = lex_result
            .errors
            .iter()
            .filter(|error| error.offset < relexed_from)
            .collect();
        let (warnings, fatal): (Vec<_>, Vec<_>) = lex_errors
            .into_iter()
            .partition(|error| error.message == "Unterminated block comment");

        if !fatal.is_empty() {
            return Err(HjsonEditError::LexError {
                errors: fatal.into_iter().cloned().collect(),
                line_index: LineIndex::new(input),
                input: std::sync::Arc::from(input),
            });
        }

        let line_index = LineIndex::new(input);
        let mut warnings: Vec<String> = warnings
            .iter()
            .map(|error| lexer_warning(error, &line_index, input))
            .collect();

        // キーが `//` を含む場合の再字句解析で見つかったエラーも、初回と同じ扱いにする
        // （未終端のブロックコメントは警告、それ以外は字句エラー）
        let (retry_warnings, retry_fatal): (Vec<_>, Vec<_>) = result
            .lex_errors
            .iter()
            .partition(|error| error.message == "Unterminated block comment");
        if !retry_fatal.is_empty() {
            return Err(HjsonEditError::LexError {
                errors: retry_fatal.into_iter().cloned().collect(),
                line_index: LineIndex::new(input),
                input: std::sync::Arc::from(input),
            });
        }
        warnings.extend(
            retry_warnings
                .iter()
                .map(|error| lexer_warning(error, &line_index, input)),
        );

        if !result.errors.is_empty() {
            return Err(HjsonEditError::ParseError {
                errors: result.errors,
                line_index: LineIndex::new(input),
                input: std::sync::Arc::from(input),
            });
        }

        let root = RedNode::new_root(result.green);
        warnings.extend(duplicate_key_warnings(&root, input));
        let newline = detect_newline_from_ast(&root).as_str().to_string();
        let indent_unit = crate::high_level::indent::indent_unit(&root);

        Ok(Self {
            root,
            string_style: Prefer::default(),
            comment_style: Prefer::default(),
            verify: true,
            format: None,
            touched: Vec::new(),
            newline,
            indent_unit,
            warnings,
        })
    }

    /// パース時に見つけた警告を、出現順に返す。
    ///
    /// 現在は次の 2 種類。
    ///
    /// - 読み飛ばした入力（閉じていないブロックコメント）。
    ///   `skipped input at line N, column M: …` の形で、基準実装と同じく EOF までを
    ///   コメントとして飲み込む。その後ろに要素を足す編集は検証で失敗し、整形も
    ///   コメントの後ろに改行を足さない。
    /// - 値の層で後勝ちになる重複キー。`duplicate key "a" at line 3, column 3
    ///   (the last value wins)` の形。
    ///
    /// 警告はパースした入力についてのもので、編集した文書でも保持される
    /// （重複キーを削除しても警告は残る）。
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// 文字列のスタイルを指定する（既定は `Prefer::Preserve`）。
    ///
    /// 文書全体に適用する。CLI の `--string-style` も 1 回の実行全体に作用する
    /// （操作ごとには指定できない）。
    pub fn string_style(mut self, style: Prefer<QuoteStyle>) -> Self {
        self.string_style = style;
        self
    }

    /// コメントのスタイルを指定する（既定は `Prefer::Preserve`）。
    ///
    /// 文書全体に適用する。CLI の `--comment-style` も 1 回の実行全体に作用する。
    pub fn comment_style(mut self, style: Prefer<CommentKind>) -> Self {
        self.comment_style = style;
        self
    }

    pub(crate) fn root(self, root: GreenNode) -> HjsonDocument {
        HjsonDocument {
            root: RedNode::new_root(root),
            ..self
        }
    }

    pub(crate) fn current_string_style(&self) -> Prefer<QuoteStyle> {
        self.string_style
    }

    pub(crate) fn current_comment_style(&self) -> Prefer<CommentKind> {
        self.comment_style
    }

    /// 編集結果の検証（再パース・値一致・コメント差分）を有効にするか。既定は有効。
    ///
    /// 検証に失敗した編集は結果を返さずエラーになる。入力長に比例する追加コストが
    /// かかるため、巨大な文書では無効化できる。CLI の `--no-verify` はこれを無効化する。
    pub fn verify_edits(mut self, verify: bool) -> Self {
        self.verify = verify;
        self
    }

    pub(crate) fn current_verify(&self) -> bool {
        self.verify
    }

    /// 部分整形（`format_container`）で使う整形設定。既定は文書から検出したもの。
    pub fn format_config(mut self, config: FormatConfig) -> Self {
        self.format = Some(config);
        self
    }

    /// 文書に設定された整形設定。
    pub fn current_format_config(&self) -> FormatConfig {
        self.format
            .clone()
            .unwrap_or_else(|| self.detected_format_config())
    }

    /// 編集で子が変わったコンテナ（外側優先・重複排除済み）。
    ///
    /// `format_container` に渡すことで、変更した箇所だけを整形できる。
    pub fn touched_containers(&self) -> &[HjsonObjectPath] {
        &self.touched
    }

    /// 変更したコンテナを記録する。祖先に覆われるものは残さない。
    pub(crate) fn note_touched(mut self, path: HjsonObjectPath) -> Self {
        let covered = self
            .touched
            .iter()
            .any(|existing| path.segments().starts_with(existing.segments()));
        if !covered {
            self.touched
                .retain(|existing| !existing.segments().starts_with(path.segments()));
            self.touched.push(path);
        }
        self
    }

    /// パスが指す要素が行頭から始まるか。
    ///
    /// 行の途中にある要素を複数行にすると、親のレイアウトと混ざる。整形の可否を
    /// 判断するために使う。
    ///
    /// # エラー
    ///
    /// - パスが存在しない場合は [`HjsonEditError::PathNotFound`]
    pub fn starts_at_line(&self, path: &HjsonObjectPath) -> Result<bool, HjsonEditError> {
        let resolved = crate::high_level::resolve::PathResolver::new(self.syntax_root().clone())
            .resolve_node(path)?;

        Ok(crate::high_level::indent::is_at_line_start(&resolved))
    }

    /// 既存の値を、値テキストのまま差し替える。
    ///
    /// [`HjsonDocumentEdit::set`](crate::high_level::ops::object::HjsonDocumentEdit::set) は
    /// 値ノードを作り直すため、数値の表記（`1e3`）・区切りの空白（`{a: 1, b: 2}`）・
    /// 引用符の使い方が正規化される。この関数はテキストを解釈した結果をそのまま置くので、
    /// 書いたとおりの体裁が残る。
    ///
    /// インラインに置ける値だけを受け付ける。
    ///
    /// - 1 行であること（改行を含むテキストは [`HjsonEditError::InvalidEdit`]）
    /// - コメントを含まないこと
    /// - 引用符なし文字列で終わらないこと（直後の文字を飲み込むため）
    /// - ブレースレス（`a: 1` のようなキー行）でないこと
    ///
    /// パスは既存の値を指している必要がある（新規作成は `set` を使う）。
    /// 空のパスは文書全体の差し替えで、複数行のテキストもキー行も受け付ける。
    /// 本文（ブレースレス文書では末尾コメントを含む）は置き換わり、`Root` 直下の
    /// ヘッダー/フッターは残る。これは `set(path(""))` と同じ挙動。
    ///
    /// # Errors
    ///
    /// 上の条件を満たさないテキスト、パスが存在しない場合、検証に失敗した場合。
    pub fn set_value_text(self, path: HjsonObjectPath, text: &str) -> Result<Self, HjsonEditError> {
        let at_root = path.depth() == 0;

        if !at_root && (text.contains('\n') || text.contains('\r')) {
            return Err(HjsonEditError::InvalidEdit {
                target: path.to_string(),
                detail: "multi-line value text is not supported (use set instead)".to_string(),
            });
        }

        let parsed = HjsonDocument::parse(text)?;
        if parsed
            .syntax_root()
            .descendants_with_tokens()
            .any(|element| match element {
                rowan::NodeOrToken::Node(node) => node.kind().is_comment(),
                rowan::NodeOrToken::Token(token) => token.kind().is_comment(),
            })
        {
            return Err(HjsonEditError::InvalidEdit {
                target: path.to_string(),
                detail: "comments are not allowed in the value text".to_string(),
            });
        }

        let body = crate::high_level::resolve::resolve_body(parsed.syntax_root())?;
        if !at_root {
            let braceless_object = body.kind() == SyntaxKind::Object
                && crate::ast::Object::cast(body.clone())
                    .map(|object| object.delimiter_style == crate::ast::DelimiterStyle::Braceless)
                    .unwrap_or(false);
            if braceless_object {
                return Err(HjsonEditError::InvalidEdit {
                    target: path.to_string(),
                    detail: "a key line cannot be a value (use set instead)".to_string(),
                });
            }
            if body
                .last_token()
                .is_some_and(|token| token.kind() == SyntaxKind::UnquotedString)
            {
                return Err(HjsonEditError::InvalidEdit {
                    target: path.to_string(),
                    detail: "a value that ends with an unquoted string cannot be placed inline \
                             (quote the string to place it inline)"
                        .to_string(),
                });
            }
        }

        let root = self.syntax_root().clone();
        let verify = self.current_verify();
        let (updated, touched) = if at_root {
            let current = crate::high_level::resolve::resolve_body(&root)?;
            let updated = root.green().clone().into_owned().splice_children(
                current.index()..current.index() + 1,
                vec![body.green().clone().into()],
            );
            (updated, HjsonObjectPath::from_segments(Vec::new()))
        } else {
            let touched = crate::high_level::ops::object::receiving_container(&root, &path);
            let updated =
                crate::high_level::ops::object::replace_value_with_node(&root, &path, &body)
                    .map_err(|error| crate::high_level::ops::object::retarget(error, &path))?;
            (updated, touched)
        };
        let document = self.root(updated).note_touched(touched);

        if verify {
            let value = crate::high_level::value::HjsonValue::from_node(&body)?;
            crate::high_level::verify::value_edit(
                &root,
                document.syntax_root(),
                &path,
                |previous| crate::high_level::value_ops::set_path(previous, &path, value),
                crate::high_level::verify::EditedRegion::Node,
            )?;
        }

        Ok(document)
    }

    /// この文書で検出した改行コード。
    pub fn current_newline(&self) -> &str {
        &self.newline
    }

    /// 文書内のコメント（本文のみ）を出現順に返す。
    pub fn all_comments(&self) -> Vec<String> {
        crate::high_level::ops::comment::all_comment_bodies(self.syntax_root())
    }

    /// 一連の値の編集を、文書全体の値とコメントで検証する。
    ///
    /// 編集ごとの検証（`set` / `remove` の既定）は操作のたびに文書全体を再パースするため、
    /// 操作が多いと文書の大きさ × 操作数の費用がかかる。この関数は再パースを 1 回だけ行う。
    ///
    /// `original` は編集前の文書、`edits` は適用した値の操作（適用順）。
    /// 検証に失敗した場合は [`HjsonEditError::InvalidEdit`] を返す。
    pub fn verify_value_edits(
        &self,
        original: &HjsonDocument,
        edits: &[verify::ValueEdit],
    ) -> Result<(), HjsonEditError> {
        verify::value_edits_unchanged(original.syntax_root(), self.syntax_root(), edits)
    }

    /// 整形の前後で、値とコメントが変わっていないことを検証する。
    ///
    /// `previous` は整形前の文書。`format_container` / `format_slot` は
    /// `verify_edits(true)` なら呼び出しごとに検証するが、複数のコンテナを
    /// まとめて整形する場合はこの関数で 1 回だけ検証できる。CLI の `edit --format` も
    /// 整形した後にこの検証を 1 回だけ行う。
    ///
    /// 検証に失敗した場合は [`HjsonEditError::InvalidEdit`] を返す。
    pub fn verify_formatting(&self, previous: &HjsonDocument) -> Result<(), HjsonEditError> {
        verify::formatting_unchanged(previous.syntax_root(), self.syntax_root())
    }

    /// この文書で検出した改行コード（編集の描画に使う）。
    /// 字下げ 1 段分（文書から推定したもの）。
    pub(crate) fn current_indent_unit(&self) -> &str {
        &self.indent_unit
    }

    pub(crate) fn current_newline_kind(&self) -> NewlineKind {
        match self.newline.as_str() {
            "\r\n" => NewlineKind::CrLf,
            _ => NewlineKind::Lf,
        }
    }

    /// lossless な構文木のルート。
    pub fn syntax_root(&self) -> &RedNode {
        &self.root
    }

    /// 意味ASTの文書を組み立てる。
    pub fn ast(&self) -> Result<semantic_ast::Document, HjsonEditError> {
        semantic_ast::Document::cast(self.root.clone())
    }
}

impl std::fmt::Display for HjsonDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root)
    }
}

/// 読み飛ばした入力の警告。CLI はそのまま "Warning: {warning}" として出す。
fn lexer_warning(
    error: &crate::syntax::lexer::LexError,
    line_index: &LineIndex,
    input: &str,
) -> String {
    let (line, column) =
        crate::syntax::char_line_col(line_index, input, error.offset).unwrap_or((0, error.offset));
    format!(
        "skipped input at line {}, column {}: {}",
        line + 1,
        column + 1,
        error.message
    )
}

/// 値の層で後勝ちになる重複キーの警告（出現順）。
fn duplicate_key_warnings(root: &RedNode, input: &str) -> Vec<String> {
    use crate::ast::{AstNode, Key};
    use std::collections::HashSet;

    let line_index = LineIndex::new(input);
    let mut warnings = Vec::new();

    for object in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Object)
    {
        let mut seen: HashSet<String> = HashSet::new();
        for member in object
            .children()
            .filter(|child| child.kind() == SyntaxKind::Member)
        {
            // メンバー全体（レイアウトとトリビア）を組み立てず、キーだけを読む
            let Some(key) = member
                .children()
                .find(|child| child.kind() == SyntaxKind::Key)
                .and_then(|node| Key::cast(node).ok())
                .and_then(|key| key.body_text())
            else {
                continue;
            };
            if !seen.insert(key.clone()) {
                let offset: u32 = member.text_range().start().into();
                let (line, column) =
                    crate::syntax::char_line_col(&line_index, input, offset as usize)
                        .unwrap_or((0, offset as usize));
                warnings.push(format!(
                    "duplicate key {key:?} at line {}, column {} (the last value wins)",
                    line + 1,
                    column + 1
                ));
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_length_at_limit_is_accepted() {
        assert!(check_input_len(MAX_INPUT_BYTES).is_ok());
    }

    #[test]
    fn input_length_over_limit_is_rejected() {
        let result = check_input_len(MAX_INPUT_BYTES + 1);
        assert!(matches!(
            result,
            Err(HjsonEditError::InputTooLong { len, limit })
                if len == MAX_INPUT_BYTES + 1 && limit == MAX_INPUT_BYTES
        ));
    }
}
