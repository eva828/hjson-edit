use rowan::GreenNode;

use crate::ast::{AstNode, Member, Object as AstObject, Trivia, ValueContent};
use crate::high_level::{
    error::HjsonEditError,
    ops::{
        ensure_depth_within_limit,
        insert::{newline_element, plan_insert, InsertPosition, RenderedElement},
        object::rebuild_tree_with_set,
        receiving_container_depth, relative_container_depth,
    },
    path::{HjsonObjectPath, PathSegment},
    resolve::{member_index, resolve_body, ExistingPrefix, PathResolver},
    style::detect_document_newline,
    value::HjsonValue,
    HjsonDocument, Prefer,
};
use crate::syntax::{GreenElement, NewlineKind, RedNode, SyntaxKind};

/// 別ドキュメントのルートオブジェクトを、クォートスタイルとコメントを保ったまま
/// 指定パスへマージするためのトレイト。
pub trait HjsonDocumentMerge {
    /// `source` のルートオブジェクトの各メンバーを `self` の `path` 配下へマージする。
    ///
    /// - 新規メンバーは、そのメンバーに付いていたコメント（先頭のコメント行と行末コメント）ごと
    ///   コピーする。文書全体に付くコメント（ヘッダー/フッター）は対象外。
    /// - オブジェクト同士は深くマージする。
    /// - 既存の葉（非オブジェクト）は後勝ちで置き換える。このときキーとコメントは
    ///   マージ先（ホスト）のものを保ち、値だけを差し替える。
    /// - パスの中間オブジェクトが存在しない場合は自動で作成する。
    /// - パス途中がオブジェクトでない、または source のルートがオブジェクトでない場合はエラー。
    fn merge_from(
        &self,
        path: &HjsonObjectPath,
        source: &HjsonDocument,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentMerge for HjsonDocument {
    fn merge_from(
        &self,
        path: &HjsonObjectPath,
        source: &HjsonDocument,
    ) -> Result<Self, HjsonEditError> {
        let source_object = resolve_source_object(source)?;
        let root = self.syntax_root().clone();

        // マージ後の深さが上限を超えないか。
        // パスで新規に作られる分と、source が持ち込む分を足して見積もる。
        let resolver = PathResolver::new(root.clone());
        let ExistingPrefix { ancestor, consumed } = resolver.resolve_existing_prefix(path);
        let created = path.depth() - consumed;
        let carried = relative_container_depth(&source_object).saturating_sub(1);
        ensure_depth_within_limit(receiving_container_depth(&ancestor) + created + carried)?;

        let updated = merge_from_root(root, path, &source_object)?;
        let document = self.clone().root(updated).note_touched(path.clone());

        if self.current_verify() {
            let source_value = HjsonValue::from_node(source.syntax_root())?;
            crate::high_level::verify::value_edit(
                self.syntax_root(),
                document.syntax_root(),
                path,
                |previous| {
                    crate::high_level::value_ops::merge_at_path(previous, path, &source_value)
                },
                crate::high_level::verify::EditedRegion::Node,
            )?;
        }

        Ok(document)
    }
}

fn resolve_source_object(source: &HjsonDocument) -> Result<RedNode, HjsonEditError> {
    let body = resolve_body(source.syntax_root())?;
    object_body(&body)
        .ok_or_else(|| HjsonEditError::MergeConflict("source root is not an object".to_string()))
}

/// ノードがオブジェクト（値に包まれていればその中身）なら、その本体を返す。
fn object_body(node: &RedNode) -> Option<RedNode> {
    match node.kind() {
        SyntaxKind::Object => Some(node.clone()),
        SyntaxKind::Value => node
            .children()
            .find(|child| child.kind() == SyntaxKind::Object),
        _ => None,
    }
}

fn merge_from_root(
    root: RedNode,
    path: &HjsonObjectPath,
    source_object: &RedNode,
) -> Result<GreenNode, HjsonEditError> {
    let body = resolve_body(&root)?;
    if object_body(&body).is_none() {
        return Err(HjsonEditError::MergeConflict(
            "target root is not an object".to_string(),
        ));
    }
    if path
        .segments()
        .iter()
        .any(|segment| !matches!(segment, PathSegment::Key(_)))
    {
        return Err(HjsonEditError::InvalidPath(
            "merge path must consist of keys".to_string(),
        ));
    }

    // マージ先が既にあるならオブジェクトでなければならない。
    let resolver = PathResolver::new(root.clone());
    let root = match resolver.resolve_node(path) {
        Ok(node) => {
            if object_body(&node).is_none() {
                return Err(HjsonEditError::MergeConflict(format!(
                    "cannot merge: {path:?} is not an object"
                )));
            }
            root
        }
        Err(HjsonEditError::PathNotFound(_)) => {
            // 途中まで存在する場合、そこがオブジェクトでなければ作成できない。
            let ExistingPrefix { ancestor, consumed } =
                PathResolver::new(root.clone()).resolve_existing_prefix(path);
            if consumed < path.depth() && object_body(&ancestor).is_none() {
                return Err(HjsonEditError::MergeConflict(format!(
                    "cannot merge: {path:?} is not inside an object"
                )));
            }

            // 中間オブジェクトの作成は set と同じ経路に任せる
            let created = rebuild_tree_with_set(
                root,
                path,
                &HjsonValue::Object(Vec::new()),
                Prefer::Preserve,
            )?;
            RedNode::new_root(created)
        }
        Err(error) => return Err(error),
    };

    let target = PathResolver::new(root.clone()).resolve_node(path)?;
    let target_object = object_body(&target).ok_or_else(|| {
        HjsonEditError::MergeConflict(format!("cannot merge: {path:?} is not an object"))
    })?;
    // 対象のコンテナは行の途中から始まることもあるため、字下げをここで求めて渡す
    let base_indent = crate::high_level::indent::line_indent(
        &crate::high_level::indent::root_of(&target),
        target_object.text_range().start().into(),
    );
    let merged = merge_objects(&target_object, source_object, &base_indent)?;

    Ok(target_object.replace_with(merged))
}

/// `target_object` を `source_object` の各メンバーで更新する。
///
/// 対象オブジェクトの green ノードを毎回更新しながら処理する。
fn merge_objects(
    target_object: &RedNode,
    source_object: &RedNode,
    base_indent: &str,
) -> Result<GreenNode, HjsonEditError> {
    let mut current: GreenNode = target_object.green().clone().into();

    for source_member in source_object
        .children()
        .filter(|child| child.kind() == SyntaxKind::Member)
    {
        let target = RedNode::new_root(current.clone());
        let source = Member::cast(source_member.clone())?;
        let key = source
            .member_key()
            .ok_or_else(|| HjsonEditError::MergeConflict("cannot read member key".to_string()))?;

        current = match member_index(&target, &key) {
            Some(index) => {
                let existing = &AstObject::cast(target.clone())?.members()[index];
                merge_existing_member(&current, existing, &source, base_indent)?
            }
            None => insert_member(&target, &current, &source, base_indent)?,
        };
    }

    Ok(current)
}

/// 新規メンバーを、付いていたコメントごと挿入する。
fn insert_member(
    target_object: &RedNode,
    current: &GreenNode,
    source: &Member,
    base_indent: &str,
) -> Result<GreenNode, HjsonEditError> {
    let newline = detect_document_newline(target_object);
    let element = copy_member_with_comments(source, newline);
    let (at, elements) = plan_insert(
        target_object,
        InsertPosition::End,
        element,
        newline,
        Some(base_indent),
    )?;

    Ok(current.splice_children(at..at, elements))
}

/// 既存メンバーがオブジェクトなら再帰マージ、それ以外は後勝ちで置き換える。
fn merge_existing_member(
    object_green: &GreenNode,
    existing: &Member,
    source: &Member,
    base_indent: &str,
) -> Result<GreenNode, HjsonEditError> {
    let existing_value = existing
        .value()
        .ok_or_else(|| HjsonEditError::InvalidTarget {
            path: HjsonObjectPath::from_segments(Vec::new()),
            reason: "member without a value".to_string(),
        })?;
    let source_value = source
        .value()
        .ok_or_else(|| HjsonEditError::InvalidTarget {
            path: HjsonObjectPath::from_segments(Vec::new()),
            reason: "source member without a value".to_string(),
        })?;
    let both_objects = matches!(existing_value.content(), Ok(ValueContent::Object(_)))
        && matches!(source_value.content(), Ok(ValueContent::Object(_)));

    let updated_value = if both_objects {
        let existing_object = match existing_value.content()? {
            ValueContent::Object(object) => object,
            _ => unreachable!("guarded by both_objects"),
        };
        let source_object = match source_value.content()? {
            ValueContent::Object(object) => object,
            _ => unreachable!("guarded by both_objects"),
        };
        let merged = merge_objects(
            existing_object.syntax(),
            source_object.syntax(),
            base_indent,
        )?;
        GreenNode::new(SyntaxKind::Value.into(), vec![merged.into()])
    } else {
        // 後勝ち。キーとコメントはホスト側のものを保ち、値だけを差し替える。
        source_value.syntax().green().clone().into_owned()
    };

    let value_index = existing_value.syntax().index();
    let updated_member = existing
        .syntax()
        .green()
        .clone()
        .splice_children(value_index..value_index + 1, vec![updated_value.into()]);
    let member_index = existing.syntax().index();

    Ok(object_green.splice_children(member_index..member_index + 1, vec![updated_member.into()]))
}

/// ソースのメンバーを、先頭のコメント行と行末コメントごと要素にする。
fn copy_member_with_comments(source: &Member, newline: NewlineKind) -> RenderedElement {
    let mut elements: Vec<GreenElement> = Vec::new();
    let mut has_leading_comments = false;

    for trivia in &source.leading_trivia {
        if trivia.is_comment() {
            if has_leading_comments {
                elements.push(newline_element(newline));
            }
            elements.push(trivia.to_green_element());
            has_leading_comments = true;
        }
    }
    if has_leading_comments {
        elements.push(newline_element(newline));
    }

    elements.push(source.syntax().green().clone().into());

    // 行末コメントは、元の空白ごとコピーして体裁を変えない
    let mut pending_whitespace: Option<GreenElement> = None;
    for trivia in &source.trailing_trivia {
        match trivia {
            Trivia::Whitespace(_) => pending_whitespace = Some(trivia.to_green_element()),
            Trivia::Comment(_) => {
                if let Some(whitespace) = pending_whitespace.take() {
                    elements.push(whitespace);
                }
                elements.push(trivia.to_green_element());
            }
            _ => pending_whitespace = None,
        }
    }

    RenderedElement::from_parts(elements, has_leading_comments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::high_level::{
        ops::object::HjsonDocumentEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
    };
    use crate::{Format, FormatConfig, RedNode};

    fn path(s: &str) -> HjsonObjectPath {
        HjsonObjectPath::parse(s.to_string()).unwrap()
    }

    fn render(doc: &HjsonDocument) -> String {
        let ast = doc.ast().unwrap();
        let formatted = ast.format(&FormatConfig::default());
        RedNode::new_root(formatted).text().to_string()
    }

    fn get_value(doc: &HjsonDocument, p: &str) -> HjsonValue {
        doc.get(path(p)).expect("value")
    }

    #[test]
    fn merge_into_empty_preserves_value_styles() {
        let host = HjsonDocument::parse("{}").unwrap();
        let source = HjsonDocument::parse(
            r#"{
  unq: plain
  q: "quoted"
  b: '''block
line'''
}"#,
        )
        .unwrap();
        let merged = host.merge_from(&path(""), &source).unwrap();
        let text = render(&merged);
        assert!(text.contains("unq: plain"), "unquoted lost:\n{text}");
        assert!(text.contains("q: \"quoted\""), "quoted lost:\n{text}");
        assert!(text.contains("b: '''block\nline'''"), "block lost:\n{text}");
    }

    #[test]
    fn merge_preserves_numeric_keys() {
        let host = HjsonDocument::parse("{}").unwrap();
        let source = HjsonDocument::parse("{ Chat: {\n  0: Oink!\n} }").unwrap();
        let merged = host.merge_from(&path(""), &source).unwrap();
        let text = render(&merged);
        assert!(text.contains("0: Oink!"), "numeric key/value lost:\n{text}");
    }

    #[test]
    fn merge_deep_merges_objects() {
        let host = HjsonDocument::parse("{ A: { x: 1 } }").unwrap();
        let source = HjsonDocument::parse("{ A: { y: 2 } }").unwrap();
        let merged = host.merge_from(&path(""), &source).unwrap();
        assert_eq!(get_value(&merged, "A.x"), HjsonValue::Integer(1));
        assert_eq!(get_value(&merged, "A.y"), HjsonValue::Integer(2));
    }

    #[test]
    fn merge_later_wins_replaces_leaf() {
        let host = HjsonDocument::parse(r#"{ A: "one" }"#).unwrap();
        let source = HjsonDocument::parse(r#"{ A: "two" }"#).unwrap();
        let merged = host.merge_from(&path(""), &source).unwrap();
        assert_eq!(
            get_value(&merged, "A"),
            HjsonValue::String("two".to_string())
        );
    }

    #[test]
    fn merge_at_path_creates_wrapper_objects() {
        let host = HjsonDocument::parse("{}").unwrap();
        let source = HjsonDocument::parse("{ X: 1 }").unwrap();
        let merged = host.merge_from(&path("Mods.Foo"), &source).unwrap();
        assert_eq!(get_value(&merged, "Mods.Foo.X"), HjsonValue::Integer(1));
    }

    #[test]
    fn merge_path_conflict_on_non_object_errors() {
        let host = HjsonDocument::parse(r#"{ Mods: "x" }"#).unwrap();
        let source = HjsonDocument::parse("{ X: 1 }").unwrap();
        let err = host.merge_from(&path("Mods.Foo"), &source).unwrap_err();
        assert!(matches!(err, HjsonEditError::MergeConflict(_)));
    }

    #[test]
    fn merge_source_root_scalar_errors() {
        let host = HjsonDocument::parse("{}").unwrap();
        let source = HjsonDocument::parse(r#""just a string""#).unwrap();
        let err = host.merge_from(&path(""), &source).unwrap_err();
        assert!(matches!(err, HjsonEditError::MergeConflict(_)));
    }

    #[test]
    fn merge_roundtrips_through_format() {
        let host = HjsonDocument::parse("{}").unwrap();
        let source = HjsonDocument::parse(
            r#"{
  Header: { Author: "me" }
  Items: { Sword: { DisplayName: "剣" } }
}"#,
        )
        .unwrap();
        let merged = host.merge_from(&path("Mods.Example"), &source).unwrap();
        let text = render(&merged);
        let reparsed = HjsonDocument::parse(&text).unwrap();
        assert_eq!(
            get_value(&reparsed, "Mods.Example.Header.Author"),
            HjsonValue::String("me".to_string())
        );
        assert_eq!(
            get_value(&reparsed, "Mods.Example.Items.Sword.DisplayName"),
            HjsonValue::String("剣".to_string())
        );
    }

    #[test]
    fn merge_sequential_files_at_prefixes() {
        let mut doc = HjsonDocument::parse("{}").unwrap();
        let a = HjsonDocument::parse("{ Sword: {\n  DisplayName: A\n} }").unwrap();
        let b = HjsonDocument::parse("{ Shield: {\n  DisplayName: B\n} }").unwrap();
        doc = doc.merge_from(&path("Mods.Alpha"), &a).unwrap();
        doc = doc.merge_from(&path("Mods.Beta"), &b).unwrap();
        assert_eq!(
            get_value(&doc, "Mods.Alpha.Sword.DisplayName"),
            HjsonValue::String("A".to_string())
        );
        assert_eq!(
            get_value(&doc, "Mods.Beta.Shield.DisplayName"),
            HjsonValue::String("B".to_string())
        );
    }
}
