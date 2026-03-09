//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

use crate::edit::api::{
    create_node, create_token, EditError as LowLevelEditError, GreenNodeEdit, RedNodeEdit,
};
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
        self.iter()
            .map(|s| PathSegment::Key(s.to_string()))
            .collect()
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

        let parent =
            self.navigate_to_node(parent_path)
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

        let parent =
            self.navigate_to_node(parent_path)
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

                // LBrace と RBrace を抽出
                let lbrace = children
                    .iter()
                    .find(|e| {
                        if let NodeOrToken::Token(t) = e {
                            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            kind == SyntaxKind::LBrace
                        } else {
                            false
                        }
                    })
                    .cloned();

                let rbrace = children
                    .iter()
                    .find(|e| {
                        if let NodeOrToken::Token(t) = e {
                            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            kind == SyntaxKind::RBrace
                        } else {
                            false
                        }
                    })
                    .cloned();

                // 新しい ObjectItem モデルで解析（LBrace/RBrace は無視される）
                let items = build_object_items(&children);

                // 削除対象キーを探す
                let mut found = false;
                let mut new_items: Vec<ObjectItem> = Vec::new();
                let mut pending_terminator: Option<EntryTerminator> = None;

                for item in items {
                    match &item {
                        ObjectItem::Entry(entry) => {
                            // キー名を取得
                            if let NodeOrToken::Node(pair_node) = &entry.pair {
                                let pair_red = RedNode::new_root(pair_node.clone());
                                if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                                    if let Some(key_text) = extract_key_text(&key_node) {
                                        if key_text == *key {
                                            // 削除対象のエントリをスキップ
                                            // その terminator は次のエントリに統合
                                            found = true;
                                            pending_terminator = Some(entry.terminator.clone());
                                            continue;
                                        }
                                    }
                                }
                            }

                            // 削除したエントリの改行終端のみ、次のエントリの前置トリビアに統合
                            let mut modified_entry = entry.clone();
                            if let Some(ref term) = pending_terminator {
                                if let EntryTerminator::Newline(suffix) = term {
                                    let mut new_leading = suffix.clone();
                                    new_leading.extend(entry.leading_line_trivia.iter().cloned());
                                    modified_entry.leading_line_trivia = new_leading;
                                }
                                pending_terminator = None;
                            }

                            new_items.push(ObjectItem::Entry(modified_entry));
                        }
                        ObjectItem::Standalone(_) => {
                            // 独立トリビアは常に保持
                            new_items.push(item);
                        }
                    }
                }

                if !found {
                    return Err(HjsonEditError::InvalidPath { path: key.clone() });
                }

                // ObjectItem から children を再構築
                let mut new_children: Vec<GreenElement> = Vec::new();

                // LBrace を追加
                if let Some(lbrace) = lbrace {
                    new_children.push(lbrace);
                }

                for item in new_items {
                    match item {
                        ObjectItem::Entry(entry) => {
                            // エントリを再構成
                            new_children.extend(entry.leading_line_trivia.into_iter());
                            new_children.push(entry.pair);
                            new_children.extend(entry.trailing_inline_trivia.into_iter());
                            // terminator を追加
                            match entry.terminator {
                                EntryTerminator::Comma(suffix) => {
                                    new_children.extend(suffix.into_iter());
                                }
                                EntryTerminator::Newline(suffix) => {
                                    new_children.extend(suffix.into_iter());
                                }
                                EntryTerminator::EndOfContainer => {
                                    // 何も追加しない
                                }
                            }
                        }
                        ObjectItem::Standalone(block) => {
                            new_children.extend(block.elements.into_iter());
                        }
                    }
                }

                // RBrace を追加
                if let Some(rbrace) = rbrace {
                    new_children.push(rbrace);
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
        &self,
        path: impl IntoPath,
        value: impl Into<HjsonValue>,
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
    pub fn remove_at(&self, path: impl IntoPath, index: usize) -> Result<Self, HjsonEditError> {
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

    /// コメントを設定
    pub fn comment_set(
        &self,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<Self, HjsonEditError> {
        match path {
            CommentPath::Key { dot_path } => {
                // キーの直前コメントを設定
                let path_segments: Vec<PathSegment> = parse_dot_path(dot_path);
                if path_segments.is_empty() {
                    return Err(HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    });
                }

                let key = match &path_segments[path_segments.len() - 1] {
                    PathSegment::Key(k) => k,
                    _ => {
                        return Err(HjsonEditError::InvalidPath {
                            path: dot_path.clone(),
                        })
                    }
                };

                // Find the parent object
                let parent_path = &path_segments[..path_segments.len() - 1];
                let obj: RedNode = if parent_path.is_empty() {
                    let red = RedNode::new_root(self.green.clone());
                    red.child_by_kind(SyntaxKind::Object).or_else(|| {
                        red.child_by_kind(SyntaxKind::Value)?
                            .child_by_kind(SyntaxKind::Object)
                    })
                } else {
                    self.navigate_to_node(parent_path).and_then(|parent| {
                        parent.child_by_kind(SyntaxKind::Object).or_else(|| {
                            parent
                                .child_by_kind(SyntaxKind::Value)?
                                .child_by_kind(SyntaxKind::Object)
                        })
                    })
                }
                .ok_or_else(|| HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Object,
                    found: SyntaxKind::Root,
                })?;

                let pair_idx =
                    find_pair_index(&obj, key).ok_or_else(|| HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    })?;

                let pair_node: RedNode = obj
                    .children_nodes()
                    .enumerate()
                    .filter(|(_, n): &(usize, RedNode)| n.kind() == SyntaxKind::Pair)
                    .nth(pair_idx)
                    .map(|(_, n)| n)
                    .ok_or_else(|| HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    })?;

                let pair_green = pair_node.to_green();
                let comment_style = determine_comment_style(&pair_green, style)?;

                let new_comment = create_comment_token(text, comment_style);
                let new_pair = insert_comment_before_pair(&pair_green, new_comment)?;

                self.replace_green_node(&pair_node.path_from_root(), new_pair)
            }
            CommentPath::Inline { dot_path } => {
                // キーのインラインコメントを設定
                let path_segments: Vec<PathSegment> = parse_dot_path(dot_path);
                if path_segments.is_empty() {
                    return Err(HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    });
                }

                let key = match &path_segments[path_segments.len() - 1] {
                    PathSegment::Key(k) => k,
                    _ => {
                        return Err(HjsonEditError::InvalidPath {
                            path: dot_path.clone(),
                        })
                    }
                };

                let parent_path = &path_segments[..path_segments.len() - 1];
                let obj: RedNode = if parent_path.is_empty() {
                    let red = RedNode::new_root(self.green.clone());
                    red.child_by_kind(SyntaxKind::Object).or_else(|| {
                        red.child_by_kind(SyntaxKind::Value)?
                            .child_by_kind(SyntaxKind::Object)
                    })
                } else {
                    self.navigate_to_node(parent_path).and_then(|parent| {
                        parent.child_by_kind(SyntaxKind::Object).or_else(|| {
                            parent
                                .child_by_kind(SyntaxKind::Value)?
                                .child_by_kind(SyntaxKind::Object)
                        })
                    })
                }
                .ok_or_else(|| HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Object,
                    found: SyntaxKind::Root,
                })?;

                let pair_idx =
                    find_pair_index(&obj, key).ok_or_else(|| HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    })?;

                let obj_green = obj.to_green();
                let new_obj = set_inline_comment(&obj_green, pair_idx, text, style)?;

                self.replace_green_node(&obj.path_from_root(), new_obj)
            }
            CommentPath::Header => {
                // ドキュメント先頭コメントを設定
                let red = RedNode::new_root(self.green.clone());
                let mut new_children: Vec<GreenElement> = Vec::new();
                for elem in red.children_with_tokens() {
                    match elem {
                        NodeOrToken::Node(n) => {
                            let node: GreenNode = n.green().clone().into_owned();
                            new_children.push(NodeOrToken::Node(node));
                        }
                        NodeOrToken::Token(t) => {
                            let text = t.text();
                            let token = rowan::GreenToken::new(t.kind().into(), text);
                            new_children.push(NodeOrToken::Token(token));
                        }
                    }
                }

                let style = style.unwrap_or(CommentStyle::Line);
                let new_comment = create_comment_token(text, style);

                // 既存のヘッダーコメントを探す
                let mut replaced = false;
                for (i, child) in new_children.clone().iter().enumerate() {
                    if let NodeOrToken::Token(t) = child {
                        let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                        if matches!(
                            kind,
                            SyntaxKind::LineComment
                                | SyntaxKind::HashComment
                                | SyntaxKind::BlockComment
                        ) {
                            new_children[i] = NodeOrToken::Token(new_comment.clone());
                            replaced = true;
                            break;
                        }
                    }
                    if !matches!(
                        child,
                        NodeOrToken::Token(t) if {
                            let k = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                            matches!(k, SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment)
                        }
                    ) {
                        break;
                    }
                }

                if !replaced {
                    // ヘッダーコメントが見つからなければ先頭に挿入
                    new_children.insert(0, NodeOrToken::Token(new_comment));
                    new_children.insert(
                        1,
                        NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")),
                    );
                }

                let new_root = create_node(SyntaxKind::Root, new_children);
                Ok(Self { green: new_root })
            }
            CommentPath::Footer => {
                // ドキュメント末尾コメントを設定
                let red = RedNode::new_root(self.green.clone());
                let mut children: Vec<GreenElement> = Vec::new();
                for elem in red.children_with_tokens() {
                    match elem {
                        NodeOrToken::Node(n) => {
                            let node: GreenNode = n.green().clone().into_owned();
                            children.push(NodeOrToken::Node(node));
                        }
                        NodeOrToken::Token(t) => {
                            let text = t.text();
                            let token = rowan::GreenToken::new(t.kind().into(), text);
                            children.push(NodeOrToken::Token(token));
                        }
                    }
                }

                let style = style.unwrap_or(CommentStyle::Line);
                let new_comment = create_comment_token(text, style);

                // 末尾のコメントを探す
                let mut replaced = false;
                let mut i = children.len();
                while i > 0 {
                    i -= 1;
                    let child = &children[i];
                    if let NodeOrToken::Token(t) = child {
                        let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                        if matches!(
                            kind,
                            SyntaxKind::LineComment
                                | SyntaxKind::HashComment
                                | SyntaxKind::BlockComment
                        ) {
                            children[i] = NodeOrToken::Token(new_comment.clone());
                            replaced = true;
                            break;
                        } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if !replaced {
                    // フッターコメントが見つからなければ末尾に追加
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")));
                    children.push(NodeOrToken::Token(new_comment));
                }

                let new_root = create_node(SyntaxKind::Root, children);
                Ok(Self { green: new_root })
            }
        }
    }

    /// コメントに行を追加
    pub fn comment_append(
        &self,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<Self, HjsonEditError> {
        // 現在のコメントを取得
        let current_comments = match path {
            CommentPath::Key { dot_path } => {
                // キーの直前コメントを取得
                // 省略：実装は get_header_comment などを参考
                Vec::new()
            }
            CommentPath::Inline { .. } => Vec::new(),
            CommentPath::Header => self.get_header_comment(),
            CommentPath::Footer => self.get_footer_comment(),
        };

        if current_comments.is_empty() {
            // 既存コメントがなければ新規作成
            return self.comment_set(path, text, style);
        }

        // 最後のコメントの末尾に新しい行を追加
        let last_comment = current_comments.last().unwrap();
        let new_text = format!("{}\n{}", last_comment, text);

        // 最後のコメントを置き換え
        self.comment_set(path, &new_text, style)
    }

    /// コメントを削除
    pub fn comment_remove(&self, path: &CommentPath) -> Result<Self, HjsonEditError> {
        match path {
            CommentPath::Key { dot_path } => {
                let path_segments: Vec<PathSegment> = parse_dot_path(dot_path);
                if path_segments.is_empty() {
                    return Err(HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    });
                }

                let key = match &path_segments[path_segments.len() - 1] {
                    PathSegment::Key(k) => k,
                    _ => {
                        return Err(HjsonEditError::InvalidPath {
                            path: dot_path.clone(),
                        })
                    }
                };

                let parent_path = &path_segments[..path_segments.len() - 1];
                let obj: RedNode = if parent_path.is_empty() {
                    let red = RedNode::new_root(self.green.clone());
                    red.child_by_kind(SyntaxKind::Object).or_else(|| {
                        red.child_by_kind(SyntaxKind::Value)?
                            .child_by_kind(SyntaxKind::Object)
                    })
                } else {
                    self.navigate_to_node(parent_path).and_then(|parent| {
                        parent.child_by_kind(SyntaxKind::Object).or_else(|| {
                            parent
                                .child_by_kind(SyntaxKind::Value)?
                                .child_by_kind(SyntaxKind::Object)
                        })
                    })
                }
                .ok_or_else(|| HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Object,
                    found: SyntaxKind::Root,
                })?;

                let pair_idx =
                    find_pair_index(&obj, key).ok_or_else(|| HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    })?;

                let pair_node: RedNode = obj
                    .children_nodes()
                    .enumerate()
                    .filter(|(_, n): &(usize, RedNode)| n.kind() == SyntaxKind::Pair)
                    .nth(pair_idx)
                    .map(|(_, n)| n)
                    .ok_or_else(|| HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    })?;

                let pair_green = pair_node.to_green();
                let new_pair = remove_comments_from_pair(&pair_green)?;

                self.replace_green_node(&pair_node.path_from_root(), new_pair)
            }
            CommentPath::Inline { dot_path } => {
                let path_segments: Vec<PathSegment> = parse_dot_path(dot_path);
                if path_segments.is_empty() {
                    return Err(HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    });
                }

                let key = match &path_segments[path_segments.len() - 1] {
                    PathSegment::Key(k) => k,
                    _ => {
                        return Err(HjsonEditError::InvalidPath {
                            path: dot_path.clone(),
                        })
                    }
                };

                let parent_path = &path_segments[..path_segments.len() - 1];
                let obj: RedNode = if parent_path.is_empty() {
                    let red = RedNode::new_root(self.green.clone());
                    red.child_by_kind(SyntaxKind::Object).or_else(|| {
                        red.child_by_kind(SyntaxKind::Value)?
                            .child_by_kind(SyntaxKind::Object)
                    })
                } else {
                    self.navigate_to_node(parent_path).and_then(|parent| {
                        parent.child_by_kind(SyntaxKind::Object).or_else(|| {
                            parent
                                .child_by_kind(SyntaxKind::Value)?
                                .child_by_kind(SyntaxKind::Object)
                        })
                    })
                }
                .ok_or_else(|| HjsonEditError::InvalidNodeKind {
                    expected: SyntaxKind::Object,
                    found: SyntaxKind::Root,
                })?;

                let pair_idx =
                    find_pair_index(&obj, key).ok_or_else(|| HjsonEditError::InvalidPath {
                        path: dot_path.clone(),
                    })?;

                let obj_green = obj.to_green();
                let new_obj = remove_inline_comment(&obj_green, pair_idx)?;

                self.replace_green_node(&obj.path_from_root(), new_obj)
            }
            CommentPath::Header => {
                let red = RedNode::new_root(self.green.clone());
                let mut new_children: Vec<GreenElement> = Vec::new();
                let mut skip = true;

                for elem in red.children_with_tokens() {
                    match elem {
                        NodeOrToken::Node(n) => {
                            if skip {
                                let node: GreenNode = n.green().clone().into_owned();
                                new_children.push(NodeOrToken::Node(node));
                                skip = false;
                            } else {
                                let node: GreenNode = n.green().clone().into_owned();
                                new_children.push(NodeOrToken::Node(node));
                            }
                        }
                        NodeOrToken::Token(t) => {
                            let kind = t.kind();
                            if skip {
                                if matches!(
                                    kind,
                                    SyntaxKind::LineComment
                                        | SyntaxKind::HashComment
                                        | SyntaxKind::BlockComment
                                ) {
                                    continue;
                                } else if kind != SyntaxKind::Whitespace
                                    && kind != SyntaxKind::Newline
                                {
                                    skip = false;
                                    new_children.push(NodeOrToken::Token(rowan::GreenToken::new(
                                        kind.into(),
                                        t.text(),
                                    )));
                                } else {
                                    new_children.push(NodeOrToken::Token(rowan::GreenToken::new(
                                        kind.into(),
                                        t.text(),
                                    )));
                                }
                            } else {
                                new_children.push(NodeOrToken::Token(rowan::GreenToken::new(
                                    kind.into(),
                                    t.text(),
                                )));
                            }
                        }
                    }
                }

                let new_root = create_node(SyntaxKind::Root, new_children);
                Ok(Self { green: new_root })
            }
            CommentPath::Footer => {
                let red = RedNode::new_root(self.green.clone());
                let mut new_children: Vec<GreenElement> = Vec::new();

                // First collect all children
                for elem in red.children_with_tokens() {
                    match elem {
                        NodeOrToken::Node(n) => {
                            let node: GreenNode = n.green().clone().into_owned();
                            new_children.push(NodeOrToken::Node(node));
                        }
                        NodeOrToken::Token(t) => {
                            let kind = t.kind();
                            new_children.push(NodeOrToken::Token(rowan::GreenToken::new(
                                kind.into(),
                                t.text(),
                            )));
                        }
                    }
                }

                // Now remove trailing comments
                let mut i = new_children.len();
                while i > 0 {
                    i -= 1;
                    if let NodeOrToken::Token(t) = &new_children[i] {
                        let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                        if matches!(
                            kind,
                            SyntaxKind::LineComment
                                | SyntaxKind::HashComment
                                | SyntaxKind::BlockComment
                        ) {
                            new_children.remove(i);
                        } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                let new_root = create_node(SyntaxKind::Root, new_children);
                Ok(Self { green: new_root })
            }
        }
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

// =============================================================================
// Comment Helper Functions
// =============================================================================

/// 指定キーの Pair のインデックスを探す
fn find_pair_index(obj: &RedNode, key: &str) -> Option<usize> {
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

/// コメントトークンを作成
fn create_comment_token(text: &str, style: CommentStyle) -> rowan::GreenToken {
    let (kind, prefix) = match style {
        CommentStyle::Line => (SyntaxKind::LineComment, "// "),
        CommentStyle::Hash => (SyntaxKind::HashComment, "# "),
        CommentStyle::Block => (SyntaxKind::BlockComment, "/* "),
    };
    let content = format!("{}{}", prefix, text);
    create_token(kind, &content)
}

/// Pair 内の既存コメントからスタイルを決定
fn determine_comment_style(
    pair_green: &GreenNode,
    style: Option<CommentStyle>,
) -> Result<CommentStyle, HjsonEditError> {
    if let Some(s) = style {
        return Ok(s);
    }

    // 既存コメントのスタイルを探す
    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            match kind {
                SyntaxKind::LineComment => return Ok(CommentStyle::Line),
                SyntaxKind::HashComment => return Ok(CommentStyle::Hash),
                SyntaxKind::BlockComment => return Ok(CommentStyle::Block),
                _ => {}
            }
        }
    }

    // デフォルトは Line
    Ok(CommentStyle::Line)
}

