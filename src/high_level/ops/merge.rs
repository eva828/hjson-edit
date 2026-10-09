use rowan::GreenNode;

use crate::ast::{AstNode, Member, Object as AstObject, QuoteStyle, ValueContent};
use crate::high_level::{
    error::HjsonEditError,
    ops::{ensure_depth_within_limit, receiving_container_depth, relative_container_depth},
    path::{HjsonObjectPath, PathSegment},
    resolve::{resolve_body, ExistingPrefix, PathResolver},
    HjsonDocument,
};
use crate::syntax::{RedNode, SyntaxKind};

/// 別ドキュメントのルートオブジェクトを、クォートスタイルを保持したまま
/// 指定パスへマージするためのトレイト。
pub trait HjsonDocumentMerge {
    /// `source` のルートオブジェクトの各メンバーを `self` の `path` 配下へマージする。
    ///
    /// - 挿入元のメンバーはノードごとコピーされるため、値のクォートスタイルが保持される。
    /// - オブジェクト同士は深くマージする。
    /// - 既存のリーフ（非オブジェクト）は後勝ちで置き換える。
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
        Ok(self.clone().root(updated))
    }
}

fn resolve_source_object(source: &HjsonDocument) -> Result<RedNode, HjsonEditError> {
    let body = resolve_body(source.syntax_root())?;
    let not_object = || HjsonEditError::MergeConflict("source root is not an object".to_string());
    match body.kind() {
        SyntaxKind::Object => Ok(body),
        SyntaxKind::Value => body
            .children()
            .find(|child| child.kind() == SyntaxKind::Object)
            .ok_or_else(not_object),
        _ => Err(not_object()),
    }
}

fn merge_from_root(
    root: RedNode,
    path: &HjsonObjectPath,
    source_object: &RedNode,
) -> Result<GreenNode, HjsonEditError> {
    let body = resolve_body(&root)?;
    match body.kind() {
        SyntaxKind::Object => {
            let updated = merge_into_object(&body, path.segments(), source_object)?;
            Ok(root
                .green()
                .splice_children(body.index()..body.index() + 1, vec![updated.into()]))
        }
        SyntaxKind::Value => {
            let inner = body
                .children()
                .find(|child| child.kind() == SyntaxKind::Object)
                .ok_or_else(|| {
                    HjsonEditError::MergeConflict("target root is not an object".to_string())
                })?;
            let updated = merge_into_object(&inner, path.segments(), source_object)?;
            let updated_value = GreenNode::new(SyntaxKind::Value.into(), vec![updated.into()]);
            Ok(root
                .green()
                .splice_children(body.index()..body.index() + 1, vec![updated_value.into()]))
        }
        _ => Err(HjsonEditError::MergeConflict(
            "target root is not an object".to_string(),
        )),
    }
}

/// `root_object` を `path` まで降りて、そのオブジェクトへ `source_object` をマージする。
/// 戻り値は更新後の `root_object` の green ノード。
fn merge_into_object(
    root_object: &RedNode,
    path: &[PathSegment],
    source_object: &RedNode,
) -> Result<GreenNode, HjsonEditError> {
    let Some((first, rest)) = path.split_first() else {
        return merge_objects(root_object, source_object);
    };

    let key = match first {
        PathSegment::Key(key) => key.clone(),
        _ => {
            return Err(HjsonEditError::InvalidPath(
                "merge path must consist of keys".to_string(),
            ));
        }
    };
    let obj_ast = AstObject::cast(root_object.clone())?;

    if let Some(existing) = obj_ast
        .members()
        .into_iter()
        .find(|member| member.member_key().as_deref() == Some(&key))
    {
        let existing_value = existing.value().ok_or(HjsonEditError::ParentNotFound)?;
        match existing_value.content() {
            Ok(ValueContent::Object(inner)) => {
                let updated_inner = merge_into_object(inner.syntax(), rest, source_object)?;
                let member_green = existing.syntax().green().clone();
                let updated_member = member_green.splice_children(
                    existing_value.syntax().index()..existing_value.syntax().index() + 1,
                    vec![
                        GreenNode::new(SyntaxKind::Value.into(), vec![updated_inner.into()]).into(),
                    ],
                );
                Ok(root_object.green().splice_children(
                    existing.syntax().index()..existing.syntax().index() + 1,
                    vec![updated_member.into()],
                ))
            }
            _ => Err(HjsonEditError::MergeConflict(format!(
                "cannot merge: '{key}' is not an object"
            ))),
        }
    } else {
        let created = create_and_merge(rest, source_object)?;
        let member = make_member_node(&key, created);
        let append_idx = obj_ast.append_member_index_before_footer();
        Ok(root_object
            .green()
            .splice_children(append_idx..append_idx, vec![member.into()]))
    }
}

