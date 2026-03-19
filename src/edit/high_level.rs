//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

pub(crate) mod array_ops;
pub(crate) mod navigation;
pub(crate) mod object_ops;
pub(crate) mod structure;
pub(crate) mod value;

use crate::document_style::{DetectNewline, DocumentStyle, NewlineKind};
use crate::edit::api::{EditError as LowLevelEditError, GreenNodeEdit};
use crate::syntax::ext::GreenElement;
use crate::syntax::ext::{RedNode, RedNodeExt};
use crate::syntax::kind::SyntaxKind;
use crate::syntax::lexer::Lexer;
use crate::syntax::parser::Parser;
use crate::ParseError;

use rowan::GreenNode;
use rowan::Language;
use rowan::NodeOrToken;

pub(crate) use navigation::parse_path_for_format;
pub(crate) use navigation::RedNodeNavigateExt;
pub(crate) use object_ops::detect_object_entry_indent;
pub(crate) use structure::{
    build_array_items, build_object_items, indentation_from_standalone_block,
    rebuild_array_children_for_format, rebuild_object_children_for_format,
    whitespace_after_last_newline, ArrayEntry, ArrayItem, EntryTerminator, ObjectEntry, ObjectItem,
    StandaloneTriviaBlock,
};
pub(crate) use value::{
    create_multiline_string_literal, create_pair_node, create_value_node, extract_key_text,
    node_to_value, parse_multiline_string, replace_pair_value, string_style_for_value_node,
    value_needs_contextual_formatting, value_to_green_node,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormatTarget {
    Document,
    Path(String),
    Node(Vec<usize>),
}

impl FormatTarget {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(path.into())
    }

    pub fn node(path_from_root: Vec<usize>) -> Self {
        Self::Node(path_from_root)
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// 高レベル編集エラー
#[derive(Debug, Clone, thiserror::Error)]
pub enum HjsonEditError {
    /// パスが無効
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },
    /// インデックスが無効
    #[error("Invalid index: {index} (child count: {count})")]
    InvalidIndex { index: usize, count: usize },
    /// ノードの種類が期待と異なる
    #[error("Expected {expected:?}, found {found:?}")]
    InvalidNodeKind {
        expected: SyntaxKind,
        found: SyntaxKind,
    },
    /// 空の配列に対してpopを試みた
    #[error("Cannot pop from an empty array")]
    EmptyArray,
    /// 低レベルAPIのエラー
    #[error("{0}")]
    LowLevel(#[from] LowLevelEditError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum FormatError {
    #[error("Invalid format target: {target:?}")]
    InvalidTarget { target: FormatTarget },
    #[error("{0}")]
    Edit(#[from] HjsonEditError),
}

impl From<FormatError> for HjsonEditError {
    fn from(error: FormatError) -> Self {
        match error {
            FormatError::Edit(edit) => edit,
            FormatError::InvalidTarget { target } => Self::InvalidPath {
                path: format!("format target {:?}", target),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditResult {
    document: HjsonDocument,
    dirty_targets: Vec<FormatTarget>,
}

impl EditResult {
    fn new(document: HjsonDocument, dirty_targets: Vec<FormatTarget>) -> Self {
        unimplemented!("formatting is not implemented yet");

        // Self {
        //     document,
        //     dirty_targets: normalize_targets(&dirty_targets),
        // }
    }

    pub fn document(&self) -> &HjsonDocument {
        &self.document
    }

    pub fn dirty_targets(&self) -> &[FormatTarget] {
        &self.dirty_targets
    }

    pub fn into_document(self) -> HjsonDocument {
        self.document
    }

    pub fn format(self) -> Result<HjsonDocument, FormatError> {
        self.document.format(self.dirty_targets)
    }
}

// =============================================================================
// Path System
// =============================================================================

/// パスセグメント
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    /// オブジェクトのキー
    Key(String),
    /// 配列のインデックス
    Index(usize),
}

/// パスへの変換トレイト
pub trait IntoPath {
    fn into_path(self) -> Result<Vec<PathSegment>, HjsonEditError>;
}

impl IntoPath for &str {
    fn into_path(self) -> Result<Vec<PathSegment>, HjsonEditError> {
        navigation::parse_dot_path(self)
    }
}

impl IntoPath for &[&str] {
    fn into_path(self) -> Result<Vec<PathSegment>, HjsonEditError> {
        Ok(self
            .iter()
            .map(|s| PathSegment::Key(s.to_string()))
            .collect())
    }
}

impl IntoPath for Vec<PathSegment> {
    fn into_path(self) -> Result<Vec<PathSegment>, HjsonEditError> {
        Ok(self)
    }
}

impl IntoPath for &[PathSegment] {
    fn into_path(self) -> Result<Vec<PathSegment>, HjsonEditError> {
        Ok(self.to_vec())
    }
}

// =============================================================================
// Value Types
// =============================================================================

/// HJSON値の表現
#[derive(Debug, Clone, PartialEq)]
pub enum HjsonValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
    Array(Vec<HjsonValue>),
    Object(Vec<(String, HjsonValue)>),
}

impl From<&str> for HjsonValue {
    fn from(s: &str) -> Self {
        HjsonValue::String(s.to_string())
    }
}

impl From<String> for HjsonValue {
    fn from(s: String) -> Self {
        HjsonValue::String(s)
    }
}

impl From<i64> for HjsonValue {
    fn from(n: i64) -> Self {
        HjsonValue::Integer(n)
    }
}

impl From<i32> for HjsonValue {
    fn from(n: i32) -> Self {
        HjsonValue::Integer(n as i64)
    }
}

impl From<f64> for HjsonValue {
    fn from(n: f64) -> Self {
        HjsonValue::Float(n)
    }
}

impl From<f32> for HjsonValue {
    fn from(n: f32) -> Self {
        HjsonValue::Float(n as f64)
    }
}

impl From<bool> for HjsonValue {
    fn from(b: bool) -> Self {
        HjsonValue::Bool(b)
    }
}

impl From<Vec<HjsonValue>> for HjsonValue {
    fn from(v: Vec<HjsonValue>) -> Self {
        HjsonValue::Array(v)
    }
}

impl From<Vec<(String, HjsonValue)>> for HjsonValue {
    fn from(v: Vec<(String, HjsonValue)>) -> Self {
        HjsonValue::Object(v)
    }
}

// =============================================================================
// Comment Types
// =============================================================================

/// コメントのスタイル
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommentStyle {
    Line,
    Hash,
    Block,
}

/// 文字列の出力スタイル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringStyle {
    Preserve,
    Auto,
    DoubleQuoted,
    Block,
    Unquoted,
}

// =============================================================================
// Comment Path Types
// =============================================================================

/// コメントの対象パス
#[derive(Debug, Clone, PartialEq)]
pub enum CommentPath {
    /// キーの直前コメント
    Key { dot_path: String },
    /// キーのインラインコメント
    Inline { dot_path: String },
    /// ドキュメント先頭コメント
    Header,
    /// ドキュメント末尾コメント
    Footer,
}

struct DocumentEditContext {
    source: String,
    indent_unit: String,
}

impl DocumentEditContext {
    fn from_document(document: &HjsonDocument) -> Self {
        let source = document.to_string();
        let style = DocumentStyle::new(&source);
        let indent_unit = style.indent_unit();
        Self {
            source,
            indent_unit,
        }
    }

    fn line_indent_for_offset(&self, offset: usize) -> String {
        DocumentStyle::new(&self.source).line_indent_for_offset(offset)
    }
}

impl DetectNewline for HjsonDocument {
    fn try_detect_newline(&self) -> Option<NewlineKind> {
        let root_green = self.green_node();
        let elements: Vec<GreenElement> = root_green.children().map(clone_element).collect();

        // まず構造要素から検出を試みる
        elements
            .try_detect_newline()
            // フォールバック: テキスト全体から検出
            .or_else(|| self.to_string().try_detect_newline())
    }
}

impl DetectNewline for RedNode {
    fn try_detect_newline(&self) -> Option<NewlineKind> {
        let green = self.to_green();
        let elements: Vec<GreenElement> = green.children().map(clone_element).collect();
        elements.try_detect_newline()
    }
}

/// コンテキストに応じた改行コードを検出（親ノードを辿る）
pub(crate) fn contextual_newline_kind(document: &HjsonDocument, node: &RedNode) -> NewlineKind {
    let mut current = Some(node.clone());

    while let Some(candidate) = current {
        if let Some(kind) = candidate.try_detect_newline() {
            return kind;
        }
        current = candidate.parent();
    }

    document.try_detect_newline().unwrap_or_default()
}

struct ArrayValueLocator;

impl ArrayValueLocator {
    fn last_value_index(children: &[GreenElement]) -> Option<usize> {
        children
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, child)| match child {
                NodeOrToken::Node(node)
                    if crate::syntax::kind::HjsonLanguage::kind_from_raw(node.kind())
                        == SyntaxKind::Value =>
                {
                    Some(index)
                }
                _ => None,
            })
    }
}

// =============================================================================
// HjsonDocument
// =============================================================================

/// HJSONドキュメント
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    green: GreenNode,
}

impl HjsonDocument {
    /// HJSON文字列をパースしてドキュメントを作成
    pub fn parse(input: &str) -> Result<Self, Vec<crate::syntax::parser::ParseError>> {
        let lex_result = Lexer::new(input).lex();

        // Lexerエラーがある場合、それをParseErrorに変換して即座に返す
        if !lex_result.errors.is_empty() {
            let parse_errors: Vec<ParseError> = lex_result
                .errors
                .into_iter() // into_iterで所有権ごと移動（高速）
                .map(|e| ParseError::new(e.offset, e.message))
                .collect();

            return Err(parse_errors);
        }

        // LexerのトークンをそのままParserに渡す（ハッピーパスは計算コスト0！）
        let parser = Parser::new(lex_result.tokens);
        let result = parser.parse();

        if !result.errors.is_empty() {
            return Err(result.errors);
        }

        Ok(Self {
            green: result.green,
        })
    }

    /// GreenNodeからドキュメントを作成
    pub fn from_green(green: GreenNode) -> Self {
        Self { green }
    }

    pub fn format_document(&self) -> Result<Self, FormatError> {
        self.format(vec![FormatTarget::Document])
    }

    pub fn format(&self, targets: Vec<FormatTarget>) -> Result<Self, FormatError> {
        unimplemented!("formatting is not implemented yet");
        // let normalized = normalize_targets(&targets);
        // if normalized.is_empty() {
        //     return Ok(self.clone());
        // }
        // format_document(self, &normalized)
    }

    /// GreenNodeへの参照を取得
    pub fn green_node(&self) -> &GreenNode {
        &self.green
    }

    /// GreenNodeを所有権ごと取得
    pub fn into_green_node(self) -> GreenNode {
        self.green
    }

    pub fn edit_set(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
    ) -> Result<EditResult, HjsonEditError> {
        self.edit_set_with_style(path, value, StringStyle::Preserve)
    }

    pub fn edit_set_with_style(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<EditResult, HjsonEditError> {
        let path = path.into_path()?;
        let value = value.into();

        let ensured = self.ensure_object_path_for_set(&path)?;
        ensured.edit_set_existing_path(&path, &value, string_style)
    }

    fn edit_set_existing_path(
        &self,
        path: &[PathSegment],
        value: &HjsonValue,
        string_style: StringStyle,
    ) -> Result<EditResult, HjsonEditError> {
        if path.is_empty() {
            let new_green = value_to_green_node(value, string_style, None)?;
            return Ok(EditResult::new(
                Self { green: new_green },
                vec![FormatTarget::Document],
            ));
        }

        let parent_path = &path[..path.len() - 1];
        let last = path.last().unwrap();

        let parent =
            self.navigate_to_node(parent_path)
                .ok_or_else(|| HjsonEditError::InvalidPath {
                    path: format!("{:?}", parent_path),
                })?;

        match last {
            PathSegment::Key(key) => {
                let obj_node = parent.resolve_object_child().ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Object,
                        found: parent.kind(),
                    }
                })?;

                let context = DocumentEditContext::from_document(self);
                let base_indent =
                    context.line_indent_for_offset(obj_node.text_range().start().into());
                let newline = contextual_newline_kind(self, &obj_node).as_str();

                let new_obj = object_ops::set_object_entry(
                    &obj_node,
                    key,
                    value,
                    string_style,
                    &base_indent,
                    &context.indent_unit,
                    newline,
                )?;
                let updated = self.replace_green_node(&obj_node.path_from_root(), new_obj)?;
                let dirty_target = if value_needs_contextual_formatting(value, string_style, None) {
                    unimplemented!("formatting is not implemented yet");
                    // format_target_for_value(
                    //     &updated,
                    //     path,
                    //     matches!(value, HjsonValue::Object(_) | HjsonValue::Array(_)),
                    // )
                } else {
                    None
                };
                let dirty_targets = dirty_target.into_iter().collect();
                Ok(EditResult::new(updated, dirty_targets))
            }
            PathSegment::Index(idx) => {
                let arr_node = parent.resolve_array_child().ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: parent.kind(),
                    }
                })?;

                let arr_green: GreenNode = arr_node.to_green();
                let children: Vec<GreenElement> = arr_green.children().map(clone_element).collect();

                let lbracket = children
                    .iter()
                    .find(|c| is_token_of_kind(c, SyntaxKind::LBracket))
                    .cloned();
                let rbracket = children
                    .iter()
                    .find(|c| is_token_of_kind(c, SyntaxKind::RBracket))
                    .cloned();
                let items = build_array_items(&children);
                let mut entry_index = 0usize;
                let mut found = false;
                let mut new_items = Vec::new();
                let mut replaced_existing_style = None;

                for item in items {
                    match item {
                        ArrayItem::Entry(mut entry) => {
                            if entry_index == *idx {
                                let existing_style = string_style_for_value_node(&entry.value);
                                replaced_existing_style = existing_style;
                                entry.value = NodeOrToken::Node(create_value_node(
                                    value,
                                    string_style,
                                    existing_style,
                                )?);
                                found = true;
                            }
                            entry_index += 1;
                            new_items.push(ArrayItem::Entry(entry));
                        }
                        ArrayItem::Standalone(block) => {
                            new_items.push(ArrayItem::Standalone(block));
                        }
                    }
                }

                if !found {
                    return Err(HjsonEditError::InvalidIndex {
                        index: *idx,
                        count: entry_index,
                    });
                }

                let new_arr = SyntaxKind::Array.create_node(rebuild_array_children_for_format(
                    lbracket, new_items, rbracket,
                ));
                let updated = self.replace_green_node(&arr_node.path_from_root(), new_arr)?;
                let dirty_target = if value_needs_contextual_formatting(
                    value,
                    string_style,
                    replaced_existing_style,
                ) {
                    unimplemented!("formatting is not implemented yet");
                    // format_target_for_value(
                    //     &updated,
                    //     path,
                    //     matches!(value, HjsonValue::Object(_) | HjsonValue::Array(_)),
                    // )
                } else {
                    None
                };
                let dirty_targets = dirty_target.into_iter().collect();
                Ok(EditResult::new(updated, dirty_targets))
            }
        }
    }

    fn ensure_object_path_for_set(&self, path: &[PathSegment]) -> Result<Self, HjsonEditError> {
        if path.len() <= 1 {
            return Ok(self.clone());
        }

        let mut current = self.clone();
        for depth in 0..path.len() - 1 {
            let current_path = &path[..=depth];
            let next_segment = &path[depth + 1];

            if let Some(node) = current.navigate_to_node(current_path) {
                match next_segment {
                    PathSegment::Key(_) => {
                        if node.resolve_object_child().is_none() {
                            return Err(HjsonEditError::InvalidNodeKind {
                                expected: SyntaxKind::Object,
                                found: node.kind(),
                            });
                        }
                    }
                    PathSegment::Index(_) => {
                        if node.resolve_array_child().is_none() {
                            return Err(HjsonEditError::InvalidNodeKind {
                                expected: SyntaxKind::Array,
                                found: node.kind(),
                            });
                        }
                    }
                }
                continue;
            }

            let parent_path = &path[..depth];
            let key = match &path[depth] {
                PathSegment::Key(key) => key.clone(),
                PathSegment::Index(_) => {
                    return Err(HjsonEditError::InvalidPath {
                        path: format!("{:?}", current_path),
                    })
                }
            };

            if matches!(next_segment, PathSegment::Index(_)) {
                return Err(HjsonEditError::InvalidPath {
                    path: format!("{:?}", current_path),
                });
            }

            current = current
                .edit_set_existing_path(
                    &[parent_path, &[PathSegment::Key(key)]].concat(),
                    &HjsonValue::Object(Vec::new()),
                    StringStyle::Preserve,
                )?
                .into_document();
        }

        Ok(current)
    }

    pub fn edit_remove(&self, path: impl IntoPath) -> Result<EditResult, HjsonEditError> {
        let path_vec = path.into_path()?;
        let updated = self.remove_structural(&path_vec)?;
        Ok(EditResult::new(updated, Vec::new()))
    }

    pub fn edit_push(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
    ) -> Result<EditResult, HjsonEditError> {
        self.edit_push_with_style(path, value, StringStyle::Auto)
    }

    pub fn edit_push_with_style(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<EditResult, HjsonEditError> {
        let path = path.into_path()?;
        let value = value.into();
        let arr_node = self.resolve_array_node(&path)?;
        let new_arr = array_ops::insert_array_entry_structural(
            self,
            &arr_node,
            usize::MAX,
            &value,
            string_style,
        )?;
        let updated = self.replace_green_node(&arr_node.path_from_root(), new_arr)?;
        Ok(EditResult::new(
            updated,
            vec![FormatTarget::node(arr_node.path_from_root())],
        ))
    }

    pub fn edit_insert_at(
        &self,
        path: impl IntoPath,
        index: usize,
        value: impl Into<HjsonValue>,
    ) -> Result<EditResult, HjsonEditError> {
        self.edit_insert_at_with_style(path, index, value, StringStyle::Auto)
    }

    pub fn edit_insert_at_with_style(
        &self,
        path: impl IntoPath,
        index: usize,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<EditResult, HjsonEditError> {
        let path = path.into_path()?;
        let value = value.into();
        let arr_node = self.resolve_array_node(&path)?;
        let new_arr =
            array_ops::insert_array_entry_structural(self, &arr_node, index, &value, string_style)?;
        let updated = self.replace_green_node(&arr_node.path_from_root(), new_arr)?;
        Ok(EditResult::new(
            updated,
            vec![FormatTarget::node(arr_node.path_from_root())],
        ))
    }

    pub fn edit_remove_at(
        &self,
        path: impl IntoPath,
        index: usize,
    ) -> Result<EditResult, HjsonEditError> {
        let path = path.into_path()?;
        let updated = self.remove_at_structural(&path, index)?;
        Ok(EditResult::new(updated, Vec::new()))
    }

    /// パスに対応する値を取得
    pub fn get(&self, path: impl IntoPath) -> Option<HjsonValue> {
        let path = path.into_path().ok()?;
        let node = self.navigate_to_node(&path)?;
        node_to_value(&node)
    }

    /// パスが存在するか確認
    pub fn contains(&self, path: impl IntoPath) -> bool {
        path.into_path()
            .ok()
            .and_then(|path| self.navigate_to_node(&path))
            .is_some()
    }

    /// 指定パスのオブジェクトのキー一覧を取得
    pub fn keys(&self, path: impl IntoPath) -> Option<Vec<String>> {
        let path = path.into_path().ok()?;
        let node = self.navigate_to_node(&path)?;

        let obj = if let Some(obj) = node.child_by_kind(SyntaxKind::Object) {
            obj
        } else if let Some(value) = node.child_by_kind(SyntaxKind::Value) {
            value.child_by_kind(SyntaxKind::Object)?
        } else {
            return None;
        };

        let mut keys = Vec::new();
        for child in obj.children() {
            if child.kind() == SyntaxKind::Pair {
                if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                    if let Some(key_text) = extract_key_text(&key_node) {
                        keys.push(key_text);
                    }
                }
            }
        }
        Some(keys)
    }

    /// 配列またはオブジェクトの要素数を取得
    pub fn len(&self, path: impl IntoPath) -> Option<usize> {
        let path = path.into_path().ok()?;
        let node = self.navigate_to_node(&path)?;

        if let Some(array) = node.child_by_kind(SyntaxKind::Array) {
            return Some(
                array
                    .children()
                    .filter(|n| n.kind() == SyntaxKind::Value)
                    .count(),
            );
        }

        if let Some(value) = node.child_by_kind(SyntaxKind::Value) {
            if let Some(array) = value.child_by_kind(SyntaxKind::Array) {
                return Some(
                    array
                        .children()
                        .filter(|n| n.kind() == SyntaxKind::Value)
                        .count(),
                );
            }
            if let Some(obj) = value.child_by_kind(SyntaxKind::Object) {
                return Some(
                    obj.children()
                        .filter(|n| n.kind() == SyntaxKind::Pair)
                        .count(),
                );
            }
        }

        if let Some(obj) = node.child_by_kind(SyntaxKind::Object) {
            return Some(
                obj.children()
                    .filter(|n| n.kind() == SyntaxKind::Pair)
                    .count(),
            );
        }

        None
    }

    /// パスに対応する値を設定
    #[must_use]
    pub fn set(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.edit_set(path, value)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// パスに対応する値を設定（文字列スタイル指定付き）
    #[must_use]
    pub fn set_with_style(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        self.edit_set_with_style(path, value, string_style)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// キー/値を削除
    #[must_use]
    pub fn remove(&self, path: impl IntoPath) -> Result<Self, HjsonEditError> {
        self.edit_remove(path)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// オブジェクトに新しいキーを追加
    #[must_use]
    pub fn insert(
        &self,
        path: impl IntoPath,
        key: &str,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.insert_with_style(path, key, value, StringStyle::Auto)
    }

    /// オブジェクトに新しいキーを追加（文字列スタイル指定付き）
    #[must_use]
    pub fn insert_with_style(
        &self,
        path: impl IntoPath,
        key: &str,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        let mut path = path.into_path()?;
        path.push(PathSegment::Key(key.to_string()));
        self.set_with_style(path, value, string_style)
    }

    /// 配列の末尾に要素を追加
    #[must_use]
    pub fn push(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.edit_push(path, value)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// 配列の末尾に要素を追加（文字列スタイル指定付き）
    #[must_use]
    pub fn push_with_style(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        self.edit_push_with_style(path, value, string_style)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// 配列の末尾の要素を削除
    #[must_use]
    pub fn pop(&self, path: impl IntoPath) -> Result<Self, HjsonEditError> {
        let path = path.into_path()?;

        let arr_node = if path.is_empty() {
            let red = RedNode::new_root(self.green.clone());
            if let Some(val) = red.child_by_kind(SyntaxKind::Value) {
                val.child_by_kind(SyntaxKind::Array).ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: val.kind(),
                    }
                })?
            } else {
                return Err(HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Array,
                    found: red.kind(),
                });
            }
        } else {
            let node = self
                .navigate_to_node(&path)
                .ok_or_else(|| HjsonEditError::InvalidPath {
                    path: format!("{:?}", path),
                })?;

            if let Some(arr) = node.child_by_kind(SyntaxKind::Array) {
                arr
            } else if let Some(val) = node.child_by_kind(SyntaxKind::Value) {
                val.child_by_kind(SyntaxKind::Array).ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: val.kind(),
                    }
                })?
            } else {
                return Err(HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Array,
                    found: node.kind(),
                });
            }
        };

        let arr_green: GreenNode = arr_node.to_green();
        let children: Vec<GreenElement> = arr_green.children().map(clone_element).collect();

        let Some(remove_idx) = ArrayValueLocator::last_value_index(&children) else {
            return Err(HjsonEditError::EmptyArray);
        };

        let new_arr = arr_green.remove_child(remove_idx)?;
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    /// 配列の指定位置に要素を挿入
    #[must_use]
    pub fn insert_at(
        &self,
        path: impl IntoPath,
        index: usize,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.edit_insert_at(path, index, value)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// 配列の指定位置に要素を挿入（文字列スタイル指定付き）
    #[must_use]
    pub fn insert_at_with_style(
        &self,
        path: impl IntoPath,
        index: usize,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        self.edit_insert_at_with_style(path, index, value, string_style)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// 配列の指定位置の要素を削除
    #[must_use]
    pub fn remove_at(&self, path: impl IntoPath, index: usize) -> Result<Self, HjsonEditError> {
        self.edit_remove_at(path, index)?
            .format()
            .map_err(HjsonEditError::from)
    }

    /// ドキュメント先頭のコメントを取得
    pub fn get_header_comment(&self) -> Vec<String> {
        crate::edit::comments::get_header_comment(self)
    }

    /// ドキュメント末尾のコメントを取得
    pub fn get_footer_comment(&self) -> Vec<String> {
        crate::edit::comments::get_footer_comment(self)
    }

    /// コメントを設定
    #[must_use]
    pub fn comment_set(
        &self,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<Self, HjsonEditError> {
        crate::edit::comments::comment_set(self, path, text, style)
    }

    /// コメントに行を追加
    #[must_use]
    pub fn comment_append(
        &self,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<Self, HjsonEditError> {
        crate::edit::comments::comment_append(self, path, text, style)
    }

    /// コメントを削除
    #[must_use]
    pub fn comment_remove(&self, path: &CommentPath) -> Result<Self, HjsonEditError> {
        crate::edit::comments::comment_remove(self, path)
    }

    fn resolve_array_node(&self, path: &[PathSegment]) -> Result<RedNode, HjsonEditError> {
        if path.is_empty() {
            let red = RedNode::new_root(self.green.clone());
            if let Some(val) = red.child_by_kind(SyntaxKind::Value) {
                val.child_by_kind(SyntaxKind::Array).ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: val.kind(),
                    }
                })
            } else {
                Err(HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Array,
                    found: red.kind(),
                })
            }
        } else {
            let node = self
                .navigate_to_node(path)
                .ok_or_else(|| HjsonEditError::InvalidPath {
                    path: format!("{:?}", path),
                })?;
            node.resolve_array_child()
                .ok_or_else(|| HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Array,
                    found: node.kind(),
                })
        }
    }

    fn remove_structural(&self, path: &[PathSegment]) -> Result<Self, HjsonEditError> {
        if path.is_empty() {
            return Err(HjsonEditError::InvalidPath {
                path: "empty".to_string(),
            });
        }

        let parent_path = &path[..path.len() - 1];
        let last = path.last().unwrap();

        let parent =
            self.navigate_to_node(parent_path)
                .ok_or_else(|| HjsonEditError::InvalidPath {
                    path: format!("{:?}", parent_path),
                })?;

        match last {
            PathSegment::Key(key) => {
                let obj_node = parent.resolve_object_child().ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Object,
                        found: parent.kind(),
                    }
                })?;

                let new_obj = object_ops::remove_object_entry(&obj_node, key)?;
                self.replace_green_node(&obj_node.path_from_root(), new_obj)
            }
            PathSegment::Index(idx) => {
                let arr_node = parent.resolve_array_child().ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: parent.kind(),
                    }
                })?;
                let new_arr = array_ops::remove_array_entry(&arr_node, *idx)?;
                self.replace_green_node(&arr_node.path_from_root(), new_arr)
            }
        }
    }

    fn remove_at_structural(
        &self,
        path: &[PathSegment],
        index: usize,
    ) -> Result<Self, HjsonEditError> {
        let arr_node = self.resolve_array_node(path)?;
        let new_arr = array_ops::remove_array_entry(&arr_node, index)?;
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    pub(crate) fn navigate_to_node_internal(&self, path: &[PathSegment]) -> Option<RedNode> {
        self.navigate_to_node(path)
    }

    pub(crate) fn replace_green_node_internal(
        &self,
        path_from_root: &[usize],
        new_node: GreenNode,
    ) -> Result<Self, HjsonEditError> {
        self.replace_green_node(path_from_root, new_node)
    }

    // -------------------------------------------------------------------------
    // Internal Helpers
    // -------------------------------------------------------------------------

    fn navigate_to_node(&self, path: &[PathSegment]) -> Option<RedNode> {
        let red = RedNode::new_root(self.green.clone());
        navigation::navigate_from_node(&red, path)
    }

    /// GreenNodeのインデックスパスに基づいてノードを置換
    fn replace_green_node(
        &self,
        path_from_root: &[usize],
        new_node: GreenNode,
    ) -> Result<Self, HjsonEditError> {
        if path_from_root.is_empty() {
            return Ok(Self { green: new_node });
        }

        let mut current = new_node;
        // ボトムアップで再構築：最も深い階層から順に親を再構築
        for (i, &idx) in path_from_root.iter().enumerate().rev() {
            // この階層の親ノードを取得（元のツリーからパスをたどる）
            let mut parent_node = self.green.clone();
            for &ancestor_idx in &path_from_root[..i] {
                if let Some(NodeOrToken::Node(n)) = parent_node.children().nth(ancestor_idx) {
                    parent_node = n.to_owned();
                } else {
                    return Err(HjsonEditError::InvalidPath {
                        path: format!("{:?}", path_from_root),
                    });
                }
            }
            current = parent_node.replace_child(idx, current.into())?;
        }

        Ok(Self { green: current })
    }
}

