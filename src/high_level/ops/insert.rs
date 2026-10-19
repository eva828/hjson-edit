//! コンテナへの要素挿入を 1 か所に集約する。
//!
//! HJSON では行末でしか終端できない値（引用符なし文字列など）があり、その直後に
//! カンマや閉じ区切りを置くとそれらが値に吸収されて文書が壊れる。この性質を扱うのは
//! このモジュールだけとし、`set` / `merge` / コメント操作はここが返す挿入位置と
//! 要素列をそのまま使う。
//!
//! 挿入位置の決定は次の規則に従う。
//!
//! 1. 直前の要素の行が改行で終わっているときは、その改行の直後（次の行頭）へ入れる。
//!    既存の改行が挿入要素の行を終端するため、足す改行は要素の後ろだけ。
//! 2. 直前の要素と同じ行に入れる場合、要素が行末でしか終端できないなら前後を改行で囲む。
//!    そうでなければ区切り（カンマ）を補う。
//! 3. 本文が空のときは閉じ区切りの直前（ブレースレスは本文末尾）へ入れ、
//!    閉じ区切りが独立した行にあるなら要素の後ろに改行を足す。
//! 4. 閉じ区切りの直前にあるコメント（footer コメント）は要素の後ろに残す。

use std::ops::Range;

use rowan::NodeOrToken;

use crate::ast::{
    Array as AstArray, AstNode, BodyInsertion, DelimiterStyle, Object as AstObject, SlotSpan,
};
use crate::high_level::error::HjsonEditError;
use crate::high_level::path::HjsonObjectPath;
use crate::syntax::{GreenElement, NewlineKind, RedNode, SyntaxKind};

/// レンダリング済みの要素（メンバーまたは配列要素）。
///
/// コメントを伴う要素（マージでコメントごとコピーする場合）もあるため、
/// 要素は複数の green 要素の並びとして持つ。
pub(crate) struct RenderedElement {
    elements: Vec<GreenElement>,
    line_terminated: bool,
    has_leading_comments: bool,
}

impl RenderedElement {
    /// 単一の green 要素から作る。
    pub(crate) fn single(green: GreenElement) -> Self {
        let line_terminated = ends_with_line_token(&green);
        Self {
            elements: vec![green],
            line_terminated,
            has_leading_comments: false,
        }
    }

    /// 並び済みの green 要素から作る（先頭コメントの有無を呼び出し側が指定する）。
    pub(crate) fn from_parts(elements: Vec<GreenElement>, has_leading_comments: bool) -> Self {
        let line_terminated = elements.last().is_some_and(ends_with_line_token);
        Self {
            elements,
            line_terminated,
            has_leading_comments,
        }
    }
}

/// 要素を挿入する位置。
pub(crate) enum InsertPosition {
    /// index の要素の直前。
    Before(usize),
    /// コンテナの末尾（閉じ区切りの直前にあるコメントの手前）。
    End,
}

/// 要素を挿入する位置と要素列を返す。
///
/// `newline` はコンテナに改行が 1 つも無い場合に使う既定の改行コード。
pub(crate) fn plan_insert(
    container: &RedNode,
    position: InsertPosition,
    element: RenderedElement,
    newline: NewlineKind,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let slots = container_slots(container)?;
    let braceless = is_braceless(container)?;
    // 混在した改行コードの文書でも、コンテナ内で使われている改行に合わせる
    let newline = container_newline(container).unwrap_or(newline);

    match position {
        InsertPosition::End => plan_append(container, &slots, element, newline, braceless),
        InsertPosition::Before(index) => {
            plan_before(container, &slots, index, element, newline, braceless)
        }
    }
}

/// コンテナ直下で最初に現れる改行の種類。
fn container_newline(container: &RedNode) -> Option<NewlineKind> {
    container
        .children_with_tokens()
        .find_map(|child| match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                NewlineKind::from_text(token.text())
            }
            _ => None,
        })
}

/// 指定 index の子要素の改行の種類。
fn newline_at(container: &RedNode, index: usize) -> Option<NewlineKind> {
    container
        .children_with_tokens()
        .nth(index)
        .and_then(|child| match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                NewlineKind::from_text(token.text())
            }
            _ => None,
        })
}

