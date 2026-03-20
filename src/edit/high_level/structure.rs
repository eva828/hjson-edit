use crate::edit::high_level::is_token_of_kind;
use crate::syntax::GreenElement;
use crate::syntax::{HjsonLanguage, SyntaxKind};
use rowan::{Language, NodeOrToken};
use std::mem;

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

pub(crate) struct LayoutTrivia;

impl LayoutTrivia {
    pub(crate) fn whitespace_after_last_newline(trivia: &[GreenElement]) -> String {
        let mut text = String::new();
        let mut after_newline = false;

        for elem in trivia {
            if let NodeOrToken::Token(token) = elem {
                let kind = HjsonLanguage::kind_from_raw(token.kind());
                if kind == SyntaxKind::Newline {
                    text.clear();
                    after_newline = true;
                } else if after_newline && kind == SyntaxKind::Whitespace {
                    text.push_str(token.text());
                }
            }
        }

        text
    }

    pub(crate) fn indentation_from_standalone_block(block: &StandaloneTriviaBlock) -> String {
        let mut after_newline = false;
        let mut current_indent = String::new();
        let mut last_comment_indent = String::new();

        for elem in &block.elements {
            if let NodeOrToken::Token(token) = elem {
                let kind = HjsonLanguage::kind_from_raw(token.kind());
                match kind {
                    SyntaxKind::Newline => {
                        after_newline = true;
                        current_indent.clear();
                    }
                    SyntaxKind::Whitespace if after_newline => {
                        current_indent.push_str(token.text());
                    }
                    SyntaxKind::LineComment
                    | SyntaxKind::HashComment
                    | SyntaxKind::BlockComment
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
            Self::whitespace_after_last_newline(&block.elements)
        } else {
            last_comment_indent
        }
    }

    fn has_blank_line(trivia: &[GreenElement]) -> bool {
        let mut had_newline = false;

        for elem in trivia {
            if let NodeOrToken::Token(token) = elem {
                let kind = HjsonLanguage::kind_from_raw(token.kind());
                if kind == SyntaxKind::Newline {
                    if had_newline {
                        return true;
                    }
                    had_newline = true;
                } else if kind != SyntaxKind::Whitespace {
                    had_newline = false;
                }
            }
        }

        false
    }

    fn find_blank_line_split(trivia: &[GreenElement]) -> usize {
        let mut had_newline = false;

        for (index, elem) in trivia.iter().enumerate() {
            if let NodeOrToken::Token(token) = elem {
                let kind = HjsonLanguage::kind_from_raw(token.kind());
                if kind == SyntaxKind::Newline {
                    if had_newline {
                        return index;
                    }
                    had_newline = true;
                } else if kind != SyntaxKind::Whitespace {
                    had_newline = false;
                }
            }
        }

        trivia.len()
    }
}

struct ObjectLayoutScanner<'a> {
    children: &'a [GreenElement],
    index: usize,
    pending_trivia: Vec<GreenElement>,
    items: Vec<ObjectItem>,
}

impl<'a> ObjectLayoutScanner<'a> {
    fn new(children: &'a [GreenElement]) -> Self {
        Self {
            children,
            index: 0,
            pending_trivia: Vec::new(),
            items: Vec::new(),
        }
    }

    fn scan(mut self) -> Vec<ObjectItem> {
        while let Some(elem) = self.children.get(self.index) {
            match elem {
                NodeOrToken::Node(node) if node.kind() == SyntaxKind::Pair.into() => {
                    let pair = elem.clone();
                    self.index += 1;
                    let leading_line_trivia = mem::take(&mut self.pending_trivia);
                    let trailing_inline_trivia = self.collect_trailing_inline_trivia();
                    let terminator = self.collect_object_terminator();
                    self.items.push(ObjectItem::Entry(ObjectEntry {
                        leading_line_trivia,
                        leading_comments: Vec::new(),
                        pair,
                        trailing_inline_trivia,
                        terminator,
                    }));
                }
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if kind == SyntaxKind::RBrace {
                        break;
                    }
                    if kind.is_trivia() {
                        self.pending_trivia.push(elem.clone());
                        self.move_standalone_trivia_if_needed();
                    }
                    self.index += 1;
                }
                _ => self.index += 1,
            }
        }

        self.flush_pending_trivia();
        self.items
    }