impl std::fmt::Display for HjsonDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::syntax::ext::GreenNodeExt;
        let text = self.green.text();
        f.write_str(&text)
    }
}

pub(crate) fn detect_array_entry_indent(items: &[ArrayItem]) -> Option<String> {
    items.iter().find_map(|item| match item {
        ArrayItem::Entry(entry) => {
            let indent = whitespace_after_last_newline(&entry.leading_line_trivia);
            if indent.is_empty() {
                None
            } else {
                Some(indent)
            }
        }
        ArrayItem::Standalone(block) => {
            let indent = indentation_from_standalone_block(block);
            if indent.is_empty() {
                None
            } else {
                Some(indent)
            }
        }
    })
}

// =============================================================================
// Internal Helper Functions
// =============================================================================

pub(crate) fn clone_element(
    elem: rowan::NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
) -> GreenElement {
    match elem {
        NodeOrToken::Node(n) => {
            let node: GreenNode = std::borrow::Cow::Borrowed(n).into();
            NodeOrToken::Node(node)
        }
        NodeOrToken::Token(t) => {
            // GreenTokenを新しく作成
            let text = t.text().to_string();
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            let token = rowan::GreenToken::new(kind.into(), &text);
            NodeOrToken::Token(token)
        }
    }
}

pub(crate) fn is_token_of_kind(elem: &GreenElement, kind: SyntaxKind) -> bool {
    if let NodeOrToken::Token(t) = elem {
        crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()) == kind
    } else {
        false
    }
}

// =============================================================================
// Tests
// =============================================================================
