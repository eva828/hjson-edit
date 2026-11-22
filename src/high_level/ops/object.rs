use std::ops::Range;

use rowan::{GreenNode, NodeOrToken};

use crate::{
    ast::{
        detected_string_style_of_node, Array as AstArray, AstNode, Key, Member,
        Object as AstObject, QuoteStyle, Trivia,
    },
    high_level::value_ops::{build_nested_value, values_equal},
    high_level::{
        error::HjsonEditError,
        ops::{
            container_depth_of_value, ensure_depth_within_limit,
            insert::{plan_insert, plan_remove, InsertPosition, RenderedElement},
            receiving_container_depth,
        },
        path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
        resolve::{array_slot_index, resolve_body, ExistingPrefix, ParentResolution, PathResolver},
        value::{HjsonValue, RenderConfig},
        HjsonDocument, Prefer,
    },
    syntax::{NewlineKind, RedNode, SyntaxKind},
    GreenElement,
};

/// パスで指した値の取得・設定・削除。
///
/// # パスの意味
///
/// | 記法 | 取得 | 設定 |
/// |---|---|---|
/// | `key` | メンバーの値 | 置き換え（無ければ追加） |
/// | `[n]` | 要素 `n` | 要素 `n` の置き換え（範囲外は [`HjsonEditError::ArrayIndexOutOfBounds`]） |
/// | `[+n]` | 要素 `n` | `n` の位置へ挿入（`0 <= n <= 長さ`） |
/// | `[-]` / `[+]` | 末尾の要素 | 末尾へ追加 |
///
/// 空のパスは文書全体（設定は置き換え、取得は文書の値）を指す。
/// 設定で中間のコンテナが無い場合は作成し、途中がスカラーなら
/// [`HjsonEditError::InvalidTarget`] を返す。同じキーのメンバーが複数ある場合は、
/// 値の層（後勝ち）に合わせてすべてを同じ値にする（取得は最後の出現）。
///
/// # 編集の規則
///
/// テキストへの変更は最小限にする。新しい要素は末尾の要素の後ろ、閉じ区切りの直前の
/// コメント（`:footer`）の手前に入り、字下げは既存の要素に揃える（閉じ区切りが独立した行に
/// ある場合、空のコンテナではコンテナの行の字下げ + 1 段）。行末でしか終端できない値
/// （引用符なし文字列など）を挿入するときは改行を補い、常に再パースできるテキストを返す。
///
/// 同じ値の設定では値ノードを差し替えない（`1.0` や `{x: 1, y: 2}` の表記は残る）。
/// ただし [`Prefer::Preferred`] / [`Prefer::Explicit`] を指定したときは、同じ値でも値ノードを
/// 作り直す（数値は正規化され、文字列は指定のスタイルで書かれる）。差し替える値の中身の
/// 体裁は、値と既存のスタイルから描き直す。
///
/// 編集結果は既定で検証する（[`HjsonDocument::verify_edits`]）。
pub trait HjsonDocumentEdit {
    /// パスが指す値を返す。
    fn get(&self, path: HjsonObjectPath) -> Result<HjsonValue, HjsonEditError>;

