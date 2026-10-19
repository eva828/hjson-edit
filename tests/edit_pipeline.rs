//! 編集パイプラインの不変条件（CLI が行う「編集 → 変更したコンテナの整形」）。
//!
//! 決定的な疑似乱数で文書と操作列を作り、各操作の後に次を確認する。
//!
//! 1. 結果は常に再パースでき、値が変わらない
//! 2. 整形しても値とコメントが変わらない
//! 3. 整形は冪等（2 回適用しても同じ）
//! 4. 変更したコンテナ（`touched_containers`）を整形すれば、変更箇所が整う

use hjson_edit::{
    high_level::{
        ops::{comment::HjsonDocumentCommentEdit, object::HjsonDocumentObjectEdit},
        path::{HjsonCommentPath, HjsonObjectPath},
    },
    HjsonDocument, HjsonEditError, HjsonValue,
};

// ------------------------------------------------------------------
// 決定的な疑似乱数（xorshift64*）
// ------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;

        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

// ------------------------------------------------------------------
// 文書の生成
// ------------------------------------------------------------------

/// 生成した要素の種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryKind {
    Object,
    Array,
    Scalar,
}

/// 生成した要素（パスと種類）。
#[derive(Clone, Debug)]
struct Entry {
    path: HjsonObjectPath,
    kind: EntryKind,
}

impl Entry {
    fn is_container(&self) -> bool {
        matches!(self.kind, EntryKind::Object | EntryKind::Array)
    }
}

fn object_path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw.to_string()).expect("valid path")
}

fn child_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}.{key}")
    }
}

