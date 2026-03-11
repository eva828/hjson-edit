use crate::edit::high_level::is_token_of_kind;
use crate::syntax::green::GreenElement;
use crate::syntax::kind::{HjsonLanguage, SyntaxKind};
use rowan::{Language, NodeOrToken};

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
                let pair = elem.clone();
                let leading_line_trivia = pending_trivia.clone();
                pending_trivia.clear();

                let mut trailing_inline_trivia = Vec::new();
                i += 1;
                while i < children.len() {
                    match &children[i] {
                        NodeOrToken::Token(t) => {
                            let kind = HjsonLanguage::kind_from_raw(t.kind());
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
                            let kind = HjsonLanguage::kind_from_raw(t.kind());
                            if kind == SyntaxKind::Comma {
                                let comma = children[i].clone();
                                i += 1;
                                let mut suffix = vec![comma];
                                while i < children.len() {
                                    match &children[i] {
                                        NodeOrToken::Token(t) => {
                                            let kind = HjsonLanguage::kind_from_raw(t.kind());
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
                                let mut suffix = vec![newline];
                                if i < children.len() {
                                    if let NodeOrToken::Token(t) = &children[i] {
                                        let kind = HjsonLanguage::kind_from_raw(t.kind());
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

                let entry = ObjectEntry {
                    leading_line_trivia,
                    leading_comments: Vec::new(),
                    pair,
                    trailing_inline_trivia,
                    terminator,
                };
                items.push(ObjectItem::Entry(entry));
            }
            NodeOrToken::Token(t) => {
                let kind = HjsonLanguage::kind_from_raw(t.kind());

                if kind == SyntaxKind::RBrace {
                    break;
                } else if is_trivia(kind) {
                    pending_trivia.push(elem.clone());

                    if has_blank_line(&pending_trivia) {
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
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

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
                            let kind = HjsonLanguage::kind_from_raw(t.kind());
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
                            let kind = HjsonLanguage::kind_from_raw(t.kind());
                            if kind == SyntaxKind::Comma {
                                let comma = children[i].clone();
                                i += 1;
                                let mut suffix = vec![comma];
                                while i < children.len() {
                                    match &children[i] {
                                        NodeOrToken::Token(t) => {
                                            let kind = HjsonLanguage::kind_from_raw(t.kind());
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
                                        let kind = HjsonLanguage::kind_from_raw(t.kind());
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
                let kind = HjsonLanguage::kind_from_raw(t.kind());
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

pub(crate) fn whitespace_after_last_newline(trivia: &[GreenElement]) -> String {
    let mut text = String::new();
    let mut after_newline = false;

    for elem in trivia {
        if let NodeOrToken::Token(t) = elem {
            let kind = HjsonLanguage::kind_from_raw(t.kind());
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

pub(crate) fn indentation_from_standalone_block(block: &StandaloneTriviaBlock) -> String {
    let mut after_newline = false;
    let mut current_indent = String::new();
    let mut last_comment_indent = String::new();

    for elem in &block.elements {
        if let NodeOrToken::Token(t) = elem {
            let kind = HjsonLanguage::kind_from_raw(t.kind());
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

fn has_blank_line(trivia: &[GreenElement]) -> bool {
    let mut had_newline = false;

    for elem in trivia {
        if let NodeOrToken::Token(t) = elem {
            let kind = HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Newline {
                if had_newline {
                    return true;
                }
                had_newline = true;
            } else if kind == SyntaxKind::Whitespace {
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
            let kind = HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Newline {
                if had_newline {
                    return i;
                }
                had_newline = true;
            } else if kind == SyntaxKind::Whitespace {
            } else { is_comment(kind); had_newline = false; }
        }
    }

    trivia.len()
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
