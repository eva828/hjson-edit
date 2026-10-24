//! `HjsonValue` に対する編集の意味。
//!
//! テキスト編集（`set` / `remove` / `merge_from`）の正しさを検査するための基準になる。
//! 「値の木に同じ操作を適用した結果」と「編集後の文書を再パースした値」が一致することを
//! 検証層（`super::verify`）が確認する。

use crate::high_level::error::HjsonEditError;
use crate::high_level::path::{ArrayPathSegment, HjsonObjectPath, PathSegment};
use crate::high_level::value::HjsonValue;

/// `set` の意味。空パスは文書全体の置き換え。
pub(crate) fn set_path(
    root: &HjsonValue,
    path: &HjsonObjectPath,
    value: HjsonValue,
) -> Result<HjsonValue, HjsonEditError> {
    if path.depth() == 0 {
        return Ok(value);
    }

    set_in(root, path.segments(), value)
}

pub(crate) fn set_in(
    current: &HjsonValue,
    segments: &[PathSegment],
    value: HjsonValue,
) -> Result<HjsonValue, HjsonEditError> {
    let (first, rest) = segments
        .split_first()
        .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;

    match (current, first) {
        (HjsonValue::Object(entries), PathSegment::Key(key)) => {
            let mut entries = entries.clone();
            match entries.iter_mut().find(|(existing, _)| existing == key) {
                Some(entry) => {
                    entry.1 = if rest.is_empty() {
                        value
                    } else {
                        set_in(&entry.1, rest, value)?
                    };
                }
                None => {
                    let created = if rest.is_empty() {
                        value
                    } else {
                        build_nested_value(rest, value)?
                    };
                    entries.push((key.clone(), created));
                }
            }
            Ok(HjsonValue::Object(entries))
        }
        (HjsonValue::Array(items), PathSegment::Array(segment)) => {
            let mut items = items.clone();
            let len = items.len();
            match segment {
                ArrayPathSegment::Index(index) => {
                    if *index >= len {
                        return Err(HjsonEditError::ArrayIndexOutOfBounds { index: *index, len });
                    }
                    items[*index] = if rest.is_empty() {
                        value
                    } else {
                        set_in(&items[*index], rest, value)?
                    };
                }
                ArrayPathSegment::InsertAt(index) => {
                    if *index > len {
                        return Err(HjsonEditError::ArrayIndexOutOfBounds { index: *index, len });
                    }
                    if rest.is_empty() {
                        items.insert(*index, value);
                    } else if *index < len {
                        items[*index] = set_in(&items[*index], rest, value)?;
                    } else {
                        items.push(build_nested_value(rest, value)?);
                    }
                }
                ArrayPathSegment::Push => {
                    if rest.is_empty() {
                        items.push(value);
                    } else if len > 0 {
                        items[len - 1] = set_in(&items[len - 1], rest, value)?;
                    } else {
                        items.push(build_nested_value(rest, value)?);
                    }
                }
            }
            Ok(HjsonValue::Array(items))
        }
        (HjsonValue::Object(_), PathSegment::Array(_)) => {
            Err(HjsonEditError::UnexpectedStructure {
                expected: crate::SyntaxKind::Array,
                found: crate::SyntaxKind::Object,
            })
        }
        (HjsonValue::Array(_), PathSegment::Key(_)) => Err(HjsonEditError::UnexpectedStructure {
            expected: crate::SyntaxKind::Object,
            found: crate::SyntaxKind::Array,
        }),
        (current, _) => Err(HjsonEditError::UnexpectedStructure {
            expected: crate::SyntaxKind::Object,
            found: value_kind(current),
        }),
    }
}

/// 値の種類を、対応する構文の種類に写す（エラー表示用）。
fn value_kind(value: &HjsonValue) -> crate::SyntaxKind {
    match value {
        HjsonValue::Object(_) => crate::SyntaxKind::Object,
        HjsonValue::Array(_) => crate::SyntaxKind::Array,
        _ => crate::SyntaxKind::Value,
    }
}

/// `remove` の意味。空パスは削除できない。
pub(crate) fn remove_path(
    root: &HjsonValue,
    path: &HjsonObjectPath,
) -> Result<HjsonValue, HjsonEditError> {
    if path.depth() == 0 {
        return Err(HjsonEditError::InvalidPath(
            "cannot remove the document body".to_string(),
        ));
    }

    remove_in(root, path.segments(), path)
}