fn child_index_path(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

/// 値（スカラー）。引用符なし文字列は行末までを飲み込むため、単語だけにする。
fn generate_scalar(rng: &mut Rng) -> (String, bool) {
    match rng.below(6) {
        0 => (format!("{}", rng.below(100)), false),
        1 => (format!("\"text{}\"", rng.below(10)), false),
        2 => ("true".to_string(), false),
        3 => ("null".to_string(), false),
        4 => (format!("word{}", rng.below(10)), true),
        _ => (format!("{}.{}", rng.below(10), rng.below(100)), false),
    }
}

fn generate_body(
    rng: &mut Rng,
    depth: usize,
    prefix: &str,
    entries: &mut Vec<Entry>,
    lines: &mut Vec<String>,
    indent: usize,
) {
    let pad = "  ".repeat(indent);
    let members = 1 + rng.below(3);

    for index in 0..members {
        let key = format!("k{depth}_{index}");
        let path = child_path(prefix, &key);

        if index > 0 && rng.chance(25) {
            lines.push(format!("{pad}// note"));
        }

        if depth < 3 && rng.chance(35) {
            // ネストしたオブジェクト
            entries.push(Entry {
                path: object_path(&path),
                kind: EntryKind::Object,
            });
            lines.push(format!("{pad}{key}: {{"));
            generate_body(rng, depth + 1, &path, entries, lines, indent + 1);
            lines.push(format!("{pad}}}"));
        } else if depth < 3 && rng.chance(30) {
            // 配列
            entries.push(Entry {
                path: object_path(&path),
                kind: EntryKind::Array,
            });
            lines.push(format!("{pad}{key}: ["));
            let items = 1 + rng.below(2);
            for item in 0..items {
                let item_path = child_index_path(&path, item);
                let (value, unquoted) = generate_scalar(rng);
                if !unquoted && rng.chance(30) {
                    // 行末コメントは行末までを飲み込まない値にだけ付ける
                    lines.push(format!("{pad}  {value} // item"));
                } else {
                    lines.push(format!("{pad}  {value}"));
                }
                entries.push(Entry {
                    path: object_path(&item_path),
                    kind: EntryKind::Scalar,
                });
            }
            lines.push(format!("{pad}]"));
        } else {
            let (value, _) = generate_scalar(rng);
            entries.push(Entry {
                path: object_path(&path),
                kind: EntryKind::Scalar,
            });
            lines.push(format!("{pad}{key}: {value}"));
        }
    }

    if rng.chance(20) {
        lines.push(format!("{pad}// footer"));
    }
}

fn generate_document(rng: &mut Rng) -> (String, Vec<Entry>) {
    let mut entries = Vec::new();
    let mut lines = vec!["{".to_string()];
    generate_body(rng, 0, "", &mut entries, &mut lines, 1);
    lines.push("}".to_string());
    lines.push(String::new());

    (lines.join("\n"), entries)
}

// ------------------------------------------------------------------
// 不変条件の確認
// ------------------------------------------------------------------

/// 記録しておいたコメント（整形で失われないことを確認する）。
type CommentRecord = (HjsonCommentPath, Option<String>);

fn comment_path(raw: &str) -> HjsonCommentPath {
    HjsonCommentPath::try_from(raw).expect("valid comment path")
}

/// コメントの対象要素のパス。
fn comment_element_path(path: &HjsonCommentPath) -> Option<HjsonObjectPath> {
    match path {
        HjsonCommentPath::Before(path) | HjsonCommentPath::Inline(path) => Some(path.clone()),
        HjsonCommentPath::Header | HjsonCommentPath::Footer => None,
    }
}

/// 消えた要素に付いていたコメントの記録を落とす。
fn drop_comment_records(comments: &mut Vec<CommentRecord>, path: &HjsonObjectPath) {
    comments.retain(|(comment, _)| {
        comment_element_path(comment)
            .is_none_or(|element| !element.segments().starts_with(path.segments()))
    });
}

/// 既存のコメントを置き換えるので、対象の記録を落とす。
fn drop_comment_record(comments: &mut Vec<CommentRecord>, target: &HjsonCommentPath) {
    comments.retain(|(comment, _)| comment != target);
}

/// 文書の値を取り出す。
fn value_of(doc: &HjsonDocument) -> HjsonValue {
    doc.get(object_path("")).expect("document value")
}

/// 変更したコンテナを整形し、不変条件を確認する。
fn format_touched(doc: HjsonDocument, comments: &[CommentRecord], context: &str) -> HjsonDocument {
    let before_value = value_of(&doc);
    let config = doc.current_format_config();
    let mut formatted = doc.clone();

    for container in doc.touched_containers().to_vec() {
        match formatted.format_container(&container, &config) {
            Ok(updated) => formatted = updated,
            // 消えた・コンテナでなくなった対象は CLI と同じく無視する
            Err(HjsonEditError::PathNotFound(_) | HjsonEditError::UnexpectedStructure { .. }) => {}
            Err(error) => panic!("{context}: formatting {container} failed: {error}"),
        }
    }

    // 再パースでき、値も変わらない
    let reparsed = HjsonDocument::parse(&formatted.to_string())
        .unwrap_or_else(|error| panic!("{context}: formatted document does not parse: {error}"));
    assert_eq!(
        value_of(&reparsed),
        before_value,
        "{context}: formatting changed values"
    );

    // 記録したコメントが保たれている
    for (path, expected) in comments {
        assert_eq!(
            formatted.get_comment(path.clone()).expect("comment read"),
            expected.clone(),
            "{context}: formatting changed the comment at {path}"
        );
    }

    // 冪等
    let mut twice = formatted.clone();
    for container in formatted.touched_containers().to_vec() {
        if let Ok(updated) = twice.format_container(&container, &config) {
            twice = updated;
        }
    }
    assert_eq!(
        twice.to_string(),
        formatted.to_string(),
        "{context}: formatting is not idempotent"
    );

    formatted
}

// ------------------------------------------------------------------
// テスト本体
// ------------------------------------------------------------------

#[test]
fn random_edit_sequences_keep_documents_valid() {
    for seed in 1..=40u64 {
        let mut rng = Rng::new(seed);
        let (input, mut entries) = generate_document(&mut rng);
        let mut doc = HjsonDocument::parse(&input).unwrap_or_else(|error| {
            panic!("seed {seed}: generated document is invalid: {error}\n{input}")
        });
        let mut comments: Vec<CommentRecord> = Vec::new();

        for step in 0..4 {
            let context = format!("seed {seed} step {step}");
            let before = doc.to_string();

            if entries.is_empty() {
                break;
            }
            let index = rng.below(entries.len());
            let entry = entries[index].clone();

            match rng.below(6) {
                0 => {
                    // 既存の値を置き換える
                    let (value, _) = generate_scalar(&mut rng);
                    let parsed = HjsonDocument::parse(&value).expect("generated value is valid");
                    let value = value_of(&parsed);

                    doc = doc
                        .set(entry.path.clone(), value)
                        .unwrap_or_else(|error| panic!("{context}: set failed: {error}\n{before}"));
                    if entry.is_container() {
                        let target = entry.path.clone();
                        entries.retain(|other| {
                            other.path == target
                                || !other.path.segments().starts_with(target.segments())
                        });
                        drop_comment_records(&mut comments, &target);
                    }
                    if let Some(slot) = entries.iter_mut().find(|other| other.path == entry.path) {
                        slot.kind = EntryKind::Scalar;
                    }
                }
                1 => {
                    // コメントを行末に付ける（引用符なし文字列では失敗する）
                    let owned = doc.clone();
                    match owned
                        .set_comment(comment_path(&format!("{}:inline", entry.path)), "inline")
                    {
                        Ok(updated) => {
                            doc = updated;
                            let target = comment_path(&format!("{}:inline", entry.path));
                            drop_comment_record(&mut comments, &target);
                            comments.push((target, Some("inline".to_string())));
                        }
                        Err(HjsonEditError::InvalidTarget { .. }) => {
                            // 引用符なし文字列の直後には置けない（文書は変わらない）
                            assert_eq!(
                                doc.to_string(),
                                before,
                                "{context}: failed edit changed the document"
                            );
                        }
                        Err(error) => panic!("{context}: set_comment failed: {error}"),
                    }
                }
                2 => {
                    // コメントを要素の前の行に付ける
                    let target = comment_path(&format!("{}:before", entry.path));
                    doc = doc
                        .set_comment(target.clone(), "lead")
                        .unwrap_or_else(|error| panic!("{context}: set_comment failed: {error}"));
                    drop_comment_record(&mut comments, &target);
                    comments.push((target, Some("lead".to_string())));
                }
                3 => {
                    // 追加したコメントを削除する
                    doc = doc
                        .remove_comment(comment_path(&format!("{}:inline", entry.path)))
                        .unwrap_or_else(|error| {
                            panic!("{context}: remove_comment failed: {error}")
                        });
                    comments
                        .retain(|(path, _)| path.to_string() != format!("{}:inline", entry.path));
                }
                4 if entries.len() > 1 => {
                    // 既存の要素を削除する
                    doc = doc.remove(entry.path.clone()).unwrap_or_else(|error| {
                        panic!("{context}: remove failed: {error}\n{before}")
                    });
                    entries
                        .retain(|other| !other.path.segments().starts_with(entry.path.segments()));
                    drop_comment_records(&mut comments, &entry.path);
                }
                _ => {
                    // 配列は末尾へ追加、オブジェクトは新しいキーを追加する
                    let value = HjsonValue::String(format!("added{}", rng.below(10)));
                    match entry.kind {
                        EntryKind::Array => {
                            let path = object_path(&format!("{}[+]", entry.path));
                            doc = doc.set(path, value).unwrap_or_else(|error| {
                                panic!("{context}: append failed: {error}")
                            });
                        }
                        EntryKind::Object => {
                            let key = format!("added{}", rng.below(10));
                            let new_path = object_path(&child_path(&entry.path.to_string(), &key));
                            let existed = entries.iter().any(|other| other.path == new_path);
                            doc = doc.set(new_path.clone(), value).unwrap_or_else(|error| {
                                panic!("{context}: adding a member failed: {error}")
                            });
                            if !existed {
                                entries.push(Entry {
                                    path: new_path,
                                    kind: EntryKind::Scalar,
                                });
                            }
                        }
                        EntryKind::Scalar => continue,
                    }
                }
            }

            // 編集後は必ず再パースできる
            HjsonDocument::parse(&doc.to_string()).unwrap_or_else(|error| {
                panic!("{context}: edited document does not parse: {error}\n{doc}")
            });

            doc = format_touched(doc, &comments, &context);
        }
    }
}

#[test]
fn touched_containers_point_at_the_changed_container() {
    let doc = HjsonDocument::parse("{\n  a: {\n    b: 1\n  }\n  c: 2\n}\n").unwrap();

    // 既存メンバーの置き換えは、その親コンテナ
    let updated = doc.clone().set(object_path("a.b"), 2).unwrap();
    assert_eq!(updated.touched_containers(), &[object_path("a")]);

    // 中間が無い場合は、実在する最深の祖先
    let updated = doc.clone().set(object_path("x.y.z"), 1).unwrap();
    assert_eq!(updated.touched_containers(), &[object_path("")]);

    // 削除は親コンテナ
    let updated = doc.clone().remove(object_path("a.b")).unwrap();
    assert_eq!(updated.touched_containers(), &[object_path("a")]);

    // コメント操作は対象要素のコンテナ、ヘッダー/フッターは記録しない
    let updated = doc
        .clone()
        .set_comment(comment_path("a.b:inline"), "note")
        .unwrap();
    assert_eq!(updated.touched_containers(), &[object_path("a")]);
    let updated = doc
        .clone()
        .set_comment(HjsonCommentPath::Header, "note")
        .unwrap();
    assert!(updated.touched_containers().is_empty());

    // 外側のコンテナが記録されたら、内側の記録は残さない
    let updated = doc
        .clone()
        .set(object_path("a.b"), 3)
        .unwrap()
        .set_comment(comment_path("c:inline"), "note")
        .unwrap();
    assert_eq!(updated.touched_containers(), &[object_path("")]);
}

#[test]
fn verification_can_be_disabled() {
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n")
        .unwrap()
        .verify_edits(false);
    let updated = doc.set(object_path("b"), "hello").unwrap();

    assert!(updated.to_string().contains("b: hello"));
}
