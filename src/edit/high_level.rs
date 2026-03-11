//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

pub(crate) mod array_ops;
pub(crate) mod object_ops;
pub(crate) mod navigation;
pub(crate) mod structure;
pub(crate) mod value;

use crate::edit::api::{
    create_node, EditError as LowLevelEditError, GreenNodeEdit, RedNodeEdit,
};
use crate::format::{normalize_targets, FormatTarget};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeExt};
use rowan::Language;
use rowan::{GreenNode, NodeOrToken};

pub(crate) use navigation::{
    find_pair_index_for_comments, find_pair_node_for_comments, parse_path_for_format,
    resolve_array_child, resolve_object_child,
};
pub(crate) use structure::{
    build_array_items, build_object_items, rebuild_array_children_for_format,
    rebuild_object_children_for_format, ArrayEntry, ArrayItem, EntryTerminator, ObjectEntry,
    ObjectItem, StandaloneTriviaBlock,
};
pub(crate) use object_ops::detect_object_entry_indent;
pub(crate) use value::{
    create_multiline_string_literal, create_pair_node, create_value_node, format_target_for_value,
    node_to_value, parse_multiline_string, replace_pair_value, string_style_for_value_node,
    value_needs_contextual_formatting, value_to_green_node,
};

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

#[derive(Debug, Clone)]
pub struct EditResult {
    document: HjsonDocument,
    dirty_targets: Vec<FormatTarget>,
}

impl EditResult {
    fn new(document: HjsonDocument, dirty_targets: Vec<FormatTarget>) -> Self {
        Self {
            document,
            dirty_targets: normalize_targets(&dirty_targets),
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NewlineStyle {
    Lf,
    CrLf,
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
    pub fn parse(input: &str) -> Result<Self, Vec<crate::parser::parser::ParseError>> {
        let result = crate::parser::parser::parse(input)?;
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
        let normalized = normalize_targets(&targets);
        if normalized.is_empty() {
            return Ok(self.clone());
        }
        crate::format::engine::format_document(self, &normalized)
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
            let new_green = value_to_green_node(&value, string_style, None)?;
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
                let obj_node = resolve_object_child(&parent).ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Object,
                        found: parent.kind(),
                    }
                })?;

                let obj_green: GreenNode = obj_node.to_green();
                let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();

                let lbrace = children
                    .iter()
                    .find(|c| is_token_of_kind(c, SyntaxKind::LBrace))
                    .cloned();
                let rbrace = children
                    .iter()
                    .find(|c| is_token_of_kind(c, SyntaxKind::RBrace))
                    .cloned();
                let items = build_object_items(&children);
                let mut found = false;
                let mut new_items = Vec::new();
                let has_braces = lbrace.is_some() && rbrace.is_some();
                let document_text = self.to_string();
                let base_indent = line_indent_for_offset_from_text(
                    &document_text,
                    obj_node.text_range().start().into(),
                );
                let indent_unit = detect_default_indent_unit_from_text(&document_text);
                let entry_indent = object_ops::detect_object_entry_indent(&items).unwrap_or_else(|| {
                    if has_braces {
                        format!("{}{}", base_indent, indent_unit)
                    } else {
                        String::new()
                    }
                });
                let newline = preferred_newline_from_text(&document_text);

                for item in items {
                    match item {
                        ObjectItem::Entry(mut entry) => {
                            if let NodeOrToken::Node(pair_green) = &entry.pair {
                                let pair_red = RedNode::new_root(pair_green.clone());
                                if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                                    if let Some(key_text) = extract_key_text(&key_node) {
                                        if key_text == *key {
                                            entry.pair = NodeOrToken::Node(replace_pair_value(
                                                pair_green,
                                                value,
                                                string_style,
                                            )?);
                                            found = true;
                                        }
                                    }
                                }
                            }
                            new_items.push(ObjectItem::Entry(entry));
                        }
                        ObjectItem::Standalone(block) => {
                            new_items.push(ObjectItem::Standalone(block));
                        }
                    }
                }

                if !found {
                    object_ops::insert_object_entry_without_reformat(
                        &mut new_items,
                        key,
                        value,
                        string_style,
                        has_braces,
                        newline,
                        &entry_indent,
                        &base_indent,
                    )?;
                }

