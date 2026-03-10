//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

use crate::edit::api::{
    create_node, create_token, EditError as LowLevelEditError, GreenNodeEdit, RedNodeEdit,
};
use crate::format::{engine, normalize_targets, FormatTarget};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeExt};
use rowan::Language;
use rowan::{GreenNode, NodeOrToken};

// =============================================================================
// Error Types
// =============================================================================

/// 高レベル編集エラー
#[derive(Debug, Clone)]
pub enum HjsonEditError {
    /// パスが無効
    InvalidPath { path: String },
    /// インデックスが無効
    InvalidIndex { index: usize, count: usize },
    /// ノードの種類が期待と異なる
    InvalidNodeKind {
        expected: SyntaxKind,
        found: SyntaxKind,
    },
    /// 低レベルAPIのエラー
    LowLevel(LowLevelEditError),
}

impl From<LowLevelEditError> for HjsonEditError {
    fn from(e: LowLevelEditError) -> Self {
        HjsonEditError::LowLevel(e)
    }
}

impl std::fmt::Display for HjsonEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HjsonEditError::InvalidPath { path } => write!(f, "Invalid path: {}", path),
            HjsonEditError::InvalidIndex { index, count } => {
                write!(f, "Invalid index: {} (child count: {})", index, count)
            }
            HjsonEditError::InvalidNodeKind { expected, found } => {
                write!(f, "Expected {:?}, found {:?}", expected, found)
            }
            HjsonEditError::LowLevel(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for HjsonEditError {}

#[derive(Debug, Clone)]
pub enum FormatError {
    InvalidTarget { target: FormatTarget },
    Edit(HjsonEditError),
}

impl From<HjsonEditError> for FormatError {
    fn from(value: HjsonEditError) -> Self {
        Self::Edit(value)
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::InvalidTarget { target } => {
                write!(f, "Invalid format target: {:?}", target)
            }
            FormatError::Edit(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for FormatError {}

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
        parse_dot_path(self)
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

/// ドット記法のパスをパースする
fn parse_dot_path(s: &str) -> Result<Vec<PathSegment>, HjsonEditError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        if chars[pos] == '.' {
            return Err(HjsonEditError::InvalidPath {
                path: s.to_string(),
            });
        }

        if chars[pos] == '[' {
            let (index, next_pos) = parse_index_segment(&chars, pos, s)?;
            segments.push(PathSegment::Index(index));
            pos = next_pos;
        } else {
            let start = pos;
            while pos < chars.len() && chars[pos] != '.' && chars[pos] != '[' {
                if chars[pos] == ']' {
                    return Err(HjsonEditError::InvalidPath {
                        path: s.to_string(),
                    });
                }
                pos += 1;
            }

            if start == pos {
                return Err(HjsonEditError::InvalidPath {
                    path: s.to_string(),
                });
            }

            segments.push(PathSegment::Key(chars[start..pos].iter().collect()));

            while pos < chars.len() && chars[pos] == '[' {
                let (index, next_pos) = parse_index_segment(&chars, pos, s)?;
                segments.push(PathSegment::Index(index));
                pos = next_pos;
            }
        }

        if pos == chars.len() {
            break;
        }

        if chars[pos] != '.' {
            return Err(HjsonEditError::InvalidPath {
                path: s.to_string(),
            });
        }

        pos += 1;
        if pos == chars.len() {
            return Err(HjsonEditError::InvalidPath {
                path: s.to_string(),
            });
        }
    }

    Ok(segments)
}

fn parse_index_segment(
    chars: &[char],
    start: usize,
    raw: &str,
) -> Result<(usize, usize), HjsonEditError> {
    let mut pos = start + 1;
    let digit_start = pos;

    while pos < chars.len() && chars[pos].is_ascii_digit() {
        pos += 1;
    }

    if digit_start == pos || pos >= chars.len() || chars[pos] != ']' {
        return Err(HjsonEditError::InvalidPath {
            path: raw.to_string(),
        });
    }

    let index = chars[digit_start..pos]
        .iter()
        .collect::<String>()
        .parse::<usize>()
        .map_err(|_| HjsonEditError::InvalidPath {
            path: raw.to_string(),
        })?;

    Ok((index, pos + 1))
}

pub(crate) fn parse_path_for_format(s: &str) -> Result<Vec<PathSegment>, HjsonEditError> {
    parse_dot_path(s)
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
        engine::format_document(self, &normalized)
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
                                                &value,
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
                    let mut trailing_standalone = Vec::new();
                    while matches!(new_items.last(), Some(ObjectItem::Standalone(_))) {
                        if let Some(item) = new_items.pop() {
                            trailing_standalone.push(item);
                        }
                    }
                    trailing_standalone.reverse();

                    new_items.push(ObjectItem::Entry(ObjectEntry {
                        leading_line_trivia: Vec::new(),
                        leading_comments: Vec::new(),
                        pair: NodeOrToken::Node(create_pair_node(
                            &key,
                            &value,
                            string_style,
                            None,
                        )?),
                        trailing_inline_trivia: Vec::new(),
                        terminator: EntryTerminator::EndOfContainer,
                    }));
                    new_items.extend(trailing_standalone);
                }

                let new_obj = create_node(
                    SyntaxKind::Object,
                    rebuild_object_children_for_format(lbrace, new_items, rbrace),
                );
                let updated = self.replace_green_node(&obj_node.path_from_root(), new_obj)?;
                let dirty_targets = if found {
                    Vec::new()
                } else {
                    vec![FormatTarget::node(obj_node.path_from_root())]
                };
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

                for item in items {
                    match item {
                        ArrayItem::Entry(mut entry) => {
                            if entry_index == *idx {
                                let existing_style = string_style_for_value_node(&entry.value);
                                entry.value = NodeOrToken::Node(create_value_node(
                                    &value,
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
                Ok(EditResult::new(updated, Vec::new()))
            }
        }
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
        navigate_from_node(&red, path)
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

/// 指定キーの Pair のインデックスを探す
pub(crate) fn find_pair_index_for_comments(obj: &RedNode, key: &str) -> Option<usize> {
    for (i, child) in obj.children_nodes().enumerate() {
        if child.kind() == SyntaxKind::Pair {
            if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                if let Some(key_text) = extract_key_text(&key_node) {
                    if key_text == key {
                        return Some(i);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn find_pair_node_for_comments(obj: &RedNode, key: &str) -> Option<RedNode> {
    for child in obj.children_nodes() {
        if child.kind() == SyntaxKind::Pair {
            if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                if let Some(key_text) = extract_key_text(&key_node) {
                    if key_text == key {
                        return Some(child);
                    }
                }
            }
        }
    }
    None
}

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

pub(crate) fn detect_object_entry_indent(items: &[ObjectItem]) -> Option<String> {
    items.iter().find_map(|item| match item {
        ObjectItem::Entry(entry) => {
            let indent = whitespace_after_last_newline(&entry.leading_line_trivia);
            if indent.is_empty() {
                None
            } else {
                Some(indent)
            }
        }
        ObjectItem::Standalone(block) => {
            let indent = indentation_from_standalone_block(block);
            if indent.is_empty() {
                None
            } else {
                Some(indent)
            }
        }
    })
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

fn navigate_from_node(start: &RedNode, path: &[PathSegment]) -> Option<RedNode> {
    let mut current: RedNode = start.clone();

    for segment in path {
        match segment {
            PathSegment::Key(key) => {
                let obj: RedNode = if let Some(obj) = current.child_by_kind(SyntaxKind::Object) {
                    obj
                } else if let Some(val) = current.child_by_kind(SyntaxKind::Value) {
                    val.child_by_kind(SyntaxKind::Object)?
                } else {
                    return None;
                };

                let mut found = None;
                for child in obj.children_nodes() {
                    if child.kind() == SyntaxKind::Pair {
                        if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                            if let Some(key_text) = extract_key_text(&key_node) {
                                if key_text == *key {
                                    found = Some(child);
                                    break;
                                }
                            }
                        }
                    }
                }
                current = found?;
                current = current.child_by_kind(SyntaxKind::Value)?;
            }
            PathSegment::Index(idx) => {
                let arr: RedNode = if let Some(arr) = current.child_by_kind(SyntaxKind::Array) {
                    arr
                } else if let Some(val) = current.child_by_kind(SyntaxKind::Value) {
                    val.child_by_kind(SyntaxKind::Array)?
                } else {
                    return None;
                };

                let values: Vec<RedNode> = arr
                    .children_nodes()
                    .filter(|n| n.kind() == SyntaxKind::Value)
                    .collect();

                current = values.get(*idx)?.clone();
            }
        }
    }

    Some(current)
}

pub(crate) fn resolve_object_child(node: &RedNode) -> Option<RedNode> {
    node.child_by_kind(SyntaxKind::Object).or_else(|| {
        node.child_by_kind(SyntaxKind::Value)?
            .child_by_kind(SyntaxKind::Object)
    })
}

pub(crate) fn resolve_array_child(node: &RedNode) -> Option<RedNode> {
    node.child_by_kind(SyntaxKind::Array).or_else(|| {
        node.child_by_kind(SyntaxKind::Value)?
            .child_by_kind(SyntaxKind::Array)
    })
}

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

fn node_to_value(node: &RedNode) -> Option<HjsonValue> {
    let target: RedNode = if node.kind() == SyntaxKind::Value {
        // Valueノードの場合、子はトークンまたはノード
        // children_nodes()ではなくchildren_with_tokens()を使う
        if let Some(child) = node.children_nodes().next() {
            child
        } else {
            // 子ノードがない場合はトークンを探す
            return token_to_value(node);
        }
    } else {
        node.clone()
    };

    match target.kind() {
        SyntaxKind::Object => {
            let mut pairs = Vec::new();
            for child in target.children_nodes() {
                if child.kind() == SyntaxKind::Pair {
                    if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                        if let Some(key_text) = extract_key_text(&key_node) {
                            if let Some(val_node) = child.child_by_kind(SyntaxKind::Value) {
                                if let Some(val) = node_to_value(&val_node) {
                                    pairs.push((key_text, val));
                                }
                            }
                        }
                    }
                }
            }
            Some(HjsonValue::Object(pairs))
        }
        SyntaxKind::Array => {
            let mut items = Vec::new();
            for child in target.children_nodes() {
                if child.kind() == SyntaxKind::Value {
                    if let Some(val) = node_to_value(&child) {
                        items.push(val);
                    }
                }
            }
            Some(HjsonValue::Array(items))
        }
        _ => token_to_value(&target),
    }
}

fn token_to_value(node: &RedNode) -> Option<HjsonValue> {
    for elem in node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            let text = token.text();

            // Skip trivia (whitespace, comments)
            if matches!(
                kind,
                SyntaxKind::Whitespace
                    | SyntaxKind::Newline
                    | SyntaxKind::LineComment
                    | SyntaxKind::HashComment
                    | SyntaxKind::BlockComment
            ) {
                continue;
            }

            return match kind {
                SyntaxKind::String => Some(HjsonValue::String(strip_surrounding(text, 1))),
                SyntaxKind::BlockString => Some(HjsonValue::String(parse_multiline_string(text))),
                SyntaxKind::UnquotedString | SyntaxKind::Ident => {
                    Some(HjsonValue::String(text.to_string()))
                }
                SyntaxKind::Integer => text.parse::<i64>().ok().map(HjsonValue::Integer),
                SyntaxKind::Float => text.parse::<f64>().ok().map(HjsonValue::Float),
                SyntaxKind::True => Some(HjsonValue::Bool(true)),
                SyntaxKind::False => Some(HjsonValue::Bool(false)),
                SyntaxKind::Null => Some(HjsonValue::Null),
                _ => None,
            };
        }
    }
    None
}

fn strip_surrounding(text: &str, delimiter_len: usize) -> String {
    let min_len = delimiter_len * 2;
    if text.len() >= min_len {
        text[delimiter_len..text.len() - delimiter_len].to_string()
    } else {
        text.to_string()
    }
}

fn parse_multiline_string(text: &str) -> String {
    let inner = strip_surrounding(text, 3).replace('\r', "");
    let inner = inner.strip_prefix('\n').unwrap_or(&inner);
    let mut lines: Vec<&str> = inner.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .count()
        })
        .min()
        .unwrap_or(0);

    lines
        .into_iter()
        .map(|line| {
            if line.len() >= indent {
                line.chars().skip(indent).collect::<String>()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_multiline_string(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let escaped = normalized.replace("'''", "''\\'");
    format!("'''\n{}\n'''", escaped)
}

fn is_hjson_number(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let bytes = value.as_bytes();
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

    if idx < bytes.len() && matches!(bytes[idx], b'e' | b'E') {
        idx += 1;
        if idx < bytes.len() && matches!(bytes[idx], b'+' | b'-') {
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

fn string_style_for_red_value_node(node: &RedNode) -> Option<StringStyle> {
    for elem in node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            if matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline) {
                continue;
            }
            return match kind {
                SyntaxKind::String => Some(StringStyle::DoubleQuoted),
                SyntaxKind::BlockString => Some(StringStyle::Block),
                SyntaxKind::UnquotedString => Some(StringStyle::Unquoted),
                _ => None,
            };
        }
    }
    None
}

fn string_style_for_value_node(node: &GreenElement) -> Option<StringStyle> {
    match node {
        NodeOrToken::Node(green) => {
            let red = RedNode::new_root(green.clone());
            string_style_for_red_value_node(&red)
        }
        NodeOrToken::Token(token) => {
            match crate::syntax::kind::HjsonLanguage::kind_from_raw(token.kind()) {
                SyntaxKind::String => Some(StringStyle::DoubleQuoted),
                SyntaxKind::BlockString => Some(StringStyle::Block),
                SyntaxKind::UnquotedString => Some(StringStyle::Unquoted),
                _ => None,
            }
        }
    }
}

fn resolve_string_style(
    value: &str,
    requested_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> StringStyle {
    match requested_style {
        StringStyle::Preserve => existing_style.unwrap_or_else(|| auto_string_style(value)),
        StringStyle::Auto => auto_string_style(value),
        explicit => explicit,
    }
}

fn auto_string_style(value: &str) -> StringStyle {
    if value.contains('\n') || value.contains('\r') {
        return StringStyle::Block;
    }

    if value.starts_with('[') || value.starts_with('{') {
        return StringStyle::DoubleQuoted;
    }

    if is_safe_unquoted_string(value) {
        StringStyle::Unquoted
    } else {
        StringStyle::DoubleQuoted
    }
}

fn is_safe_unquoted_string(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    if matches!(value, "true" | "false" | "null") {
        return false;
    }

    if is_hjson_number(value) {
        return false;
    }

    if value.starts_with("//") || value.starts_with("/*") || value.starts_with('#') {
        return false;
    }

    if matches!(
        value.chars().next(),
        Some('{') | Some('}') | Some('[') | Some(']') | Some(',') | Some(':')
    ) {
        return false;
    }

    !value
        .chars()
        .any(|ch| matches!(ch, '\n' | '\r' | '\u{0000}'..='\u{001F}'))
}

fn create_string_token(value: &str, style: StringStyle) -> rowan::GreenToken {
    match style {
        StringStyle::DoubleQuoted | StringStyle::Preserve | StringStyle::Auto => create_token(
            SyntaxKind::String,
            format!("\"{}\"", escape_double_quoted(value)),
        ),
        StringStyle::Block => create_token(SyntaxKind::BlockString, render_multiline_string(value)),
        StringStyle::Unquoted => create_token(SyntaxKind::UnquotedString, value),
    }
}

fn escape_double_quoted(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn value_to_green_element(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenElement, HjsonEditError> {
    match value {
        HjsonValue::String(s) => {
            let token =
                create_string_token(s, resolve_string_style(s, string_style, existing_style));
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Integer(n) => {
            let token = create_token(SyntaxKind::Integer, n.to_string());
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Float(n) => {
            let token = create_token(SyntaxKind::Float, format!("{}", n));
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Bool(true) => {
            let token = create_token(SyntaxKind::True, "true");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Bool(false) => {
            let token = create_token(SyntaxKind::False, "false");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Null => {
            let token = create_token(SyntaxKind::Null, "null");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Array(items) => {
            let mut children = vec![NodeOrToken::Token(create_token(SyntaxKind::LBracket, "["))];

            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")));
                }
                let val_node = create_value_node(item, string_style, None)?;
                children.push(NodeOrToken::Node(val_node));
            }

            children.push(NodeOrToken::Token(create_token(SyntaxKind::RBracket, "]")));

            let node = create_node(SyntaxKind::Array, children);
            Ok(NodeOrToken::Node(node))
        }
        HjsonValue::Object(pairs) => {
            let mut children = vec![NodeOrToken::Token(create_token(SyntaxKind::LBrace, "{"))];

            for (i, (key, val)) in pairs.iter().enumerate() {
                if i > 0 {
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")));
                }
                let pair_node = create_pair_node(key, val, string_style, None)?;
                children.push(NodeOrToken::Node(pair_node));
            }

            children.push(NodeOrToken::Token(create_token(SyntaxKind::RBrace, "}")));

            let node = create_node(SyntaxKind::Object, children);
            Ok(NodeOrToken::Node(node))
        }
    }
}

fn value_to_green_node(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    match value_to_green_element(value, string_style, existing_style)? {
        NodeOrToken::Node(n) => Ok(n),
        NodeOrToken::Token(t) => {
            // スカラー値の場合はValueノードでラップ
            Ok(create_node(SyntaxKind::Value, vec![NodeOrToken::Token(t)]))
        }
    }
}

fn create_value_node(
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    let inner = value_to_green_element(value, string_style, existing_style)?;
    let children = vec![inner];
    Ok(create_node(SyntaxKind::Value, children))
}

fn replace_value_node(
    _existing_value_node: &GreenNode,
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    create_value_node(value, string_style, existing_style)
}

fn create_pair_node(
    key: &str,
    value: &HjsonValue,
    string_style: StringStyle,
    existing_style: Option<StringStyle>,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();

    let key_token = if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        create_token(SyntaxKind::Ident, key)
    } else {
        create_token(
            SyntaxKind::String,
            format!("\"{}\"", escape_double_quoted(key)),
        )
    };
    let key_node = create_node(SyntaxKind::Key, vec![NodeOrToken::Token(key_token)]);
    children.push(NodeOrToken::Node(key_node));

    children.push(NodeOrToken::Token(create_token(SyntaxKind::Colon, ":")));
    children.push(NodeOrToken::Token(create_token(
        SyntaxKind::Whitespace,
        " ",
    )));

    let val_node = create_value_node(value, string_style, existing_style)?;
    children.push(NodeOrToken::Node(val_node));

    Ok(create_node(SyntaxKind::Pair, children))
}

fn replace_pair_value(
    pair_green: &GreenNode,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let pair_red = RedNode::new_root(pair_green.clone());
    let existing_value_red = pair_red.child_by_kind(SyntaxKind::Value);
    let existing_style = existing_value_red
        .as_ref()
        .and_then(|value_node| string_style_for_red_value_node(value_node));
    let mut replaced = false;
    let mut children = Vec::new();

    for child in pair_green.children() {
        let cloned = clone_element(child);
        match &cloned {
            NodeOrToken::Node(n)
                if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind())
                    == SyntaxKind::Value =>
            {
                children.push(NodeOrToken::Node(replace_value_node(
                    n,
                    value,
                    string_style,
                    existing_style,
                )?));
                replaced = true;
            }
            NodeOrToken::Token(t)
                if crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind())
                    == SyntaxKind::Colon =>
            {
                children.push(cloned.clone());
                let has_space = pair_green.children().any(|child| {
                    if let NodeOrToken::Token(token) = child {
                        crate::syntax::kind::HjsonLanguage::kind_from_raw(token.kind())
                            == SyntaxKind::Whitespace
                    } else {
                        false
                    }
                });
                if !has_space {
                    children.push(NodeOrToken::Token(create_token(
                        SyntaxKind::Whitespace,
                        " ",
                    )));
                }
            }
            _ => children.push(cloned),
        }
    }

    if !replaced {
        return create_pair_node(
            &extract_key_text(
                &RedNode::new_root(pair_green.clone())
                    .child_by_kind(SyntaxKind::Key)
                    .ok_or(HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Key,
                        found: SyntaxKind::Pair,
                    })?,
            )
            .ok_or_else(|| HjsonEditError::InvalidPath {
                path: "pair key".to_string(),
            })?,
            value,
            string_style,
            existing_style,
        );
    }

    Ok(create_node(SyntaxKind::Pair, children))
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
// Object Entry Model
// =============================================================================

/// Object 要素の終端種別
#[derive(Debug, Clone)]
pub(crate) enum EntryTerminator {
    /// カンマ区切り
    Comma(Vec<GreenElement>),
    /// 改行区切り
    Newline(Vec<GreenElement>),
    /// 末尾終端
    EndOfContainer,
}

/// Object の1エントリ（1キー分）
#[derive(Debug, Clone)]
pub(crate) struct ObjectEntry {
    /// 対象キー行の直前にある改行と行頭インデント
    pub leading_line_trivia: Vec<GreenElement>,
    /// 対象キーの直前にあるコメント群
    pub leading_comments: Vec<GreenElement>,
    /// キーと値のペア本体 (SyntaxKind::Pair)
    pub pair: GreenElement,
    /// 同じ行にある後ろの空白やインラインコメント
    pub trailing_inline_trivia: Vec<GreenElement>,
    /// カンマ区切り / 改行区切り / 末尾終端
    pub terminator: EntryTerminator,
}

/// 独立したトリビアブロック（どのキーにも属さない）
#[derive(Debug, Clone)]
pub(crate) struct StandaloneTriviaBlock {
    /// 独立コメント本体と、その直前の改行・インデントも含む
    pub elements: Vec<GreenElement>,
}

/// Object の要素（エントリまたは独立トリビア）
#[derive(Debug, Clone)]
pub(crate) enum ObjectItem {
    Entry(ObjectEntry),
    Standalone(StandaloneTriviaBlock),
}

/// Array の1エントリ（1要素分）
#[derive(Debug, Clone)]
pub(crate) struct ArrayEntry {
    /// 対象要素行の直前にある改行と行頭インデント
    pub leading_line_trivia: Vec<GreenElement>,
    /// 実際の値ノード (SyntaxKind::Value)
    pub value: GreenElement,
    /// 同じ行にある後ろの空白やインラインコメント
    pub trailing_inline_trivia: Vec<GreenElement>,
    /// カンマ区切り / 改行区切り / 末尾終端
    pub terminator: EntryTerminator,
}

/// Array の要素（エントリまたは独立トリビア）
#[derive(Debug, Clone)]
pub(crate) enum ArrayItem {
    Entry(ArrayEntry),
    Standalone(StandaloneTriviaBlock),
}

/// トリビアかどうかを判定
fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Whitespace
            | SyntaxKind::Newline
            | SyntaxKind::LineComment
            | SyntaxKind::HashComment
            | SyntaxKind::BlockComment
    )
}

/// コメントかどうかを判定
fn is_comment(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
    )
}

/// Object の children から ObjectItem のリストを構築
/// 空行判定は \n\s*\n を検出
pub(crate) fn build_object_items(children: &[GreenElement]) -> Vec<ObjectItem> {
    let mut items: Vec<ObjectItem> = Vec::new();
    let mut pending_trivia: Vec<GreenElement> = Vec::new();

    let mut i = 0;
    while i < children.len() {
        let elem = &children[i];

        match elem {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair.into() => {
                // Pair を見つけたら、新しいエントリを開始
                let pair = elem.clone();
                let leading_line_trivia = pending_trivia.clone();
                pending_trivia.clear();

                // trailing_inline_trivia を収集
                let mut trailing_inline_trivia = Vec::new();
                i += 1;
                while i < children.len() {
                    match &children[i] {
                        NodeOrToken::Token(t) => {
                            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            if kind == SyntaxKind::Whitespace {
                                // 同じ行内の空白は trailing inline
                                trailing_inline_trivia.push(children[i].clone());
                                i += 1;
                            } else if is_comment(kind) {
                                // インラインコメント
                                trailing_inline_trivia.push(children[i].clone());
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }

                // terminator を収集
                let terminator = if i < children.len() {
                    match &children[i] {
                        NodeOrToken::Token(t) => {
                            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            if kind == SyntaxKind::Comma {
                                // Comma とその後の trivia
                                let comma = children[i].clone();
                                i += 1;
                                let mut suffix = vec![comma];
                                while i < children.len() {
                                    match &children[i] {
                                        NodeOrToken::Token(t) => {
                                            let kind =
                                                crate::syntax::kind::HjsonLanguage::kind_from_raw(
                                                    t.kind(),
                                                );
                                            if kind == SyntaxKind::Newline
                                                || kind == SyntaxKind::Whitespace
                                            {
                                                suffix.push(children[i].clone());
                                                i += 1;
                                            } else {
                                                break;
                                            }
                                        }
                                        _ => break,
                                    }
                                }
                                EntryTerminator::Comma(suffix)
                            } else if kind == SyntaxKind::Newline {
                                // 改行区切り - 改行とインデントを terminator に含める
                                let newline = children[i].clone();
                                i += 1;
                                let mut suffix = vec![newline];
                                // インデントがあれば terminator に含める
                                if i < children.len() {
                                    if let NodeOrToken::Token(t) = &children[i] {
                                        let kind =
                                            crate::syntax::kind::HjsonLanguage::kind_from_raw(
                                                t.kind(),
                                            );
                                        if kind == SyntaxKind::Whitespace {
                                            suffix.push(children[i].clone());
                                            i += 1;
                                        }
                                    }
                                }
                                EntryTerminator::Newline(suffix)
                            } else {
                                EntryTerminator::EndOfContainer
                            }
                        }
                        _ => EntryTerminator::EndOfContainer,
                    }
                } else {
                    EntryTerminator::EndOfContainer
                };

                // 現在のエントリを作成
                let entry = ObjectEntry {
                    leading_line_trivia: leading_line_trivia.clone(),
                    leading_comments: Vec::new(),
                    pair,
                    trailing_inline_trivia,
                    terminator,
                };
                items.push(ObjectItem::Entry(entry));
            }
            NodeOrToken::Token(t) => {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());

                if kind == SyntaxKind::RBrace {
                    break;
                } else if is_trivia(kind) {
                    // trivia を保留
                    pending_trivia.push(elem.clone());

                    // 空行判定（\n\s*\n）
                    if has_blank_line(&pending_trivia) {
                        // 空白行を検出したら、それまでの trivia を standalone として確定
                        let split_idx = find_blank_line_split(&pending_trivia);
                        if split_idx > 0 {
                            let standalone_elements: Vec<_> =
                                pending_trivia.drain(0..split_idx).collect();
                            items.push(ObjectItem::Standalone(StandaloneTriviaBlock {
                                elements: standalone_elements,
                            }));
                        }
                    }

                    i += 1;
                } else {
                    // その他のトークンはスキップ
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    // 残っているトリビアは末尾 standalone として扱う
    if !pending_trivia.is_empty() {
        items.push(ObjectItem::Standalone(StandaloneTriviaBlock {
            elements: pending_trivia,
        }));
    }

    items
}

/// Array の children から ArrayItem のリストを構築
pub(crate) fn build_array_items(children: &[GreenElement]) -> Vec<ArrayItem> {
    let mut items: Vec<ArrayItem> = Vec::new();
    let mut pending_trivia: Vec<GreenElement> = Vec::new();

    let mut i = 0;
    while i < children.len() {
        let elem = &children[i];

        match elem {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value.into() => {
                let value = elem.clone();
                let leading_line_trivia = pending_trivia.clone();
                pending_trivia.clear();

                let mut trailing_inline_trivia = Vec::new();
                i += 1;
                while i < children.len() {
                    match &children[i] {
                        NodeOrToken::Token(t) => {
                            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            if kind == SyntaxKind::Whitespace || is_comment(kind) {
                                trailing_inline_trivia.push(children[i].clone());
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }

                let terminator = if i < children.len() {
                    match &children[i] {
                        NodeOrToken::Token(t) => {
                            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            if kind == SyntaxKind::Comma {
                                let comma = children[i].clone();
                                i += 1;
                                let mut suffix = vec![comma];
                                while i < children.len() {
                                    match &children[i] {
                                        NodeOrToken::Token(t) => {
                                            let kind =
                                                crate::syntax::kind::HjsonLanguage::kind_from_raw(
                                                    t.kind(),
                                                );
                                            if kind == SyntaxKind::Newline
                                                || kind == SyntaxKind::Whitespace
                                            {
                                                suffix.push(children[i].clone());
                                                i += 1;
                                            } else {
                                                break;
                                            }
                                        }
                                        _ => break,
                                    }
                                }
                                EntryTerminator::Comma(suffix)
                            } else if kind == SyntaxKind::Newline {
                                let newline = children[i].clone();
                                i += 1;
                                if i < children.len() {
                                    if let NodeOrToken::Token(t) = &children[i] {
                                        let kind =
                                            crate::syntax::kind::HjsonLanguage::kind_from_raw(
                                                t.kind(),
                                            );
                                        if kind == SyntaxKind::Whitespace {
                                            pending_trivia.push(children[i].clone());
                                            i += 1;
                                        }
                                    }
                                }
                                EntryTerminator::Newline(vec![newline])
                            } else {
                                EntryTerminator::EndOfContainer
                            }
                        }
                        _ => EntryTerminator::EndOfContainer,
                    }
                } else {
                    EntryTerminator::EndOfContainer
                };

                items.push(ArrayItem::Entry(ArrayEntry {
                    leading_line_trivia,
                    value,
                    trailing_inline_trivia,
                    terminator,
                }));
            }
            NodeOrToken::Token(t) => {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                if kind == SyntaxKind::RBracket {
                    break;
                } else if is_trivia(kind) {
                    pending_trivia.push(elem.clone());
                    if has_blank_line(&pending_trivia) {
                        let split_idx = find_blank_line_split(&pending_trivia);
                        if split_idx > 0 {
                            let standalone_elements: Vec<_> =
                                pending_trivia.drain(0..split_idx).collect();
                            items.push(ArrayItem::Standalone(StandaloneTriviaBlock {
                                elements: standalone_elements,
                            }));
                        }
                    }
                    i += 1;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    if !pending_trivia.is_empty() {
        items.push(ArrayItem::Standalone(StandaloneTriviaBlock {
            elements: pending_trivia,
        }));
    }

    items
}

pub(crate) fn rebuild_object_children_for_format(
    lbrace: Option<GreenElement>,
    items: Vec<ObjectItem>,
    rbrace: Option<GreenElement>,
) -> Vec<GreenElement> {
    let mut new_children = Vec::new();
    if let Some(lbrace) = lbrace {
        new_children.push(lbrace);
    }
    for item in items {
        match item {
            ObjectItem::Entry(entry) => {
                remove_orphan_indent_before_entry(&mut new_children);
                new_children.extend(entry.leading_line_trivia);
                new_children.extend(entry.leading_comments);
                new_children.push(entry.pair);
                new_children.extend(entry.trailing_inline_trivia);
                match entry.terminator {
                    EntryTerminator::Comma(suffix) | EntryTerminator::Newline(suffix) => {
                        new_children.extend(suffix);
                    }
                    EntryTerminator::EndOfContainer => {}
                }
            }
            ObjectItem::Standalone(block) => new_children.extend(block.elements),
        }
    }
    if let Some(rbrace) = rbrace {
        new_children.push(rbrace);
    }
    new_children
}

pub(crate) fn rebuild_array_children_for_format(
    lbracket: Option<GreenElement>,
    items: Vec<ArrayItem>,
    rbracket: Option<GreenElement>,
) -> Vec<GreenElement> {
    let mut new_children = Vec::new();
    if let Some(lbracket) = lbracket {
        new_children.push(lbracket);
    }
    for item in items {
        match item {
            ArrayItem::Entry(entry) => {
                new_children.extend(entry.leading_line_trivia);
                new_children.push(entry.value);
                new_children.extend(entry.trailing_inline_trivia);
                match entry.terminator {
                    EntryTerminator::Comma(suffix) | EntryTerminator::Newline(suffix) => {
                        new_children.extend(suffix);
                    }
                    EntryTerminator::EndOfContainer => {}
                }
            }
            ArrayItem::Standalone(block) => new_children.extend(block.elements),
        }
    }
    if let Some(rbracket) = rbracket {
        new_children.push(rbracket);
    }
    new_children
}

fn remove_array_entry(arr_node: &RedNode, index: usize) -> Result<GreenNode, HjsonEditError> {
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

    let mut found = false;
    let mut entry_index = 0usize;
    let mut new_items: Vec<ArrayItem> = Vec::new();
    let mut pending_leading: Vec<GreenElement> = Vec::new();

    for item in items {
        match item {
            ArrayItem::Entry(mut entry) => {
                if entry_index == index {
                    found = true;
                    if new_items
                        .iter()
                        .all(|item| !matches!(item, ArrayItem::Entry(_)))
                    {
                        pending_leading = entry.leading_line_trivia.clone();
                    }
                    entry_index += 1;
                    continue;
                }

                if !pending_leading.is_empty() {
                    let mut merged = pending_leading;
                    merged.extend(entry.leading_line_trivia);
                    entry.leading_line_trivia = merged;
                    pending_leading = Vec::new();
                }

                new_items.push(ArrayItem::Entry(entry));
                entry_index += 1;
            }
            ArrayItem::Standalone(block) => new_items.push(ArrayItem::Standalone(block)),
        }
    }

    if !found {
        return Err(HjsonEditError::InvalidIndex {
            index,
            count: entry_index,
        });
    }

    Ok(create_node(
        SyntaxKind::Array,
        rebuild_array_children_for_format(lbracket, new_items, rbracket),
    ))
}

fn insert_array_entry_structural(
    arr_node: &RedNode,
    index: usize,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
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

    let mut new_items = Vec::new();
    let mut entry_index = 0usize;
    let insert_at = index;
    let is_append = index == usize::MAX;
    let mut inserted = false;
    let mut last_entry_indent: Option<GreenElement> = None;
    let mut trailing_standalone = Vec::new();

    for item in items {
        match item {
            ArrayItem::Entry(entry) => {
                if let Some(indent) = entry
                    .leading_line_trivia
                    .iter()
                    .rev()
                    .find(|elem| is_token_of_kind(elem, SyntaxKind::Whitespace))
                    .cloned()
                {
                    last_entry_indent = Some(indent);
                }
                if !is_append && !inserted && entry_index == insert_at {
                    new_items.push(ArrayItem::Entry(ArrayEntry {
                        leading_line_trivia: entry.leading_line_trivia.clone(),
                        value: NodeOrToken::Node(create_value_node(value, string_style, None)?),
                        trailing_inline_trivia: Vec::new(),
                        terminator: EntryTerminator::Newline(vec![NodeOrToken::Token(
                            create_token(SyntaxKind::Newline, "\n"),
                        )]),
                    }));
                    inserted = true;
                }

                entry_index += 1;
                new_items.push(ArrayItem::Entry(entry));
            }
            ArrayItem::Standalone(block) => {
                if is_append {
                    trailing_standalone.push(ArrayItem::Standalone(block));
                } else {
                    new_items.push(ArrayItem::Standalone(block));
                }
            }
        }
    }

    if !inserted {
        let terminator = if trailing_standalone.is_empty() {
            match rbracket.as_ref() {
                Some(elem) if is_token_of_kind(elem, SyntaxKind::RBracket) => {
                    EntryTerminator::Newline(vec![NodeOrToken::Token(create_token(
                        SyntaxKind::Newline,
                        "\n",
                    ))])
                }
                _ => EntryTerminator::EndOfContainer,
            }
        } else {
            EntryTerminator::Newline(vec![NodeOrToken::Token(create_token(
                SyntaxKind::Newline,
                "\n",
            ))])
        };
        let leading_line_trivia = if new_items
            .iter()
            .any(|item| matches!(item, ArrayItem::Entry(_)))
        {
            let mut trivia = Vec::new();
            if let Some(indent) = last_entry_indent {
                trivia.push(indent);
            }
            trivia
        } else {
            Vec::new()
        };
        new_items.push(ArrayItem::Entry(ArrayEntry {
            leading_line_trivia,
            value: NodeOrToken::Node(create_value_node(value, string_style, None)?),
            trailing_inline_trivia: Vec::new(),
            terminator,
        }));
    }

    new_items.extend(trailing_standalone);

    Ok(create_node(
        SyntaxKind::Array,
        rebuild_array_children_for_format(lbracket, new_items, rbracket),
    ))
}

/// 保留中のトリビアに空行が含まれるか判定（\n\s*\n のパターンを検出）
fn has_blank_line(trivia: &[GreenElement]) -> bool {
    let mut had_newline = false;

    for elem in trivia {
        if let NodeOrToken::Token(t) = elem {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Newline {
                if had_newline {
                    // \n\n のパターン（空白が含まれていてもよい）
                    return true;
                }
                had_newline = true;
            } else if kind == SyntaxKind::Whitespace {
                // 改行後の空白は空行判定に含まれる
            } else if is_comment(kind) {
                // コメントがあると空行判定をリセット
                had_newline = false;
            } else {
                had_newline = false;
            }
        }
    }

    false
}

/// 空行で split する位置を見つける
fn find_blank_line_split(trivia: &[GreenElement]) -> usize {
    let mut had_newline = false;

    for (i, elem) in trivia.iter().enumerate() {
        if let NodeOrToken::Token(t) = elem {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Newline {
                if had_newline {
                    // 2つ目の改行が見つかった位置で split
                    return i;
                }
                had_newline = true;
            } else if kind == SyntaxKind::Whitespace {
                // 空白は無視
            } else if is_comment(kind) {
                // コメントがあればリセット
                had_newline = false;
            } else {
                had_newline = false;
            }
        }
    }

    trivia.len()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dot_path() {
        assert_eq!(
            parse_dot_path("server.host").unwrap(),
            vec![
                PathSegment::Key("server".to_string()),
                PathSegment::Key("host".to_string())
            ]
        );

        assert_eq!(
            parse_dot_path("items[0].name").unwrap(),
            vec![
                PathSegment::Key("items".to_string()),
                PathSegment::Index(0),
                PathSegment::Key("name".to_string())
            ]
        );

        assert!(matches!(
            parse_dot_path("items[abc]"),
            Err(HjsonEditError::InvalidPath { .. })
        ));
        assert!(matches!(
            parse_dot_path("items[1x]"),
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
            Some(HjsonValue::String("line1\nline2".to_string()))
        );
    }
}