/// 要素本体だけを差し替える位置と要素を返す。
pub(crate) fn plan_replace(
    container: &RedNode,
    index: usize,
    element: RenderedElement,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let slot = slot_at(container, index)?;
    Ok((slot.elem().start, element.elements))
}

/// 要素をトリビアごと削除する範囲を返す。
pub(crate) fn plan_remove(
    container: &RedNode,
    index: usize,
) -> Result<Range<usize>, HjsonEditError> {
    Ok(slot_at(container, index)?.span())
}

/// コンテナの要素位置を返す。
pub(crate) fn container_slots(container: &RedNode) -> Result<Vec<SlotSpan>, HjsonEditError> {
    match container.kind() {
        SyntaxKind::Object => Ok(AstObject::cast(container.clone())?.slots()),
        SyntaxKind::Array => Ok(AstArray::cast(container.clone())?.slots()),
        found => Err(HjsonEditError::UnexpectedStructure {
            expected: SyntaxKind::Object,
            found,
        }),
    }
}

/// コンテナ内の要素位置を 1 つ返す。
pub(crate) fn slot_at(container: &RedNode, index: usize) -> Result<SlotSpan, HjsonEditError> {
    let slots = container_slots(container)?;
    let len = slots.len();
    slots
        .get(index)
        .cloned()
        .ok_or_else(|| HjsonEditError::InvalidTarget {
            path: HjsonObjectPath::from_segments(Vec::new()),
            reason: format!("element index {index} is out of bounds (length {len})"),
        })
}

/// 末尾へ要素を追加する。
fn plan_append(
    container: &RedNode,
    slots: &[SlotSpan],
    element: RenderedElement,
    newline: NewlineKind,
    braceless: bool,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let RenderedElement {
        elements,
        line_terminated,
        has_leading_comments,
    } = element;

    let Some(last) = slots.last() else {
        return plan_append_to_empty_body(
            container,
            RenderedElement {
                elements,
                line_terminated,
                has_leading_comments,
            },
            newline,
            braceless,
        );
    };

    // 直前の要素の行が改行で終わっているなら、その次の行頭へ入れる。
    // 既存の改行が挿入要素の行を終端するため、足すのは要素の後ろの改行だけ。
    if let Some(line_break) = last.line_break() {
        let kind = newline_at(container, line_break).unwrap_or(newline);
        let mut inserted = elements;
        inserted.push(newline_element(kind));
        return Ok((line_break + 1, inserted));
    }

    let insert_index = last.trail().end;

    // 先頭にコメント行を持つ要素は行の途中に置けないため、前後を改行で囲む。
    if has_leading_comments {
        return Ok((insert_index, surrounded_by_newlines(elements, newline)));
    }

    if braceless {
        // ブレースレスの本文は行単位なので、前に改行を入れて独立した行にする。
        let mut inserted = vec![newline_element(newline)];
        inserted.extend(elements);
        return Ok((insert_index, inserted));
    }

    // 直前の要素と同じ行に入れる。
    let mut inserted = Vec::new();
    if !has_separator_between(container, last.elem().end, insert_index) {
        inserted.extend(separator_elements());
    }
    inserted.extend(elements);
    if line_terminated {
        inserted.push(newline_element(newline));
    }

    Ok((insert_index, inserted))
}

/// 本文が空のコンテナへ要素を追加する。
fn plan_append_to_empty_body(
    container: &RedNode,
    element: RenderedElement,
    newline: NewlineKind,
    braceless: bool,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let RenderedElement {
        elements,
        line_terminated,
        has_leading_comments,
    } = element;

    let insertion = body_insertion(container)?;
    let index = insertion.index();

    let mut inserted = Vec::new();
    if insertion.start_new_line() {
        inserted.push(newline_element(newline));
    }
    inserted.extend(elements);

    if !braceless && (insertion.terminate_line() || line_terminated || has_leading_comments) {
        inserted.push(newline_element(newline));
    }

    Ok((index, inserted))
}