                let new_obj = create_node(
                    SyntaxKind::Object,
                    rebuild_object_children_for_format(lbrace, new_items, rbrace),
                );
                let updated = self.replace_green_node(&obj_node.path_from_root(), new_obj)?;
                let dirty_target = if value_needs_contextual_formatting(value, string_style, None) {
                    format_target_for_value(
                        &updated,
                        path,
                        matches!(value, HjsonValue::Object(_) | HjsonValue::Array(_)),
                    )
                } else {
                    None
                };
                let dirty_targets = dirty_target.into_iter().collect();
                Ok(EditResult::new(updated, dirty_targets))
            }
            PathSegment::Index(idx) => {
                let arr_node = resolve_array_child(&parent).ok_or_else(|| {
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

                let new_arr = create_node(
                    SyntaxKind::Array,
                    rebuild_array_children_for_format(lbracket, new_items, rbracket),
                );
                let updated = self.replace_green_node(&arr_node.path_from_root(), new_arr)?;
                let dirty_target = if value_needs_contextual_formatting(
                    value,
                    string_style,
                    replaced_existing_style,
                ) {
                    format_target_for_value(
                        &updated,
                        path,
                        matches!(value, HjsonValue::Object(_) | HjsonValue::Array(_)),
                    )
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
                        if resolve_object_child(&node).is_none() {
                            return Err(HjsonEditError::InvalidNodeKind {
                                expected: SyntaxKind::Object,
                                found: node.kind(),
                            });
                        }
                    }
                    PathSegment::Index(_) => {
                        if resolve_array_child(&node).is_none() {
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
        let new_arr = insert_array_entry_structural(&arr_node, usize::MAX, &value, string_style)?;
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
        let new_arr = insert_array_entry_structural(&arr_node, index, &value, string_style)?;
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
        for child in obj.children_nodes() {
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
                    .children_nodes()
                    .filter(|n| n.kind() == SyntaxKind::Value)
                    .count(),
            );
        }

        if let Some(value) = node.child_by_kind(SyntaxKind::Value) {
            if let Some(array) = value.child_by_kind(SyntaxKind::Array) {
                return Some(
                    array
                        .children_nodes()
                        .filter(|n| n.kind() == SyntaxKind::Value)
                        .count(),
                );
            }
            if let Some(obj) = value.child_by_kind(SyntaxKind::Object) {
                return Some(
                    obj.children_nodes()
                        .filter(|n| n.kind() == SyntaxKind::Pair)
                        .count(),
                );
            }
        }

        if let Some(obj) = node.child_by_kind(SyntaxKind::Object) {
            return Some(
                obj.children_nodes()
                    .filter(|n| n.kind() == SyntaxKind::Pair)
                    .count(),
            );
        }

        None
    }

    /// パスに対応する値を設定
    pub fn set(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.edit_set(path, value)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// パスに対応する値を設定（文字列スタイル指定付き）
    pub fn set_with_style(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        self.edit_set_with_style(path, value, string_style)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// キー/値を削除
    pub fn remove(&self, path: impl IntoPath) -> Result<Self, HjsonEditError> {
        self.edit_remove(path)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// オブジェクトに新しいキーを追加
    pub fn insert(
        &self,
        path: impl IntoPath,
        key: &str,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.insert_with_style(path, key, value, StringStyle::Auto)
    }

    /// オブジェクトに新しいキーを追加（文字列スタイル指定付き）
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
    pub fn push(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.edit_push(path, value)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// 配列の末尾に要素を追加（文字列スタイル指定付き）
    pub fn push_with_style(
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        self.edit_push_with_style(path, value, string_style)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// 配列の末尾の要素を削除
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

        let value_indices: Vec<usize> = children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                if let NodeOrToken::Node(n) = c {
                    if n.kind() == SyntaxKind::Value.into() {
                        return Some(i);
                    }
                }
                None
            })
            .collect();

        if value_indices.is_empty() {
            return Err(HjsonEditError::InvalidIndex { index: 0, count: 0 });
        }

        let remove_idx = *value_indices.last().unwrap();
        let new_arr = arr_green.remove_child(remove_idx)?;
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    /// 配列の指定位置に要素を挿入
    pub fn insert_at(
        &self,
        path: impl IntoPath,
        index: usize,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        self.edit_insert_at(path, index, value)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// 配列の指定位置に要素を挿入（文字列スタイル指定付き）
    pub fn insert_at_with_style(
        &self,
        path: impl IntoPath,
        index: usize,
        value: impl Into<HjsonValue>,
        string_style: StringStyle,
    ) -> Result<Self, HjsonEditError> {
        self.edit_insert_at_with_style(path, index, value, string_style)?
            .format()
            .map_err(edit_from_format_error)
    }

    /// 配列の指定位置の要素を削除
    pub fn remove_at(&self, path: impl IntoPath, index: usize) -> Result<Self, HjsonEditError> {
        self.edit_remove_at(path, index)?
            .format()
            .map_err(edit_from_format_error)
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
    pub fn comment_set(
        &self,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<Self, HjsonEditError> {
        crate::edit::comments::comment_set(self, path, text, style)
    }

    /// コメントに行を追加
    pub fn comment_append(
        &self,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<Self, HjsonEditError> {
        crate::edit::comments::comment_append(self, path, text, style)
    }

    /// コメントを削除
    pub fn comment_remove(&self, path: &CommentPath) -> Result<Self, HjsonEditError> {
        crate::edit::comments::comment_remove(self, path)
    }

    /// 他のドキュメントとの差分行を検出
    pub fn changed_lines(&self, other: &Self) -> Vec<usize> {
        let before = self.to_string();
        let after = other.to_string();
        crate::diff::simple_line_diff(&before, &after)
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
            resolve_array_child(&node).ok_or_else(|| HjsonEditError::InvalidNodeKind {
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
                let obj_node = resolve_object_child(&parent).ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Object,
                        found: parent.kind(),
                    }
                })?;

                let obj_green: GreenNode = obj_node.to_green();
                let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();
                let lbrace = children
                    .iter()
                    .find(|e| is_token_of_kind(e, SyntaxKind::LBrace))
                    .cloned();
                let rbrace = children
                    .iter()
                    .find(|e| is_token_of_kind(e, SyntaxKind::RBrace))
                    .cloned();
                let items = build_object_items(&children);

                let mut found = false;
                let mut new_items: Vec<ObjectItem> = Vec::new();
                let mut pending_terminator: Option<EntryTerminator> = None;
                let mut has_surviving_entry = false;

                for item in items {
                    match &item {
                        ObjectItem::Entry(entry) => {
                            if let NodeOrToken::Node(pair_node) = &entry.pair {
                                let pair_red = RedNode::new_root(pair_node.clone());
                                if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                                    if let Some(key_text) = extract_key_text(&key_node) {
                                        if key_text == *key {
                                            found = true;
                                            if !has_surviving_entry {
                                                pending_terminator = Some(entry.terminator.clone());
                                            }
                                            continue;
                                        }
                                    }
                                }
                            }

                            let mut modified_entry = entry.clone();
                            if let Some(ref term) = pending_terminator {
                                if let EntryTerminator::Newline(suffix) = term {
                                    let mut new_leading = entry.leading_line_trivia.clone();
                                    if !new_leading
                                        .iter()
                                        .any(|elem| is_token_of_kind(elem, SyntaxKind::Newline))
                                    {
                                        let mut merged = suffix.clone();
                                        merged.extend(new_leading);
                                        new_leading = merged;
                                    }
                                    modified_entry.leading_line_trivia = new_leading;
                                }
                                pending_terminator = None;
                            }

                            new_items.push(ObjectItem::Entry(modified_entry));
                            has_surviving_entry = true;
                        }
                        ObjectItem::Standalone(_) => {
                            new_items.push(item);
                        }
                    }
                }

                if !found {
                    return Err(HjsonEditError::InvalidPath { path: key.clone() });
                }

                let new_obj = create_node(
                    SyntaxKind::Object,
                    rebuild_object_children_for_format(lbrace, new_items, rbrace),
                );
                self.replace_green_node(&obj_node.path_from_root(), new_obj)
            }
            PathSegment::Index(idx) => {
                let arr_node = resolve_array_child(&parent).ok_or_else(|| {
                    HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: parent.kind(),
                    }
                })?;
                let new_arr = remove_array_entry(&arr_node, *idx)?;
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
        let new_arr = remove_array_entry(&arr_node, index)?;
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
        use crate::syntax::green::GreenNodeExt;
        let text = self.green.text();
        let preferred = if text.contains("\r\n") {
            NewlineStyle::CrLf
        } else {
            NewlineStyle::Lf
        };
        f.write_str(&normalize_newlines(&text, &preferred))
    }
}

// =============================================================================
// Comment Helper Functions
// =============================================================================

fn whitespace_after_last_newline(trivia: &[GreenElement]) -> String {
    let mut text = String::new();
    let mut after_newline = false;

    for elem in trivia {
        if let NodeOrToken::Token(t) = elem {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Newline {
                text.clear();
                after_newline = true;
            } else if after_newline && kind == SyntaxKind::Whitespace {
                text.push_str(t.text());
            }
        }
    }

    text
}

fn indentation_from_standalone_block(block: &StandaloneTriviaBlock) -> String {
    let mut after_newline = false;
    let mut current_indent = String::new();
    let mut last_comment_indent = String::new();

    for elem in &block.elements {
        if let NodeOrToken::Token(t) = elem {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            match kind {
                SyntaxKind::Newline => {
                    after_newline = true;
                    current_indent.clear();
                }
                SyntaxKind::Whitespace if after_newline => {
                    current_indent.push_str(t.text());
                }
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                    if after_newline =>
                {
                    last_comment_indent = current_indent.clone();
                    after_newline = false;
                }
                _ => after_newline = false,
            }
        } else {
            after_newline = false;
        }
    }

    if last_comment_indent.is_empty() {
        whitespace_after_last_newline(&block.elements)
    } else {
        last_comment_indent
    }
}

fn remove_orphan_indent_before_entry(children: &mut Vec<GreenElement>) {
    let mut idx = children.len();
    while idx > 0 {
        let elem = &children[idx - 1];
        if is_token_of_kind(elem, SyntaxKind::Whitespace) {
            idx -= 1;
            continue;
        }
        if is_token_of_kind(elem, SyntaxKind::Newline) {
            return;
        }
        break;
    }

    while let Some(last) = children.last() {
        if is_token_of_kind(last, SyntaxKind::Whitespace) {
            children.pop();
        } else {
            break;
        }
    }
}

fn preferred_newline_from_text(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn detect_default_indent_unit_from_text(source: &str) -> String {
    let mut space_indents = Vec::new();
    let mut tab_count = 0usize;

    for line in source.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let indent: String = line
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        if indent.is_empty() {
            continue;
        }
        if indent.contains('\t') {
            tab_count += 1;
        } else {
            space_indents.push(indent.len());
        }
    }

    if tab_count > 0 {
        return "\t".to_string();
    }

    let width = space_indents
        .into_iter()
        .filter(|len| *len > 0)
        .min()
        .unwrap_or(2);
    " ".repeat(width)
}

fn line_indent_for_offset_from_text(source: &str, offset: usize) -> String {
    let start = source[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    source[start..offset]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
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

fn normalize_newlines(text: &str, style: &NewlineStyle) -> String {
    let normalized = text.replace("\r\n", "\n");
    match style {
        NewlineStyle::Lf => normalized,
        NewlineStyle::CrLf => normalized.replace('\n', "\r\n"),
    }
}

// =============================================================================
// Internal Helper Functions
// =============================================================================

fn edit_from_format_error(error: FormatError) -> HjsonEditError {
    match error {
        FormatError::Edit(edit) => edit,
        FormatError::InvalidTarget { target } => HjsonEditError::InvalidPath {
            path: format!("format target {:?}", target),
        },
    }
}

pub(crate) fn extract_key_text(key_node: &RedNode) -> Option<String> {
    for elem in key_node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            match kind {
                SyntaxKind::Ident | SyntaxKind::UnquotedString => {
                    return Some(token.text().to_string());
                }
                SyntaxKind::String => {
                    let text = token.text();
                    if text.len() >= 2 {
                        return Some(text[1..text.len() - 1].to_string());
                    }
                    return Some(text.to_string());
                }
                _ => {}
            }
        }
    }
    None
}

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
/// Object Entry Model
// =============================================================================

fn remove_array_entry(arr_node: &RedNode, index: usize) -> Result<GreenNode, HjsonEditError> {
    array_ops::remove_array_entry(arr_node, index)
}

fn insert_array_entry_structural(
    arr_node: &RedNode,
    index: usize,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    return array_ops::insert_array_entry_structural(arr_node, index, value, string_style);
}


/// 保留中のトリビアに空行が含まれるか判定（\n\s*\n のパターンを検出）

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dot_path() {
        assert_eq!(
            navigation::parse_dot_path("server.host").unwrap(),
            vec![
                PathSegment::Key("server".to_string()),
                PathSegment::Key("host".to_string())
            ]
        );

        assert_eq!(
            navigation::parse_dot_path("items[0].name").unwrap(),
            vec![
                PathSegment::Key("items".to_string()),
                PathSegment::Index(0),
                PathSegment::Key("name".to_string())
            ]
        );

        assert!(matches!(
            navigation::parse_dot_path("items[abc]"),
            Err(HjsonEditError::InvalidPath { .. })
        ));
        assert!(matches!(
            navigation::parse_dot_path("items[1x]"),
            Err(HjsonEditError::InvalidPath { .. })
        ));
    }

    #[test]
    fn test_hjson_value_from() {
        let v: HjsonValue = "hello".into();
        assert_eq!(v, HjsonValue::String("hello".to_string()));

        let v: HjsonValue = 42i64.into();
        assert_eq!(v, HjsonValue::Integer(42));

        let v: HjsonValue = true.into();
        assert_eq!(v, HjsonValue::Bool(true));
    }

    #[test]
    fn test_document_parse_and_get() {
        let input = r#"{"name": "test", "count": 42}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        // Debug: print the structure
        let red = RedNode::new_root(doc.green.clone());
        println!("Root kind: {:?}", red.kind());
        for child in red.children() {
            println!("  Child kind: {:?}", child.kind());
            if child.kind() == SyntaxKind::Value {
                for grandchild in child.children() {
                    println!("    Grandchild kind: {:?}", grandchild.kind());
                    if grandchild.kind() == SyntaxKind::Object {
                        for pair in grandchild.children() {
                            println!("      Pair kind: {:?}", pair.kind());
                            if pair.kind() == SyntaxKind::Pair {
                                if let Some(key_node) = pair.child_by_kind(SyntaxKind::Key) {
                                    println!("        Key node found");
                                    if let Some(key_text) = extract_key_text(&key_node) {
                                        println!("        Key text: {}", key_text);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Test the path navigation
        let path = vec![PathSegment::Key("name".to_string())];
        if let Some(node) = doc.navigate_to_node(&path) {
            println!("Found node: {:?}", node.kind());
        } else {
            println!("Node not found for path: {:?}", path);
        }

        assert_eq!(
            doc.get("name"),
            Some(HjsonValue::String("test".to_string()))
        );
        assert_eq!(doc.get("count"), Some(HjsonValue::Integer(42)));
    }

    #[test]
    fn test_document_get_nested() {
        let input = r#"{"server": {"host": "localhost", "port": 8080}}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        assert_eq!(
            doc.get("server.host"),
            Some(HjsonValue::String("localhost".to_string()))
        );
        assert_eq!(doc.get("server.port"), Some(HjsonValue::Integer(8080)));
    }

    #[test]
    fn test_document_get_ident_and_block_string() {
        let input = "key: look ma, no quotes!\nblock: '''abc'''\n";
        let doc = HjsonDocument::parse(input).unwrap();

        assert_eq!(
            doc.get("key"),
            Some(HjsonValue::String("look ma, no quotes!".to_string()))
        );
        assert_eq!(
            doc.get("block"),
            Some(HjsonValue::String("abc".to_string()))
        );
    }

    #[test]
    fn test_document_get_array() {
        let input = r#"{"items": ["a", "b", "c"]}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        assert_eq!(
            doc.get("items[0]"),
            Some(HjsonValue::String("a".to_string()))
        );
        assert_eq!(
            doc.get("items[1]"),
            Some(HjsonValue::String("b".to_string()))
        );
    }

    #[test]
    fn test_document_set_simple() {
        let input = r#"{"key": "old"}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        let new_doc = doc.set("key", "new").unwrap();
        assert_eq!(
            new_doc.get("key"),
            Some(HjsonValue::String("new".to_string()))
        );
        assert_eq!(doc.get("key"), Some(HjsonValue::String("old".to_string())));
    }

    #[test]
    fn test_document_push() {
        let input = r#"{"items": [1, 2]}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        let new_doc = doc.push("items", 3).unwrap();
        assert!(new_doc.to_string().contains("3"));
    }

    #[test]
    fn test_insert_uses_path_and_escapes_key() {
        let doc = HjsonDocument::parse("root: {}\n").unwrap();
        let new_doc = doc.insert("root", "a\"b", 1).unwrap();
        let rendered = new_doc.to_string();

        assert!(rendered.contains("\"a\\\"b\": 1"));
        assert!(HjsonDocument::parse(&rendered).is_ok());
    }

    #[test]
    fn test_multiline_string_value_is_normalized() {
        let doc = HjsonDocument::parse("text: '''\n  line1\n  line2\n'''\n").unwrap();

        assert_eq!(
            doc.get("text"),
            Some(HjsonValue::String("  line1\n  line2".to_string()))
        );
    }
}