/// Pair の先頭にコメントを挿入
fn insert_comment_before_pair(
    pair_green: &GreenNode,
    comment: rowan::GreenToken,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();

    // 先頭にインデント（あれば）
    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Whitespace || kind == SyntaxKind::Newline {
                children.push(NodeOrToken::Token(create_token(kind, t.text())));
                continue;
            }
        }
        break;
    }

    // コメントを追加
    children.push(NodeOrToken::Token(comment));
    children.push(NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")));

    // 残りの子要素を追加（コメントをスキップ）
    let mut found_non_trivia = false;
    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if !found_non_trivia {
                if kind == SyntaxKind::Whitespace
                    || kind == SyntaxKind::Newline
                    || matches!(
                        kind,
                        SyntaxKind::LineComment
                            | SyntaxKind::HashComment
                            | SyntaxKind::BlockComment
                    )
                {
                    continue;
                }
                found_non_trivia = true;
            }
        } else {
            found_non_trivia = true;
        }
        children.push(clone_element(child));
    }

    Ok(create_node(SyntaxKind::Pair, children))
}

/// Pair 内のコメントを削除
fn remove_comments_from_pair(pair_green: &GreenNode) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();
    let mut found_non_trivia = false;

    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if !found_non_trivia {
                if matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                ) {
                    continue;
                }
            }
            if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                found_non_trivia = true;
            }
        } else {
            found_non_trivia = true;
        }
        children.push(clone_element(child));
    }

    Ok(create_node(SyntaxKind::Pair, children))
}

