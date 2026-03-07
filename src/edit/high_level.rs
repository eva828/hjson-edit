//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

use crate::edit::api::{create_node, create_token, EditError as LowLevelEditError, GreenNodeEdit, RedNodeEdit};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeExt};
use rowan::{GreenNode, NodeOrToken};
use rowan::Language;

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
    InvalidNodeKind { expected: SyntaxKind, found: SyntaxKind },
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
            HjsonEditError::InvalidIndex { index, count } => write!(f, "Invalid index: {} (child count: {})", index, count),
            HjsonEditError::InvalidNodeKind { expected, found } => {
                write!(f, "Expected {:?}, found {:?}", expected, found)
            }
            HjsonEditError::LowLevel(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for HjsonEditError {}

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
    fn into_path(self) -> Vec<PathSegment>;
}

impl IntoPath for &str {
    fn into_path(self) -> Vec<PathSegment> {
        parse_dot_path(self)
    }
}

impl IntoPath for &[&str] {
    fn into_path(self) -> Vec<PathSegment> {
        self.iter().map(|s| PathSegment::Key(s.to_string())).collect()
    }
}

impl IntoPath for Vec<PathSegment> {
    fn into_path(self) -> Vec<PathSegment> {
        self
    }
}

impl IntoPath for &[PathSegment] {
    fn into_path(self) -> Vec<PathSegment> {
        self.to_vec()
    }
}

/// ドット記法のパスをパースする
fn parse_dot_path(s: &str) -> Vec<PathSegment> {
    if s.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !current.is_empty() {
                    segments.push(PathSegment::Key(current.clone()));
                    current.clear();
                }
            }
            '[' => {
                if !current.is_empty() {
                    segments.push(PathSegment::Key(current.clone()));
                    current.clear();
                }
                let mut index_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ']' {
                        chars.next();
                        break;
                    }
                    if c.is_ascii_digit() {
                        index_str.push(c);
                        chars.next();
                    } else {
                        chars.next();
                    }
                }
                if let Ok(index) = index_str.parse::<usize>() {
                    segments.push(PathSegment::Index(index));
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        segments.push(PathSegment::Key(current));
    }

    segments
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
    NaN,
    Infinity,
    NegInfinity,
    HexInteger(i64),
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

/// コメントの位置
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommentPosition {
    Before,
    Inline,
    After,
}