    fn collect_trailing_inline_trivia(&mut self) -> Vec<GreenElement> {
        let mut trailing_inline_trivia = Vec::new();
        while let Some(elem) = self.children.get(self.index) {
            match elem {
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if kind == SyntaxKind::Whitespace || kind.is_comment() {
                        trailing_inline_trivia.push(elem.clone());
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        trailing_inline_trivia
    }

    fn collect_object_terminator(&mut self) -> EntryTerminator {
        let Some(elem) = self.children.get(self.index) else {
            return EntryTerminator::EndOfContainer;
        };

        match elem {
            NodeOrToken::Token(token) => match HjsonLanguage::kind_from_raw(token.kind()) {
                SyntaxKind::Comma => EntryTerminator::Comma(self.collect_comma_suffix()),
                SyntaxKind::Newline => EntryTerminator::Newline(self.collect_newline_suffix()),
                _ => EntryTerminator::EndOfContainer,
            },
            _ => EntryTerminator::EndOfContainer,
        }
    }

    fn collect_comma_suffix(&mut self) -> Vec<GreenElement> {
        let mut suffix = vec![self.children[self.index].clone()];
        self.index += 1;

        while let Some(elem) = self.children.get(self.index) {
            match elem {
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if matches!(kind, SyntaxKind::Newline | SyntaxKind::Whitespace) {
                        suffix.push(elem.clone());
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        suffix
    }

    fn collect_newline_suffix(&mut self) -> Vec<GreenElement> {
        let mut suffix = vec![self.children[self.index].clone()];
        self.index += 1;

        if let Some(NodeOrToken::Token(token)) = self.children.get(self.index) {
            if HjsonLanguage::kind_from_raw(token.kind()) == SyntaxKind::Whitespace {
                suffix.push(self.children[self.index].clone());
                self.index += 1;
            }
        }

        suffix
    }

    fn move_standalone_trivia_if_needed(&mut self) {
        if !LayoutTrivia::has_blank_line(&self.pending_trivia) {
            return;
        }

        let split_idx = LayoutTrivia::find_blank_line_split(&self.pending_trivia);
        if split_idx > 0 {
            self.items
                .push(ObjectItem::Standalone(StandaloneTriviaBlock {
                    elements: self.pending_trivia.drain(0..split_idx).collect(),
                }));
        }
    }

    fn flush_pending_trivia(&mut self) {
        if !self.pending_trivia.is_empty() {
            self.items
                .push(ObjectItem::Standalone(StandaloneTriviaBlock {
                    elements: mem::take(&mut self.pending_trivia),
                }));
        }
    }
}

struct ArrayLayoutScanner<'a> {
    children: &'a [GreenElement],
    index: usize,
    pending_trivia: Vec<GreenElement>,
    items: Vec<ArrayItem>,
}

impl<'a> ArrayLayoutScanner<'a> {
    fn new(children: &'a [GreenElement]) -> Self {
        Self {
            children,
            index: 0,
            pending_trivia: Vec::new(),
            items: Vec::new(),
        }
    }

    fn scan(mut self) -> Vec<ArrayItem> {
        while let Some(elem) = self.children.get(self.index) {
            match elem {
                NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value.into() => {
                    let value = elem.clone();
                    self.index += 1;
                    let leading_line_trivia = mem::take(&mut self.pending_trivia);
                    let trailing_inline_trivia = self.collect_trailing_inline_trivia();
                    let terminator = self.collect_array_terminator();
                    self.items.push(ArrayItem::Entry(ArrayEntry {
                        leading_line_trivia,
                        value,
                        trailing_inline_trivia,
                        terminator,
                    }));
                }
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if kind == SyntaxKind::RBracket {
                        break;
                    }
                    if kind.is_trivia() {
                        self.pending_trivia.push(elem.clone());
                        self.move_standalone_trivia_if_needed();
                    }
                    self.index += 1;
                }
                _ => self.index += 1,
            }
        }

        self.flush_pending_trivia();
        self.items
    }

    fn collect_trailing_inline_trivia(&mut self) -> Vec<GreenElement> {
        let mut trailing_inline_trivia = Vec::new();
        while let Some(elem) = self.children.get(self.index) {
            match elem {
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if kind == SyntaxKind::Whitespace || kind.is_comment() {
                        trailing_inline_trivia.push(elem.clone());
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        trailing_inline_trivia
    }

    fn collect_array_terminator(&mut self) -> EntryTerminator {
        let Some(elem) = self.children.get(self.index) else {
            return EntryTerminator::EndOfContainer;
        };

        match elem {
            NodeOrToken::Token(token) => match HjsonLanguage::kind_from_raw(token.kind()) {
                SyntaxKind::Comma => EntryTerminator::Comma(self.collect_comma_suffix()),
                SyntaxKind::Newline => EntryTerminator::Newline(self.collect_newline_suffix()),
                _ => EntryTerminator::EndOfContainer,
            },
            _ => EntryTerminator::EndOfContainer,
        }
    }

    fn collect_comma_suffix(&mut self) -> Vec<GreenElement> {
        let mut suffix = vec![self.children[self.index].clone()];
        self.index += 1;

        while let Some(elem) = self.children.get(self.index) {
            match elem {
                NodeOrToken::Token(token) => {
                    let kind = HjsonLanguage::kind_from_raw(token.kind());
                    if matches!(kind, SyntaxKind::Newline | SyntaxKind::Whitespace) {
                        suffix.push(elem.clone());
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        suffix
    }

    fn collect_newline_suffix(&mut self) -> Vec<GreenElement> {
        let suffix = vec![self.children[self.index].clone()];
        self.index += 1;

        if let Some(NodeOrToken::Token(token)) = self.children.get(self.index) {
            if HjsonLanguage::kind_from_raw(token.kind()) == SyntaxKind::Whitespace {
                self.pending_trivia.push(self.children[self.index].clone());
                self.index += 1;
            }
        }

        suffix
    }

    fn move_standalone_trivia_if_needed(&mut self) {
        if !LayoutTrivia::has_blank_line(&self.pending_trivia) {
            return;
        }

        let split_idx = LayoutTrivia::find_blank_line_split(&self.pending_trivia);
        if split_idx > 0 {
            self.items
                .push(ArrayItem::Standalone(StandaloneTriviaBlock {
                    elements: self.pending_trivia.drain(0..split_idx).collect(),
                }));
        }
    }

    fn flush_pending_trivia(&mut self) {
        if !self.pending_trivia.is_empty() {
            self.items
                .push(ArrayItem::Standalone(StandaloneTriviaBlock {
                    elements: mem::take(&mut self.pending_trivia),
                }));
        }
    }
}

struct ObjectChildrenBuilder;

impl ObjectChildrenBuilder {
    fn rebuild(
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
                    Self::remove_orphan_indent_before_entry(&mut new_children);
                    new_children.extend(entry.leading_line_trivia);
                    new_children.extend(entry.leading_comments);
                    new_children.push(entry.pair);
                    new_children.extend(entry.trailing_inline_trivia);
                    match entry.terminator {
                        EntryTerminator::Comma(suffix) | EntryTerminator::Newline(suffix) => {
                            new_children.extend(suffix)
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
}

struct ArrayChildrenBuilder;

impl ArrayChildrenBuilder {
    fn rebuild(
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
                            new_children.extend(suffix)
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
}

/// Object の children から ObjectItem のリストを構築
/// 空行判定は \n\s*\n を検出
pub(crate) fn build_object_items(children: &[GreenElement]) -> Vec<ObjectItem> {
    ObjectLayoutScanner::new(children).scan()
}

/// Array の children から ArrayItem のリストを構築
pub(crate) fn build_array_items(children: &[GreenElement]) -> Vec<ArrayItem> {
    ArrayLayoutScanner::new(children).scan()
}

pub(crate) fn rebuild_object_children_for_format(
    lbrace: Option<GreenElement>,
    items: Vec<ObjectItem>,
    rbrace: Option<GreenElement>,
) -> Vec<GreenElement> {
    ObjectChildrenBuilder::rebuild(lbrace, items, rbrace)
}

pub(crate) fn rebuild_array_children_for_format(
    lbracket: Option<GreenElement>,
    items: Vec<ArrayItem>,
    rbracket: Option<GreenElement>,
) -> Vec<GreenElement> {
    ArrayChildrenBuilder::rebuild(lbracket, items, rbracket)
}

pub(crate) fn whitespace_after_last_newline(trivia: &[GreenElement]) -> String {
    LayoutTrivia::whitespace_after_last_newline(trivia)
}

pub(crate) fn indentation_from_standalone_block(block: &StandaloneTriviaBlock) -> String {
    LayoutTrivia::indentation_from_standalone_block(block)
}