/// Object 内で指定インデックスの Pair の後にインラインコメントを設定
fn set_inline_comment(
    obj_green: &GreenNode,
    pair_idx: usize,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();

    // Pair を数えて位置を特定
    let mut pair_count = 0;
    let mut pair_end_pos = None;
    let mut comma_pos = None;

    for (i, child) in children.iter().enumerate() {
        if let NodeOrToken::Node(n) = child {
            if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair {
                if pair_count == pair_idx {
                    pair_end_pos = Some(i);
                }
                pair_count += 1;
            }
        }
        if pair_count == pair_idx + 1 {
            // 次の要素を見つけたらそこまで
            if let NodeOrToken::Token(t) = child {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                if kind == SyntaxKind::Comma {
                    comma_pos = Some(i);
                    break;
                }
            }
        }
    }

    let insert_pos = comma_pos.or_else(|| pair_end_pos.map(|p| p + 1));
    if let Some(pos) = insert_pos {
        let style = style.unwrap_or(CommentStyle::Line);
        let comment = create_comment_token(text, style);
        children.insert(
            pos,
            NodeOrToken::Token(create_token(SyntaxKind::Whitespace, "  ")),
        );
        children.insert(pos + 1, NodeOrToken::Token(comment));
    }

    Ok(create_node(SyntaxKind::Object, children))
}