    /// パスが指す値を設定した文書を返す。
    fn set(
        self,
        path: HjsonObjectPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;

    /// パスが指す要素を削除した文書を返す。
    fn remove(self, path: HjsonObjectPath) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentEdit for HjsonDocument {
    fn get(&self, path: HjsonObjectPath) -> Result<HjsonValue, HjsonEditError> {
        let resolved = PathResolver::new(self.syntax_root().clone()).resolve_node(&path)?;
        HjsonValue::from_node(&resolved)
    }

    fn set(
        self,
        path: HjsonObjectPath,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError> {
        let value = value.into();

        let root = self.syntax_root().clone();
        let string_style = self.current_string_style();
        let verify = self.current_verify();
        let touched = receiving_container(&root, &path);

        let newline = self.current_newline_kind();
        let updated = if path.depth() == 0 {
            replace_root_value(&root, &value, string_style, newline)?
        } else {
            rebuild_tree_with_set(root.clone(), &path, &value, string_style, newline)?
        };
        let document = self.root(updated).note_touched(touched);

        if verify {
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

    fn remove(self, path: HjsonObjectPath) -> Result<Self, HjsonEditError> {
        let root = self.syntax_root().clone();
        let verify = self.current_verify();
        let touched = path.split_last().map(|(parent, _)| parent);

        let updated = rebuild_tree_with_remove(root.clone(), &path)?;
        let document = match touched {
            Some(container) => self.root(updated).note_touched(container),
            None => self.root(updated),
        };

        if verify {
            crate::high_level::verify::value_edit(
                &root,
                document.syntax_root(),
                &path,
                |previous| crate::high_level::value_ops::remove_path(previous, &path),
                crate::high_level::verify::EditedRegion::Slot,
            )?;
        }

        Ok(document)
    }
}

pub(crate) fn replace_root_value(
    root: &RedNode,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    ensure_depth_within_limit(container_depth_of_value(value))?;

    let child = resolve_body(root)?;
    let config = RenderConfig {
        string_style: None,
        newline,
    };
    let updated_child = render_value_like_node(&child, value, string_style, &config)?;

    Ok(root
        .green()
        .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
}

pub(crate) fn rebuild_tree_with_set(
    root: RedNode,
    path: &HjsonObjectPath,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    rebuild_tree_with_set_inner(root, path, value, string_style, newline)
        .map_err(|error| retarget(error, path))
}

fn rebuild_tree_with_set_inner(
    root: RedNode,
    path: &HjsonObjectPath,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    let resolver = PathResolver::new(root.clone());
    match resolver.resolve_parent(path) {
        Ok(ParentResolution { parent, last }) => {
            ensure_depth_within_limit(
                receiving_container_depth(&parent) + container_depth_of_value(value),
            )?;

            let new_parent = apply_set_to_node(&parent, &last, value, string_style, newline)?;

            if parent.kind() == SyntaxKind::Root {
                Ok(new_parent)
            } else {
                Ok(parent.replace_with(new_parent))
            }
        }
        Err(HjsonEditError::PathNotFound(_)) => {
            let ExistingPrefix { ancestor, consumed } = resolver.resolve_existing_prefix(path);

            let missing = &path.segments()[consumed];

            // 深さの検査は値の構築前に行う。深い値を作ってから捨てると、
            // HjsonValue の再帰的な Drop でスタックを使い切るため。
            // `missing` は受け取り先のコンテナ内のキーなので、新しく増える
            // コンテナは残りのセグメント分だけ。
            let created = path.depth() - consumed - 1;
            let carried = container_depth_of_value(value);
            ensure_depth_within_limit(receiving_container_depth(&ancestor) + created + carried)?;

            let nested_value = build_nested_value(&path.segments()[consumed + 1..], value.clone())?;
            let new_parent =
                apply_set_to_node(&ancestor, missing, &nested_value, string_style, newline)?;

            if ancestor.kind() == SyntaxKind::Root {
                Ok(new_parent)
            } else {
                Ok(ancestor.replace_with(new_parent))
            }
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn rebuild_tree_with_remove(
    root: RedNode,
    path: &HjsonObjectPath,
) -> Result<GreenNode, HjsonEditError> {
    remove_with_retarget(root, path).map_err(|error| retarget(error, path))
}

fn remove_with_retarget(
    root: RedNode,
    path: &HjsonObjectPath,
) -> Result<GreenNode, HjsonEditError> {
    let ParentResolution { parent, last } = PathResolver::new(root.clone()).resolve_parent(path)?;
    let new_parent = apply_remove_to_node(&parent, &last)?;

    Ok(parent.replace_with(new_parent))
}

/// Root / Value を降りて、実際に要素を書き換えるコンテナへ適用する。
fn transform_node(
    node: &RedNode,
    last: &PathSegment,
    on_object: impl FnOnce(&RedNode, &PathSegment) -> Result<GreenNode, HjsonEditError>,
    on_array: impl FnOnce(&RedNode, &PathSegment) -> Result<GreenNode, HjsonEditError>,
) -> Result<GreenNode, HjsonEditError> {
    match node.kind() {
        SyntaxKind::Root | SyntaxKind::Value => {
            let child = resolve_body(node).map_err(|_| HjsonEditError::InvalidTarget {
                path: HjsonObjectPath::from_segments(vec![last.clone()]),
                reason: "the path points into a scalar value".to_string(),
            })?;
            let updated_child = transform_node(&child, last, on_object, on_array)?;
            Ok(node
                .green()
                .splice_children(child.index()..child.index() + 1, vec![updated_child.into()]))
        }
        SyntaxKind::Object => on_object(node, last),
        SyntaxKind::Array => on_array(node, last),
        found => Err(HjsonEditError::UnexpectedStructure {
            expected: SyntaxKind::Object,
            found,
        }),
    }
}

fn apply_set_to_node(
    node: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    transform_node(
        node,
        last,
        |node, last| set_object_child(node, last, value, string_style, newline),
        |node, last| set_array_child(node, last, value, string_style, newline),
    )
}

fn apply_remove_to_node(node: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    transform_node(node, last, remove_object_child, remove_array_child)
}

/// メンバーのキーが `key` と一致するか。
///
/// 大半のキーは引用符なしなので、生テキストの比較で済ませる（デコードしない）。
pub(crate) fn member_key_matches(member: &RedNode, key: &str) -> bool {
    let Some(key_node) = member
        .children()
        .find(|child| child.kind() == SyntaxKind::Key)
    else {
        return false;
    };
    let token = key_node
        .children_with_tokens()
        .find_map(|element| match element {
            NodeOrToken::Token(token)
                if matches!(
                    token.kind(),
                    SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString
                ) =>
            {
                Some(token)
            }
            _ => None,
        });
    let Some(token) = token else {
        return false;
    };
    if token.kind() == SyntaxKind::UnquotedString {
        return token.text() == key;
    }

    // 引用符付きのキーは本文へデコードして比較する（まれ）
    Key::cast(key_node)
        .ok()
        .and_then(|key_node| key_node.body_text())
        .as_deref()
        == Some(key)
}

fn set_object_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Key(key) => key,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Member,
                found: SyntaxKind::Object,
            })
        }
    };

    // 既存のメンバーは値だけを差し替える。キー・区切り・前後のコメントはそのまま残る。
    // 同じキーが複数ある場合はすべて同じ値にする（値の層では後勝ちのため）。
    // メンバーの一覧を作らず、子を 1 回走査して一致する要素だけを扱う。
    let mut existing: Vec<(usize, RedNode, RedNode)> = Vec::new();
    let mut matched = false;
    for child in parent.children() {
        if child.kind() != SyntaxKind::Member || !member_key_matches(&child, key) {
            continue;
        }
        matched = true;
        let member = Member::cast(child.clone())?;
        let value_child = member
            .value()
            .ok_or_else(|| HjsonEditError::InvalidTarget {
                path: HjsonObjectPath::from_segments(vec![last.clone()]),
                reason: "member without a value".to_string(),
            })?;
        // 同じ値なら書き換えない（値を変えない編集で体裁を変えない）
        if matches!(string_style, Prefer::Preserve)
            && values_equal(&HjsonValue::from_node(value_child.syntax())?, value)
        {
            continue;
        }
        existing.push((child.index(), child, value_child.syntax().clone()));
    }

    // 一致するメンバーがあるときは差し替えだけで終える（同値で全部を飛ばした場合も含む）
    if matched {
        let mut green: GreenNode = parent.green().clone().into_owned();
        for (member_index, member, value_child) in existing {
            let updated = render_value_like_node(
                &value_child,
                value,
                string_style,
                &RenderConfig {
                    string_style: None,
                    newline,
                },
            )?;
            // 値はメンバーの子なので、メンバー → オブジェクトの順に差し替える
            let value_index = value_child.index();
            let updated_member = member
                .green()
                .clone()
                .splice_children(value_index..value_index + 1, vec![updated.into()]);

            green =
                green.splice_children(member_index..member_index + 1, vec![updated_member.into()]);
        }

        return Ok(green);
    }

    let object_ast = AstObject::cast(parent.clone())?;
    let detect_style = matches!(string_style, Prefer::Preserve) && value.has_string();
    let insert_style = resolve_insert_style(
        string_style,
        if detect_style {
            object_ast.detected_string_style()
        } else {
            None
        },
    );
    let insert_style = checked_style(value, insert_style, string_style)?;
    let config = RenderConfig {
        string_style: insert_style,
        newline,
    };

    let first_member = parent
        .children()
        .find(|child| child.kind() == SyntaxKind::Member);
    let (before_colon_trivia, after_colon_trivia) = match first_member {
        Some(node) => {
            let member = Member::cast(node)?;
            (
                style_whitespace(&member.before_colon_trivia),
                style_whitespace(&member.after_colon_trivia),
            )
        }
        None => (vec![], vec![Trivia::Whitespace(" ".to_string())]),
    };

    let member_node: GreenElement = value
        .render_member_node(
            key,
            None,
            &config,
            &before_colon_trivia,
            &after_colon_trivia,
        )
        .into();
    let element = RenderedElement::single(member_node);

    let (at, elements) = plan_insert(parent, InsertPosition::End, element, newline, None)?;
    Ok(parent.green().splice_children(at..at, elements))
}

/// 新しいメンバーの体裁として写すトリビア。
///
/// 写すのは `:` の直後の空白の並びだけ。コメントと改行はそのメンバーの内容・レイアウトで
/// あり、複製すると新しいメンバーへコメントが増える。
fn style_whitespace(trivia: &[Trivia]) -> Vec<Trivia> {
    match trivia.first() {
        Some(Trivia::Whitespace(_)) => trivia
            .iter()
            .take_while(|item| matches!(item, Trivia::Whitespace(_)))
            .cloned()
            .collect(),
        Some(_) => vec![Trivia::Whitespace(" ".to_string())],
        None => Vec::new(),
    }
}

fn set_array_child(
    parent: &RedNode,
    last: &PathSegment,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    let seg = match last {
        PathSegment::Array(seg) => seg,
        _ => {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: SyntaxKind::Object,
            })
        }
    };

    // 要素の一覧を作らず、子をたどって長さと対象を得る
    let len = parent
        .children()
        .filter(|child| child.kind() == SyntaxKind::Value)
        .count();

    match seg {
        ArrayPathSegment::Index(index) => {
            if *index >= len {
                return Err(HjsonEditError::ArrayIndexOutOfBounds { index: *index, len });
            }
            let current = parent
                .children()
                .filter(|child| child.kind() == SyntaxKind::Value)
                .nth(*index)
                .ok_or(HjsonEditError::ArrayIndexOutOfBounds { index: *index, len })?;
            if matches!(string_style, Prefer::Preserve)
                && values_equal(&HjsonValue::from_node(&current)?, value)
            {
                return Ok(parent.green().clone().into_owned());
            }
            let array_ast = AstArray::cast(parent.clone())?;
            let insert_style = resolve_insert_style(
                string_style,
                if matches!(string_style, Prefer::Preserve) && value.has_string() {
                    array_ast.detected_string_style()
                } else {
                    None
                },
            );
            let insert_style = checked_style(value, insert_style, string_style)?;
            let updated = render_value_like_node(
                &current,
                value,
                string_style,
                &RenderConfig {
                    string_style: insert_style,
                    newline,
                },
            )?;
            // 値ノードをそのまま差し替える（トリビアを含むスロットの再計算は不要）
            let at = current.index();
            Ok(parent
                .green()
                .splice_children(at..at + 1, vec![updated.into()]))
        }
        ArrayPathSegment::InsertAt(index) => {
            if *index > len {
                return Err(HjsonEditError::ArrayIndexOutOfBounds { index: *index, len });
            }
            let position = if *index == len {
                InsertPosition::End
            } else {
                InsertPosition::Before(*index)
            };
            let insert_style = array_insert_style(parent, value, string_style)?;
            insert_array_element(parent, position, value, insert_style, newline)
        }
        ArrayPathSegment::Push => {
            let insert_style = array_insert_style(parent, value, string_style)?;
            insert_array_element(parent, InsertPosition::End, value, insert_style, newline)
        }
    }
}

/// 配列へ要素を入れるときの文字列スタイル。
fn array_insert_style(
    parent: &RedNode,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
) -> Result<Option<QuoteStyle>, HjsonEditError> {
    let detected = if matches!(string_style, Prefer::Preserve) && value.has_string() {
        AstArray::cast(parent.clone())?.detected_string_style()
    } else {
        None
    };
    checked_style(
        value,
        resolve_insert_style(string_style, detected),
        string_style,
    )
}

fn insert_array_element(
    parent: &RedNode,
    position: InsertPosition,
    value: &HjsonValue,
    insert_style: Option<QuoteStyle>,
    newline: NewlineKind,
) -> Result<GreenNode, HjsonEditError> {
    let value_node: GreenElement = value
        .render_value_node(&RenderConfig {
            string_style: insert_style,
            newline,
        })
        .into();
    let element = RenderedElement::single(value_node);

    let (at, elements) = plan_insert(parent, position, element, newline, None)?;
    Ok(parent.green().splice_children(at..at, elements))
}

fn remove_object_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let key = match last {
        PathSegment::Key(key) => key,
        _ => {
            return Err(HjsonEditError::PathNotFound(
                HjsonObjectPath::from_segments(vec![last.clone()]),
            ))
        }
    };

    // 同じキーが複数ある場合はすべて削除する（値の層では 1 つのキーになるため）
    let object = AstObject::cast(parent.clone())?;
    let members = object.members();
    if !members
        .iter()
        .any(|member| member.member_key().as_deref() == Some(key.as_str()))
    {
        return Err(HjsonEditError::PathNotFound(
            HjsonObjectPath::from_segments(vec![last.clone()]),
        ));
    }

    let children: Vec<_> = parent.children_with_tokens().collect();
    let mut ranges: Vec<_> = members
        .iter()
        .zip(object.slots())
        .filter(|(member, _)| member.member_key().as_deref() == Some(key.as_str()))
        .map(|(_, slot)| crate::high_level::ops::insert::removal_range(&children, &slot))
        .collect();
    ranges.sort_by_key(|range| range.start);

    // 直前トリビアを含む範囲は前の要素の行末と重なる。重なったまま順に消すと
    // インデックスがずれて別の要素まで消えるため、先にまとめる。
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }

    let mut green: GreenNode = parent.green().clone().into_owned();
    for range in merged.into_iter().rev() {
        green = green.splice_children(range, Vec::new());
    }

    Ok(green)
}

fn remove_array_child(parent: &RedNode, last: &PathSegment) -> Result<GreenNode, HjsonEditError> {
    let seg = match last {
        PathSegment::Array(seg) => seg,
        _ => {
            return Err(HjsonEditError::PathNotFound(
                HjsonObjectPath::from_segments(vec![last.clone()]),
            ))
        }
    };

    let index = array_slot_index(parent, seg).ok_or_else(|| {
        HjsonEditError::PathNotFound(HjsonObjectPath::from_segments(vec![last.clone()]))
    })?;
    let range = plan_remove(parent, index)?;

    Ok(parent.green().splice_children(range, Vec::new()))
}

/// エラーのパスを、利用者が指定したパスに付け替える（理由はそのまま）。
pub(crate) fn retarget(error: HjsonEditError, path: &HjsonObjectPath) -> HjsonEditError {
    match error {
        HjsonEditError::InvalidTarget { reason, .. } => HjsonEditError::InvalidTarget {
            path: path.clone(),
            reason,
        },
        HjsonEditError::PathNotFound(_) => HjsonEditError::PathNotFound(path.clone()),
        other => other,
    }
}