fn remove_in(
    current: &HjsonValue,
    segments: &[PathSegment],
    path: &HjsonObjectPath,
) -> Result<HjsonValue, HjsonEditError> {
    let (first, rest) = segments
        .split_first()
        .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;

    match (current, first) {
        (HjsonValue::Object(entries), PathSegment::Key(key)) => {
            let mut entries = entries.clone();
            let position = entries
                .iter()
                .position(|(existing, _)| existing == key)
                .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;

            if rest.is_empty() {
                entries.remove(position);
            } else {
                entries[position].1 = remove_in(&entries[position].1, rest, path)?;
            }
            Ok(HjsonValue::Object(entries))
        }
        (HjsonValue::Array(items), PathSegment::Array(segment)) => {
            let mut items = items.clone();
            let len = items.len();
            let index = match segment {
                ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => *index,
                ArrayPathSegment::Push => len.saturating_sub(1),
            };
            if index >= len {
                return Err(HjsonEditError::PathNotFound(path.clone()));
            }

            if rest.is_empty() {
                items.remove(index);
            } else {
                items[index] = remove_in(&items[index], rest, path)?;
            }
            Ok(HjsonValue::Array(items))
        }
        _ => Err(HjsonEditError::PathNotFound(path.clone())),
    }
}

/// `merge` の意味。`path` の先にオブジェクトを作り、`source` を深くマージする。
pub(crate) fn merge_at_path(
    root: &HjsonValue,
    path: &HjsonObjectPath,
    source: &HjsonValue,
) -> Result<HjsonValue, HjsonEditError> {
    merge_in(root, path.segments(), path, source)
}

fn merge_in(
    current: &HjsonValue,
    segments: &[PathSegment],
    path: &HjsonObjectPath,
    source: &HjsonValue,
) -> Result<HjsonValue, HjsonEditError> {
    let Some((first, rest)) = segments.split_first() else {
        return merge_objects_in_value(current, source);
    };

    match (current, first) {
        (HjsonValue::Object(entries), PathSegment::Key(key)) => {
            let mut entries = entries.clone();
            match entries.iter_mut().find(|(existing, _)| existing == key) {
                Some(entry) => {
                    entry.1 = merge_in(&entry.1, rest, path, source)?;
                }
                None => {
                    let created = merge_in(&HjsonValue::Object(Vec::new()), rest, path, source)?;
                    entries.push((key.clone(), created));
                }
            }
            Ok(HjsonValue::Object(entries))
        }
        _ => Err(HjsonEditError::MergeConflict(format!(
            "cannot merge: {path:?} is not an object"
        ))),
    }
}

fn merge_objects_in_value(
    target: &HjsonValue,
    source: &HjsonValue,
) -> Result<HjsonValue, HjsonEditError> {
    let (HjsonValue::Object(target_entries), HjsonValue::Object(source_entries)) = (target, source)
    else {
        return Err(HjsonEditError::MergeConflict(
            "merge requires objects".to_string(),
        ));
    };

    let mut entries = target_entries.clone();
    for (key, source_value) in source_entries {
        match entries.iter_mut().find(|(existing, _)| existing == key) {
            Some(entry) => {
                entry.1 = if matches!(entry.1, HjsonValue::Object(_))
                    && matches!(source_value, HjsonValue::Object(_))
                {
                    merge_objects_in_value(&entry.1, source_value)?
                } else {
                    source_value.clone()
                };
            }
            None => entries.push((key.clone(), source_value.clone())),
        }
    }

    Ok(HjsonValue::Object(entries))
}

/// 値の並びから、ネストした値を作る（中間コンテナの作成）。
///
/// 新しく作る配列は長さ 1 なので、位置を指定するセグメントは先頭（`0`）だけを受け付ける。
/// `[1]` のような指定を黙って先頭に丸めず、エラーとして返す。
pub(crate) fn build_nested_value(
    path: &[PathSegment],
    value: HjsonValue,
) -> Result<HjsonValue, HjsonEditError> {
    let mut current = value;
    for segment in path.iter().rev() {
        current = match segment {
            PathSegment::Key(key) => HjsonValue::Object(vec![(key.clone(), current)]),
            PathSegment::Array(segment) => {
                if let Some(index) = segment.position() {
                    if index != 0 {
                        return Err(HjsonEditError::ArrayIndexOutOfBounds { index, len: 0 });
                    }
                }
                HjsonValue::Array(vec![current])
            }
        };
    }

    Ok(current)
}