/// Object 内のインラインコメントを削除
fn remove_inline_comment(
    obj_green: &GreenNode,
    pair_idx: usize,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();

    // Pair を数えて位置を特定
    let mut pair_count = 0;
    let mut remove_range = None;

    for (i, child) in children.iter().enumerate() {
        if let NodeOrToken::Node(n) = child {
            if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair {
                if pair_count == pair_idx {
                    // この Pair の後のコメントを探す
                    let start = i + 1;
                    let mut end = start;
                    for j in start..children.len() {
                        match &children[j] {
                            NodeOrToken::Token(t) => {
                                let kind =
                                    crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                                if matches!(
                                    kind,
                                    SyntaxKind::LineComment
                                        | SyntaxKind::HashComment
                                        | SyntaxKind::BlockComment
                                ) {
                                    end = j + 1;
                                } else if kind != SyntaxKind::Whitespace
                                    && kind != SyntaxKind::Newline
                                {
                                    break;
                                }
                            }
                            NodeOrToken::Node(_) => break,
                        }
                    }
                    if end > start {
                        remove_range = Some((start, end));
                    }
                    break;
                }
                pair_count += 1;
            }
        }
    }

    if let Some((start, end)) = remove_range {
        children.drain(start..end);
    }

    Ok(create_node(SyntaxKind::Object, children))
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
                        i64::from_str_radix(&text[2..], 16)
                            .ok()
                            .map(HjsonValue::HexInteger)
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

fn clone_element(
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

fn is_token_of_kind(elem: &GreenElement, kind: SyntaxKind) -> bool {
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
pub enum EntryTerminator {
    /// カンマ区切り
    Comma(Vec<GreenElement>),
    /// 改行区切り
    Newline(Vec<GreenElement>),
    /// 末尾終端
    EndOfContainer,
}

/// Object の1エントリ（1キー分）
#[derive(Debug, Clone)]
pub struct ObjectEntry {
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
pub struct StandaloneTriviaBlock {
    /// 独立コメント本体と、その直前の改行・インデントも含む
    pub elements: Vec<GreenElement>,
}

/// Object の要素（エントリまたは独立トリビア）
#[derive(Debug, Clone)]
pub enum ObjectItem {
    Entry(ObjectEntry),
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
fn build_object_items(children: &[GreenElement]) -> Vec<ObjectItem> {
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
                    leading_comments: leading_line_trivia.clone(),
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
            parse_dot_path("server.host"),
            vec![
                PathSegment::Key("server".to_string()),
                PathSegment::Key("host".to_string())
            ]
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