/// `set` が実際に書き込むコンテナ（実在する最深の祖先）のパス。
pub(crate) fn receiving_container(root: &RedNode, path: &HjsonObjectPath) -> HjsonObjectPath {
    let Some((parent, _)) = path.split_last() else {
        return path.clone();
    };

    let prefix = PathResolver::new(root.clone()).resolve_existing_prefix(&parent);
    parent
        .split_at(prefix.consumed)
        .map(|(prefix, _)| prefix)
        .unwrap_or_else(|| HjsonObjectPath::from_segments(Vec::new()))
}

fn render_value_like_node(
    node: &RedNode,
    value: &HjsonValue,
    string_style: Prefer<QuoteStyle>,
    config: &RenderConfig,
) -> Result<GreenNode, HjsonEditError> {
    let detected = detected_string_style_of_node(node);
    let quote_style = resolve_quote_style(string_style, detected);
    let quote_style = checked_style(value, Some(quote_style), string_style)?;

    Ok(value.render_value_node(&RenderConfig {
        string_style: quote_style,
        newline: config.newline,
    }))
}

/// 値が指定スタイルで書けるかを確認し、必要なら降格する。
///
/// `Prefer::Preserve` と `Prefer::Preferred` は希望なので、書けない場合は二重引用符へ降格する。
/// `Prefer::Explicit` は指定なので、書けない場合はエラーを返す。
fn checked_style(
    value: &HjsonValue,
    style: Option<QuoteStyle>,
    preference: Prefer<QuoteStyle>,
) -> Result<Option<QuoteStyle>, HjsonEditError> {
    value.ensure_representable()?;

    match style {
        Some(style) if !value.is_representable_with(style) => match preference {
            Prefer::Preserve | Prefer::Preferred(_) => Ok(Some(QuoteStyle::DoubleQuoted)),
            Prefer::Explicit(_) => Err(HjsonEditError::ValueNotRepresentable {
                reason: format!("the requested string style {style:?} cannot represent this value"),
            }),
        },
        other => Ok(other),
    }
}