/// 値の等価判定。整数と浮動小数は数値として比較する。
pub(crate) fn values_equal(left: &HjsonValue, right: &HjsonValue) -> bool {
    match (left, right) {
        (HjsonValue::Integer(left), HjsonValue::Integer(right)) => left == right,
        (HjsonValue::Float(left), HjsonValue::Float(right)) => left == right,
        (HjsonValue::Integer(left), HjsonValue::Float(right))
        | (HjsonValue::Float(right), HjsonValue::Integer(left)) => (*left as f64) == *right,
        (HjsonValue::String(left), HjsonValue::String(right)) => left == right,
        (HjsonValue::Bool(left), HjsonValue::Bool(right)) => left == right,
        (HjsonValue::Null, HjsonValue::Null) => true,
        (HjsonValue::Array(left), HjsonValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| values_equal(left, right))
        }
        (HjsonValue::Object(left), HjsonValue::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|((left_key, left), (right_key, right))| {
                        left_key == right_key && values_equal(left, right)
                    })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> HjsonObjectPath {
        HjsonObjectPath::parse(s.to_string()).unwrap()
    }

    fn object(entries: &[(&str, HjsonValue)]) -> HjsonValue {
        HjsonValue::Object(
            entries
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect(),
        )
    }

    #[test]
    fn set_creates_missing_intermediate_objects() {
        let root = object(&[]);
        let updated = set_path(&root, &path("a.b"), HjsonValue::Integer(1)).unwrap();
        assert_eq!(
            updated,
            object(&[("a", object(&[("b", HjsonValue::Integer(1))]))])
        );
    }

    #[test]
    fn set_creates_missing_intermediate_arrays() {
        let root = object(&[]);
        let updated = set_path(&root, &path("a[0].b"), HjsonValue::Integer(1)).unwrap();
        assert_eq!(
            updated,
            object(&[(
                "a",
                HjsonValue::Array(vec![object(&[("b", HjsonValue::Integer(1))])])
            )])
        );
    }

    #[test]
    fn set_through_missing_index_errors() {
        let root = object(&[("a", HjsonValue::Array(Vec::new()))]);
        let error = set_path(&root, &path("a[0].b"), HjsonValue::Integer(1)).unwrap_err();
        assert!(matches!(
            error,
            HjsonEditError::ArrayIndexOutOfBounds { index: 0, len: 0 }
        ));
    }

    #[test]
    fn set_inserts_and_pushes_into_arrays() {
        let root = HjsonValue::Array(vec![HjsonValue::Integer(1)]);
        let inserted = set_path(&root, &path("[+0]"), HjsonValue::Integer(0)).unwrap();
        assert_eq!(
            inserted,
            HjsonValue::Array(vec![HjsonValue::Integer(0), HjsonValue::Integer(1)])
        );

        let pushed = set_path(&root, &path("[+]"), HjsonValue::Integer(2)).unwrap();
        assert_eq!(
            pushed,
            HjsonValue::Array(vec![HjsonValue::Integer(1), HjsonValue::Integer(2)])
        );
    }

    #[test]
    fn remove_deletes_member_and_element() {
        let root = object(&[
            ("a", HjsonValue::Integer(1)),
            ("b", HjsonValue::Array(vec![HjsonValue::Integer(2)])),
        ]);
        let removed = remove_path(&root, &path("b[0]")).unwrap();
        assert_eq!(
            removed,
            object(&[
                ("a", HjsonValue::Integer(1)),
                ("b", HjsonValue::Array(vec![]))
            ])
        );

        let removed = remove_path(&root, &path("a")).unwrap();
        assert_eq!(
            removed,
            object(&[("b", HjsonValue::Array(vec![HjsonValue::Integer(2)]))])
        );
    }

    #[test]
    fn merge_deep_merges_and_replaces_scalars() {
        let target = object(&[("a", object(&[("x", HjsonValue::Integer(1))]))]);
        let source = object(&[
            ("a", object(&[("y", HjsonValue::Integer(2))])),
            ("b", HjsonValue::Integer(3)),
        ]);
        let merged = merge_at_path(&target, &path(""), &source).unwrap();
        assert_eq!(
            merged,
            object(&[
                (
                    "a",
                    object(&[("x", HjsonValue::Integer(1)), ("y", HjsonValue::Integer(2))])
                ),
                ("b", HjsonValue::Integer(3)),
            ])
        );
    }

    #[test]
    fn values_equal_compares_numbers_by_value() {
        assert!(values_equal(
            &HjsonValue::Integer(2),
            &HjsonValue::Float(2.0)
        ));
        assert!(values_equal(
            &HjsonValue::Float(2.5),
            &HjsonValue::Float(2.5)
        ));
        assert!(!values_equal(
            &HjsonValue::Integer(2),
            &HjsonValue::Float(2.5)
        ));
        assert!(!values_equal(
            &HjsonValue::Integer(2),
            &HjsonValue::String("2".to_string())
        ));
    }
}