/// 存在しないパス部分を空オブジェクトとして構築し、末端に `source_object` をマージする。
/// 戻り値は `{ key1: { key2: { ... <source merged> } } }` の Object green ノード。
fn create_and_merge(
    path: &[PathSegment],
    source_object: &RedNode,
) -> Result<GreenNode, HjsonEditError> {
    if path.is_empty() {
        let empty = GreenNode::new(
            SyntaxKind::Object.into(),
            vec![
                SyntaxKind::LBrace.create_token("{").into(),
                SyntaxKind::RBrace.create_token("}").into(),
            ],
        );
        let wrapped = RedNode::new_root(empty);
        let body = resolve_body(&wrapped)?;
        return merge_objects(&body, source_object);
    }

    let (first, rest) = path.split_first().unwrap();
    let key = match first {
        PathSegment::Key(key) => key.clone(),
        _ => {
            return Err(HjsonEditError::InvalidPath(
                "merge path must consist of keys".to_string(),
            ));
        }
    };
    let inner = create_and_merge(rest, source_object)?;
    let member = make_member_node(&key, inner);
    Ok(GreenNode::new(
        SyntaxKind::Object.into(),
        vec![
            SyntaxKind::LBrace.create_token("{").into(),
            member.into(),
            SyntaxKind::RBrace.create_token("}").into(),
        ],
    ))
}

/// `source_object` の各メンバーを `target_object` へマージする。
/// 対象オブジェクトの green ノードを毎回更新しながら処理する。
fn merge_objects(
    target_object: &RedNode,
    source_object: &RedNode,
) -> Result<GreenNode, HjsonEditError> {
    let mut current: GreenNode = target_object.green().clone().into();

    for src_member in source_object
        .children()
        .filter(|child| child.kind() == SyntaxKind::Member)
    {
        let wrapped = RedNode::new_root(current.clone());
        let body = resolve_body(&wrapped)?;
        let obj_node = match body.kind() {
            SyntaxKind::Object => body.clone(),
            SyntaxKind::Value => body
                .children()
                .find(|child| child.kind() == SyntaxKind::Object)
                .ok_or_else(|| {
                    HjsonEditError::MergeConflict("target is not an object".to_string())
                })?
                .clone(),
            _ => {
                return Err(HjsonEditError::MergeConflict(
                    "target is not an object".to_string(),
                ));
            }
        };
        let obj_ast = AstObject::cast(obj_node)?;
        let src = Member::cast(src_member.clone())?;
        let key = src
            .member_key()
            .ok_or_else(|| HjsonEditError::MergeConflict("cannot read member key".to_string()))?;

        if let Some(existing) = obj_ast
            .members()
            .into_iter()
            .find(|member| member.member_key().as_deref() == Some(&key))
        {
            current = merge_existing_member(&current, &existing, &src)?;
        } else {
            let append_idx = obj_ast.append_member_index_before_footer();
            current =
                current.splice_children(append_idx..append_idx, vec![src_member.green().into()]);
        }
    }

    Ok(current)
}

/// 既存メンバーがオブジェクトなら再帰マージ、それ以外は後勝ちで置き換える。
fn merge_existing_member(
    object_green: &GreenNode,
    existing: &Member,
    source: &Member,
) -> Result<GreenNode, HjsonEditError> {
    let existing_value = existing.value().ok_or(HjsonEditError::ParentNotFound)?;
    let source_value = source.value().ok_or(HjsonEditError::ParentNotFound)?;
    let both_objects = matches!(existing_value.content(), Ok(ValueContent::Object(_)))
        && matches!(source_value.content(), Ok(ValueContent::Object(_)));

    if both_objects {
        let existing_obj = match existing_value.content()? {
            ValueContent::Object(object) => object,
            _ => unreachable!("guarded by both_objects"),
        };
        let source_obj = match source_value.content()? {
            ValueContent::Object(object) => object,
            _ => unreachable!("guarded by both_objects"),
        };
        let merged = merge_objects(existing_obj.syntax(), source_obj.syntax())?;
        let member_green = existing.syntax().green().clone();
        let updated_member = member_green.splice_children(
            existing_value.syntax().index()..existing_value.syntax().index() + 1,
            vec![GreenNode::new(SyntaxKind::Value.into(), vec![merged.into()]).into()],
        );
        Ok(object_green.splice_children(
            existing.syntax().index()..existing.syntax().index() + 1,
            vec![updated_member.into()],
        ))
    } else {
        // 後勝ち: 既存メンバー（その前後のtrivia込み）を source のメンバーノードで置き換える
        let range = existing.span_range();
        Ok(object_green.splice_children(range, vec![source.syntax().green().into()]))
    }
}

fn make_member_node(key: &str, value: GreenNode) -> GreenNode {
    let key_node = render_key_node(key);
    let value_node = GreenNode::new(SyntaxKind::Value.into(), vec![value.into()]);
    GreenNode::new(
        SyntaxKind::Member.into(),
        vec![
            key_node.into(),
            SyntaxKind::Colon.create_token(":").into(),
            SyntaxKind::Whitespace.create_token(" ").into(),
            value_node.into(),
        ],
    )
}

fn render_key_node(key: &str) -> GreenNode {
    let token = if QuoteStyle::Unquoted.is_valid_for_text(key) {
        SyntaxKind::UnquotedString.create_token(key).into()
    } else {
        SyntaxKind::String
            .create_token(QuoteStyle::DoubleQuoted.render(key).as_str())
            .into()
    };
    GreenNode::new(SyntaxKind::Key.into(), vec![token])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::high_level::{
        ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, value::HjsonValue,
        HjsonDocument,
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
        let text = render(&merged);
        assert!(text.contains("Mods:"), "wrapper missing:\n{text}");
        assert!(text.contains("Foo:"), "wrapper missing:\n{text}");
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