/// 既存要素の直前へ挿入する。
fn plan_before(
    container: &RedNode,
    slots: &[SlotSpan],
    index: usize,
    element: RenderedElement,
    newline: NewlineKind,
    braceless: bool,
) -> Result<(usize, Vec<GreenElement>), HjsonEditError> {
    let target = slots
        .get(index)
        .ok_or_else(|| HjsonEditError::InvalidTarget {
            path: HjsonObjectPath::from_segments(Vec::new()),
            reason: format!(
                "element index {index} is out of bounds (length {})",
                slots.len()
            ),
        })?
        .clone();

    let RenderedElement {
        elements,
        line_terminated,
        has_leading_comments,
    } = element;

    let insert_index = target.insert_before();

    if braceless || target.at_line_start() {
        // 対象は行頭から始まる。挿入要素をその 1 行前に入れ、対象を次の行に残す。
        let mut inserted = elements;
        inserted.push(newline_element(newline));
        return Ok((insert_index, inserted));
    }

    if has_leading_comments {
        // 先頭にコメント行を持つ要素は行の途中に置けないため、前後を改行で囲む。
        return Ok((insert_index, surrounded_by_newlines(elements, newline)));
    }

    // 同一行への挿入。直前の要素と区切られていなければカンマを補う。
    let mut inserted = Vec::new();
    if index > 0 {
        let previous = &slots[index - 1];
        if !has_separator_between(container, previous.elem().end, insert_index) {
            inserted.extend(separator_elements());
        }
    }
    inserted.extend(elements);
    if line_terminated {
        inserted.push(newline_element(newline));
    } else {
        inserted.push(comma_element());
    }

    Ok((insert_index, inserted))
}

/// 要素列の前後を改行で囲む。
fn surrounded_by_newlines(
    mut elements: Vec<GreenElement>,
    newline: NewlineKind,
) -> Vec<GreenElement> {
    let mut surrounded = vec![newline_element(newline)];
    surrounded.append(&mut elements);
    surrounded.push(newline_element(newline));
    surrounded
}

/// コンテナが閉じ区切りを持たない（ブレースレス）か。
fn is_braceless(container: &RedNode) -> Result<bool, HjsonEditError> {
    let style = match container.kind() {
        SyntaxKind::Object => AstObject::cast(container.clone())?.delimiter_style,
        SyntaxKind::Array => AstArray::cast(container.clone())?.delimiter_style,
        found => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found,
            })
        }
    };
    Ok(style == DelimiterStyle::Braceless)
}

/// 本文が空のコンテナへ最初の要素を挿入する位置。
fn body_insertion(container: &RedNode) -> Result<BodyInsertion, HjsonEditError> {
    match container.kind() {
        SyntaxKind::Object => Ok(AstObject::cast(container.clone())?.body_insertion()),
        SyntaxKind::Array => Ok(AstArray::cast(container.clone())?.body_insertion()),
        found => Err(HjsonEditError::UnexpectedStructure {
            expected: SyntaxKind::Object,
            found,
        }),
    }
}

/// `from..to` の子要素に区切り（カンマまたは改行）があるか。
fn has_separator_between(container: &RedNode, from: usize, to: usize) -> bool {
    container
        .children_with_tokens()
        .skip(from)
        .take(to.saturating_sub(from))
        .any(|child| {
            matches!(
                child,
                NodeOrToken::Token(token)
                    if matches!(token.kind(), SyntaxKind::Comma | SyntaxKind::Newline)
            )
        })
}

/// 要素の末尾トークンが行末でしか終端できないか。
///
/// 引用符なし文字列と行コメントは行末までを飲み込むため、直後にトークンを置けない。
fn ends_with_line_token(element: &GreenElement) -> bool {
    let red = match element {
        NodeOrToken::Token(token) => RedNode::new_root(rowan::GreenNode::new(
            SyntaxKind::Value.into(),
            vec![token.clone().into()],
        )),
        NodeOrToken::Node(node) => RedNode::new_root(node.clone()),
    };

    red.last_token().is_some_and(|token| {
        matches!(
            token.kind(),
            SyntaxKind::UnquotedString | SyntaxKind::LineComment | SyntaxKind::HashComment
        )
    })
}

pub(crate) fn newline_element(kind: NewlineKind) -> GreenElement {
    SyntaxKind::Newline.create_token(kind.as_str()).into()
}

/// 要素の区切り（カンマと空白）。
fn separator_elements() -> Vec<GreenElement> {
    vec![
        comma_element(),
        SyntaxKind::Whitespace
            .create_token(SyntaxKind::Whitespace.canonical_text())
            .into(),
    ]
}

fn comma_element() -> GreenElement {
    SyntaxKind::Comma
        .create_token(SyntaxKind::Comma.canonical_text())
        .into()
}