/// コメントのスタイル
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommentStyle {
    Line,
    Hash,
    Block,
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
        Ok(Self { green: result.green })
    }

    /// GreenNodeからドキュメントを作成
    pub fn from_green(green: GreenNode) -> Self {
        Self { green }
    }

    /// ドキュメントを文字列に変換（フォーマット保持）
    pub fn to_string(&self) -> String {
        use crate::syntax::green::GreenNodeExt;
        self.green.text()
    }

    /// GreenNodeへの参照を取得
    pub fn green_node(&self) -> &GreenNode {
        &self.green
    }

    /// GreenNodeを所有権ごと取得
    pub fn into_green_node(self) -> GreenNode {
        self.green
    }

    /// パスに対応する値を取得
    pub fn get(&self, path: impl IntoPath) -> Option<HjsonValue> {
        let path = path.into_path();
        let node = self.navigate_to_node(&path)?;
        node_to_value(&node)
    }

    /// パスが存在するか確認
    pub fn contains(&self, path: impl IntoPath) -> bool {
        let path = path.into_path();
        self.navigate_to_node(&path).is_some()
    }

    /// 指定パスのオブジェクトのキー一覧を取得
    pub fn keys(&self, path: impl IntoPath) -> Option<Vec<String>> {
        let path = path.into_path();
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
        let path = path.into_path();
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
        let path = path.into_path();
        let value = value.into();

        if path.is_empty() {
            // ルートを置換
            let new_green = value_to_green_node(&value)?;
            return Ok(Self { green: new_green });
        }

        let parent_path = &path[..path.len() - 1];
        let last = path.last().unwrap();

        let parent = self
            .navigate_to_node(parent_path)
            .ok_or_else(|| HjsonEditError::InvalidPath {
                path: format!("{:?}", parent_path),
            })?;

        match last {
            PathSegment::Key(key) => {
                // オブジェクト内のキーを探して更新
                let obj_node = if let Some(obj) = parent.child_by_kind(SyntaxKind::Object) {
                    obj
                } else if let Some(val) = parent.child_by_kind(SyntaxKind::Value) {
                    val.child_by_kind(SyntaxKind::Object).ok_or_else(|| {
                        HjsonEditError::InvalidNodeKind {
                            expected: SyntaxKind::Object,
                            found: val.kind(),
                        }
                    })?
                } else {
                    return Err(HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Object,
                        found: parent.kind(),
                    });
                };

                let obj_green: GreenNode = obj_node.to_green();
                let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();

                // キーを探して置換または追加
                let mut new_children = children.clone();
                let mut found = false;

                // Find Pair nodes in the children and check their keys
                let pair_indices: Vec<usize> = children
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        if let NodeOrToken::Node(n) = c {
                            if n.kind() == SyntaxKind::Pair.into() {
                                return Some(i);
                            }
                        }
                        None
                    })
                    .collect();

                for idx in pair_indices {
                    if found {
                        break;
                    }
                    // Get the Pair node at this index directly from children
                    if let NodeOrToken::Node(pair_green) = &children[idx] {
                        // Create a RedNode from the GreenNode to access Key
                        let pair_red = RedNode::new_root(pair_green.clone());
                        if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                            if let Some(key_text) = extract_key_text(&key_node) {
                                if key_text == *key {
                                    // Found the Pair to replace
                                    let new_pair = create_pair_node(key, &value)?;
                                    new_children[idx] = NodeOrToken::Node(new_pair);
                                    found = true;
                                }
                            }
                        }
                    }
                }

                if !found {
                    // 新規追加
                    let new_pair = create_pair_node(key, &value)?;

                    // RBraceの前に挿入
                    let insert_pos = new_children
                        .iter()
                        .position(|c| is_token_of_kind(c, SyntaxKind::RBrace))
                        .unwrap_or(new_children.len());

                    if insert_pos > 1 && !new_children.is_empty() {
                        new_children.insert(
                            insert_pos,
                            NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")),
                        );
                        new_children.insert(
                            insert_pos + 1,
                            NodeOrToken::Token(create_token(SyntaxKind::Whitespace, " ")),
                        );
                        new_children.insert(insert_pos + 2, NodeOrToken::Node(new_pair));
                    } else {
                        new_children.insert(insert_pos, NodeOrToken::Node(new_pair));
                    }
                }

                let new_obj = create_node(SyntaxKind::Object, new_children);
                self.replace_green_node(&obj_node.path_from_root(), new_obj)
            }
            PathSegment::Index(idx) => {
                // 配列内のインデックスを更新
                let arr_node = if let Some(arr) = parent.child_by_kind(SyntaxKind::Array) {
                    arr
                } else if let Some(val) = parent.child_by_kind(SyntaxKind::Value) {
                    val.child_by_kind(SyntaxKind::Array).ok_or_else(|| {
                        HjsonEditError::InvalidNodeKind {
                            expected: SyntaxKind::Array,
                            found: val.kind(),
                        }
                    })?
                } else {
                    return Err(HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: parent.kind(),
                    });
                };

                let arr_green: GreenNode = arr_node.to_green();
                let children: Vec<GreenElement> = arr_green.children().map(clone_element).collect();

                // 値インデックスを探す
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

                if *idx >= value_indices.len() {
                    return Err(HjsonEditError::InvalidIndex {
                        index: *idx,
                        count: value_indices.len(),
                    });
                }

                let target_idx = value_indices[*idx];
                let new_value = create_value_node(&value)?;

                let mut new_children = children;
                new_children[target_idx] = NodeOrToken::Node(new_value);

                let new_arr = create_node(SyntaxKind::Array, new_children);
                self.replace_green_node(&arr_node.path_from_root(), new_arr)
            }
        }
    }

    /// キー/値を削除
    pub fn remove(&self, path: impl IntoPath) -> Result<Self, HjsonEditError> {
        let path = path.into_path();

        if path.is_empty() {
            return Err(HjsonEditError::InvalidPath {
                path: "empty".to_string(),
            });
        }

        let parent_path = &path[..path.len() - 1];
        let last = path.last().unwrap();

        let parent = self
            .navigate_to_node(parent_path)
            .ok_or_else(|| HjsonEditError::InvalidPath {
                path: format!("{:?}", parent_path),
            })?;

        match last {
            PathSegment::Key(key) => {
                let obj_node = if let Some(obj) = parent.child_by_kind(SyntaxKind::Object) {
                    obj
                } else if let Some(val) = parent.child_by_kind(SyntaxKind::Value) {
                    val.child_by_kind(SyntaxKind::Object).ok_or_else(|| {
                        HjsonEditError::InvalidNodeKind {
                            expected: SyntaxKind::Object,
                            found: val.kind(),
                        }
                    })?
                } else {
                    return Err(HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Object,
                        found: parent.kind(),
                    });
                };

                let obj_green: GreenNode = obj_node.to_green();
                let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();

                // キーを探して削除
                let mut new_children = Vec::new();
                let mut i = 0;
                let mut found = false;

                while i < children.len() {
                    let child = &children[i];

                    if let NodeOrToken::Node(n) = child {
                        if n.kind() == SyntaxKind::Pair.into() {
                            // Pairノードのキーを確認（children[i] から直接取得）
                            let pair_red = RedNode::new_root(n.clone());
                            if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                                if let Some(key_text) = extract_key_text(&key_node) {
                                    if key_text == *key {
                                        found = true;
                                        // このPairと後続のカンマをスキップ
                                        i += 1;
                                        if i < children.len() {
                                            if let NodeOrToken::Token(t) = &children[i] {
                                                if crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()) == SyntaxKind::Comma {
                                                    i += 1;
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    new_children.push(child.clone());
                    i += 1;
                }

                if !found {
                    return Err(HjsonEditError::InvalidPath {
                        path: key.clone(),
                    });
                }

                let new_obj = create_node(SyntaxKind::Object, new_children);
                self.replace_green_node(&obj_node.path_from_root(), new_obj)
            }
            PathSegment::Index(idx) => {
                let arr_node = if let Some(arr) = parent.child_by_kind(SyntaxKind::Array) {
                    arr
                } else if let Some(val) = parent.child_by_kind(SyntaxKind::Value) {
                    val.child_by_kind(SyntaxKind::Array).ok_or_else(|| {
                        HjsonEditError::InvalidNodeKind {
                            expected: SyntaxKind::Array,
                            found: val.kind(),
                        }
                    })?
                } else {
                    return Err(HjsonEditError::InvalidNodeKind {
                        expected: SyntaxKind::Array,
                        found: parent.kind(),
                    });
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

                if *idx >= value_indices.len() {
                    return Err(HjsonEditError::InvalidIndex {
                        index: *idx,
                        count: value_indices.len(),
                    });
                }

                let remove_idx = value_indices[*idx];
                let new_arr = arr_green.remove_child(remove_idx)?;
                self.replace_green_node(&arr_node.path_from_root(), new_arr)
            }
        }
    }

    /// オブジェクトに新しいキーを追加
    pub fn insert(
        &self,
        path: impl IntoPath,
        _key: &str,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.into_path();
        let value = value.into();

        // 既存のsetメソッドを使えば、キーが存在しない場合は自動的に追加される
        self.set(path, value)
    }

    /// 配列の末尾に要素を追加
    pub fn push(
        &self, path: impl IntoPath, value: impl Into<HjsonValue>) -> Result<Self, HjsonEditError> {
        let path = path.into_path();
        let value = value.into();

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

        // RBracketの前に挿入
        let insert_pos = children
            .iter()
            .position(|c| is_token_of_kind(c, SyntaxKind::RBracket))
            .unwrap_or(children.len());

        let mut new_children = children;
        let new_value = create_value_node(&value)?;

        if insert_pos > 1 {
            new_children.insert(
                insert_pos,
                NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")),
            );
            new_children.insert(insert_pos + 1, NodeOrToken::Node(new_value));
        } else {
            new_children.insert(insert_pos, NodeOrToken::Node(new_value));
        }

        let new_arr = create_node(SyntaxKind::Array, new_children);
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    /// 配列の末尾の要素を削除
    pub fn pop(&self, path: impl IntoPath) -> Result<Self, HjsonEditError> {
        let path = path.into_path();

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
            return Err(HjsonEditError::InvalidIndex {
                index: 0,
                count: 0,
            });
        }

        let remove_idx = *value_indices.last().unwrap();
        let new_arr = arr_green.remove_child(remove_idx)?;
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    /// 配列の指定位置に要素を挿入
    pub fn insert_at(
        &self, path: impl IntoPath, index: usize, value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.into_path();
        let value = value.into();

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

        let insert_pos = if index >= value_indices.len() {
            // 末尾に追加
            children
                .iter()
                .position(|c| is_token_of_kind(c, SyntaxKind::RBracket))
                .unwrap_or(children.len())
        } else {
            value_indices[index]
        };

        let mut new_children = children;
        let new_value = create_value_node(&value)?;

        if !value_indices.is_empty() && insert_pos > 0 {
            new_children.insert(
                insert_pos,
                NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")),
            );
            new_children.insert(insert_pos + 1, NodeOrToken::Node(new_value));
        } else {
            new_children.insert(insert_pos, NodeOrToken::Node(new_value));
        }

        let new_arr = create_node(SyntaxKind::Array, new_children);
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    /// 配列の指定位置の要素を削除
    pub fn remove_at(&self, path: impl IntoPath, index: usize,
    ) -> Result<Self, HjsonEditError> {
        let path = path.into_path();

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

        if index >= value_indices.len() {
            return Err(HjsonEditError::InvalidIndex {
                index,
                count: value_indices.len(),
            });
        }

        let remove_idx = value_indices[index];
        let new_arr = arr_green.remove_child(remove_idx)?;
        self.replace_green_node(&arr_node.path_from_root(), new_arr)
    }

    /// ドキュメント先頭のコメントを取得
    pub fn get_header_comment(&self) -> Vec<String> {
        let red = RedNode::new_root(self.green.clone());
        let mut comments = Vec::new();

        for elem in red.children_with_tokens() {
            if let Some(token) = elem.as_token() {
                let kind = token.kind();
                if matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                ) {
                    comments.push(token.text().to_string());
                }
            } else {
                break;
            }
        }

        comments
    }

    /// ドキュメント末尾のコメントを取得
    pub fn get_footer_comment(&self) -> Vec<String> {
        let red = RedNode::new_root(self.green.clone());
        let mut comments = Vec::new();

        // 子要素を収集
        let children: Vec<_> = red.children_with_tokens().collect();

        for elem in children.iter().rev() {
            if let Some(token) = elem.as_token() {
                let kind = token.kind();
                if matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                ) {
                    comments.push(token.text().to_string());
                } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                    break;
                }
            } else {
                break;
            }
        }

        comments.reverse();
        comments
    }

    /// 他のドキュメントとの差分行を検出
    pub fn changed_lines(&self, other: &Self) -> Vec<usize> {
        let before = self.to_string();
        let after = other.to_string();
        crate::diff::simple_line_diff(&before, &after)
    }

    // -------------------------------------------------------------------------
    // Internal Helpers
    // -------------------------------------------------------------------------

    fn navigate_to_node(&self,
        path: &[PathSegment],
    ) -> Option<RedNode> {
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

fn extract_key_text(key_node: &RedNode) -> Option<String> {
    for elem in key_node.children_with_tokens() {
        if let Some(token) = elem.as_token() {
            let kind = token.kind();
            match kind {
                SyntaxKind::Ident | SyntaxKind::UnquotedString => {
                    return Some(token.text().to_string());
                }
                SyntaxKind::String | SyntaxKind::SingleQuoteString => {
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
            if matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment) {
                continue;
            }

            return match kind {
                SyntaxKind::String | SyntaxKind::SingleQuoteString | SyntaxKind::BlockString => {
                    if text.len() >= 2 {
                        Some(HjsonValue::String(text[1..text.len() - 1].to_string()))
                    } else {
                        Some(HjsonValue::String(text.to_string()))
                    }
                }
                SyntaxKind::UnquotedString => Some(HjsonValue::String(text.to_string())),
                SyntaxKind::Integer => text.parse::<i64>().ok().map(HjsonValue::Integer),
                SyntaxKind::HexInteger => {
                    if text.starts_with("0x") || text.starts_with("0X") {
                        i64::from_str_radix(&text[2..], 16).ok().map(HjsonValue::HexInteger)
                    } else {
                        None
                    }
                }
                SyntaxKind::Float => text.parse::<f64>().ok().map(HjsonValue::Float),
                SyntaxKind::True => Some(HjsonValue::Bool(true)),
                SyntaxKind::False => Some(HjsonValue::Bool(false)),
                SyntaxKind::Null => Some(HjsonValue::Null),
                SyntaxKind::NaN => Some(HjsonValue::NaN),
                SyntaxKind::Infinity => Some(HjsonValue::Infinity),
                SyntaxKind::NegInfinity => Some(HjsonValue::NegInfinity),
                _ => None,
            };
        }
    }
    None
}

fn value_to_green_element(value: &HjsonValue) -> Result<GreenElement, HjsonEditError> {
    match value {
        HjsonValue::String(s) => {
            let token = create_token(SyntaxKind::String, &format!("\"{}\"", s));
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Integer(n) => {
            let token = create_token(SyntaxKind::Integer, &n.to_string());
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Float(n) => {
            let token = create_token(SyntaxKind::Float, &format!("{}", n));
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
        HjsonValue::NaN => {
            let token = create_token(SyntaxKind::NaN, "NaN");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Infinity => {
            let token = create_token(SyntaxKind::Infinity, "Infinity");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::NegInfinity => {
            let token = create_token(SyntaxKind::NegInfinity, "-Infinity");
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::HexInteger(n) => {
            let token = create_token(SyntaxKind::HexInteger, &format!("0x{:X}", n));
            Ok(NodeOrToken::Token(token))
        }
        HjsonValue::Array(items) => {
            let mut children = vec![NodeOrToken::Token(create_token(SyntaxKind::LBracket, "["))];

            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Comma, ",")));
                }
                let val_node = create_value_node(item)?;
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
                let pair_node = create_pair_node(key, val)?;
                children.push(NodeOrToken::Node(pair_node));
            }

            children.push(NodeOrToken::Token(create_token(SyntaxKind::RBrace, "}")));

            let node = create_node(SyntaxKind::Object, children);
            Ok(NodeOrToken::Node(node))
        }
    }
}

fn value_to_green_node(value: &HjsonValue) -> Result<GreenNode, HjsonEditError> {
    match value_to_green_element(value)? {
        NodeOrToken::Node(n) => Ok(n),
        NodeOrToken::Token(t) => {
            // スカラー値の場合はValueノードでラップ
            Ok(create_node(SyntaxKind::Value, vec![NodeOrToken::Token(t)]))
        }
    }
}

fn create_value_node(value: &HjsonValue) -> Result<GreenNode, HjsonEditError> {
    let inner = value_to_green_element(value)?;
    let children = vec![inner];
    Ok(create_node(SyntaxKind::Value, children))
}

fn create_pair_node(key: &str, value: &HjsonValue) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();

    let key_token = if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        create_token(SyntaxKind::Ident, key)
    } else {
        create_token(SyntaxKind::String, &format!("\"{}\"", key))
    };
    let key_node = create_node(SyntaxKind::Key, vec![NodeOrToken::Token(key_token)]);
    children.push(NodeOrToken::Node(key_node));

    children.push(NodeOrToken::Token(create_token(SyntaxKind::Colon, ":")));

    let val_node = create_value_node(value)?;
    children.push(NodeOrToken::Node(val_node));

    Ok(create_node(SyntaxKind::Pair, children))
}

fn clone_element(elem: rowan::NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>) -> GreenElement {
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

fn is_token_of_kind(elem: &GreenElement, kind: SyntaxKind) -> bool {
    if let NodeOrToken::Token(t) = elem {
        crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()) == kind
    } else {
        false
    }
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
            parse_dot_path("server.host"),
            vec![PathSegment::Key("server".to_string()), PathSegment::Key("host".to_string())]
        );

        assert_eq!(
            parse_dot_path("items[0].name"),
            vec![
                PathSegment::Key("items".to_string()),
                PathSegment::Index(0),
                PathSegment::Key("name".to_string())
            ]
        );
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

        assert_eq!(doc.get("name"), Some(HjsonValue::String("test".to_string())));
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
    fn test_document_get_array() {
        let input = r#"{"items": ["a", "b", "c"]}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        assert_eq!(doc.get("items[0]"), Some(HjsonValue::String("a".to_string())));
        assert_eq!(doc.get("items[1]"), Some(HjsonValue::String("b".to_string())));
    }

    #[test]
    fn test_document_set_simple() {
        let input = r#"{"key": "old"}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        let new_doc = doc.set("key", "new").unwrap();
        assert_eq!(new_doc.get("key"), Some(HjsonValue::String("new".to_string())));
        assert_eq!(doc.get("key"), Some(HjsonValue::String("old".to_string())));
    }

    #[test]
    fn test_document_push() {
        let input = r#"{"items": [1, 2]}"#;
        let doc = HjsonDocument::parse(input).unwrap();

        let new_doc = doc.push("items", 3).unwrap();
        assert_eq!(new_doc.len("items"), Some(3));
    }

    #[test]
    fn test_special_values() {
        let doc = HjsonDocument::parse(r#"{"a": NaN, "b": Infinity, "c": 0xFF}"#).unwrap();

        assert_eq!(doc.get("a"), Some(HjsonValue::NaN));
        assert_eq!(doc.get("b"), Some(HjsonValue::Infinity));
        assert_eq!(doc.get("c"), Some(HjsonValue::HexInteger(255)));
    }
}