fn resolve_quote_style(preference: Prefer<QuoteStyle>, detected: Option<QuoteStyle>) -> QuoteStyle {
    match preference {
        Prefer::Preserve => detected.unwrap_or(QuoteStyle::DoubleQuoted),
        Prefer::Preferred(style) | Prefer::Explicit(style) => style,
    }
}

fn resolve_insert_style(
    string_style: Prefer<QuoteStyle>,
    detected: Option<QuoteStyle>,
) -> Option<QuoteStyle> {
    match (string_style, detected) {
        (Prefer::Preserve, None) => None,
        _ => Some(resolve_quote_style(string_style, detected)),
    }
}

/// 既存の値を、パース済みの値ノードでそのまま差し替える。
///
/// 値の綴り（数値の表記・区切りの空白・引用符）をそのまま残す。
pub(crate) fn replace_value_with_node(
    root: &RedNode,
    path: &HjsonObjectPath,
    value: &RedNode,
) -> Result<GreenNode, HjsonEditError> {
    let ParentResolution { parent, last } = PathResolver::new(root.clone()).resolve_parent(path)?;
    // 文書直下は Value に包まれていることがあるため、実体のコンテナまで降りる
    let container = match parent.kind() {
        SyntaxKind::Value | SyntaxKind::Root => {
            resolve_body(&parent).map_err(|_| HjsonEditError::InvalidTarget {
                path: path.clone(),
                reason: "the path points into a scalar value".to_string(),
            })?
        }
        _ => parent,
    };

    let new_parent = match (&last, container.kind()) {
        (PathSegment::Key(key), SyntaxKind::Object) => {
            let mut green: GreenNode = container.green().clone().into_owned();
            let mut found = false;
            for child in container.children() {
                if child.kind() != SyntaxKind::Member || !member_key_matches(&child, key) {
                    continue;
                }
                let member = Member::cast(child.clone())?;
                let Some(value_child) = member.value() else {
                    continue;
                };
                let value_index = value_child.syntax().index();
                let updated_member = child.green().clone().splice_children(
                    value_index..value_index + 1,
                    vec![value.green().clone().into()],
                );
                let member_index = child.index();
                green = green
                    .splice_children(member_index..member_index + 1, vec![updated_member.into()]);
                found = true;
            }

            if !found {
                return Err(HjsonEditError::PathNotFound(path.clone()));
            }

            green
        }
        (PathSegment::Array(seg), SyntaxKind::Array) => {
            let index = match seg {
                ArrayPathSegment::Index(index) => *index,
                ArrayPathSegment::Push | ArrayPathSegment::InsertAt(_) => {
                    return Err(HjsonEditError::InvalidTarget {
                        path: path.clone(),
                        reason: "set_value_text needs an existing element".to_string(),
                    })
                }
            };
            let current = container
                .children()
                .filter(|child| child.kind() == SyntaxKind::Value)
                .nth(index)
                .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;
            let at = current.index();

            container
                .green()
                .clone()
                .into_owned()
                .splice_children(at..at + 1, vec![value.green().clone().into()])
        }
        _ => {
            return Err(HjsonEditError::InvalidTarget {
                path: path.clone(),
                reason: "the parent is not a container".to_string(),
            })
        }
    };

    Ok(container.replace_with(new_parent))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::QuoteStyle;
    use crate::high_level::value_ops::build_nested_value;

    #[test]
    fn test_resolve_quote_style() {
        assert_eq!(
            resolve_quote_style(Prefer::Preserve, Some(QuoteStyle::DoubleQuoted)),
            QuoteStyle::DoubleQuoted
        );
        assert_eq!(
            resolve_quote_style(Prefer::Preserve, None),
            QuoteStyle::DoubleQuoted
        );
        assert_eq!(
            resolve_quote_style(Prefer::Explicit(QuoteStyle::Unquoted), None),
            QuoteStyle::Unquoted
        );
    }

    #[test]
    fn test_resolve_insert_style() {
        assert_eq!(
            resolve_insert_style(Prefer::Preserve, Some(QuoteStyle::DoubleQuoted)),
            Some(QuoteStyle::DoubleQuoted)
        );
        assert_eq!(resolve_insert_style(Prefer::Preserve, None), None);
        assert_eq!(
            resolve_insert_style(
                Prefer::Explicit(QuoteStyle::Unquoted),
                Some(QuoteStyle::DoubleQuoted)
            ),
            Some(QuoteStyle::Unquoted)
        );
        assert_eq!(
            resolve_insert_style(Prefer::Explicit(QuoteStyle::Unquoted), None),
            Some(QuoteStyle::Unquoted)
        );
    }

    #[test]
    fn test_build_nested_value_single_key_wraps_in_object() {
        let path = vec![PathSegment::Key("a".to_string())];
        let result = build_nested_value(&path, HjsonValue::Integer(1)).unwrap();
        assert_eq!(
            result,
            HjsonValue::Object(vec![("a".into(), HjsonValue::Integer(1))])
        );
    }

    #[test]
    fn test_build_nested_value_nested_keys() {
        let path = vec![
            PathSegment::Key("a".to_string()),
            PathSegment::Key("b".to_string()),
        ];
        let result = build_nested_value(&path, HjsonValue::Integer(1)).unwrap();
        assert_eq!(
            result,
            HjsonValue::Object(vec![(
                "a".into(),
                HjsonValue::Object(vec![("b".into(), HjsonValue::Integer(1))])
            )])
        );
    }

    #[test]
    fn test_build_nested_value_array_segment() {
        let path = vec![PathSegment::Array(ArrayPathSegment::Index(0))];
        let result = build_nested_value(&path, HjsonValue::Integer(1)).unwrap();
        assert_eq!(result, HjsonValue::Array(vec![HjsonValue::Integer(1)]));
    }
}
