use crate::edit::high_level::{extract_key_text, HjsonEditError, PathSegment};
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeExt};

pub(crate) fn parse_dot_path(s: &str) -> Result<Vec<PathSegment>, HjsonEditError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        if chars[pos] == '.' {
            return Err(HjsonEditError::InvalidPath {
                path: s.to_string(),
            });
        }

        if chars[pos] == '[' {
            let (index, next_pos) = parse_index_segment(&chars, pos, s)?;
            segments.push(PathSegment::Index(index));
            pos = next_pos;
        } else {
            let start = pos;
            while pos < chars.len() && chars[pos] != '.' && chars[pos] != '[' {
                if chars[pos] == ']' {
                    return Err(HjsonEditError::InvalidPath {
                        path: s.to_string(),
                    });
                }
                pos += 1;
            }

            if start == pos {
                return Err(HjsonEditError::InvalidPath {
                    path: s.to_string(),
                });
            }

            segments.push(PathSegment::Key(chars[start..pos].iter().collect()));

            while pos < chars.len() && chars[pos] == '[' {
                let (index, next_pos) = parse_index_segment(&chars, pos, s)?;
                segments.push(PathSegment::Index(index));
                pos = next_pos;
            }
        }

        if pos == chars.len() {
            break;
        }

        if chars[pos] != '.' {
            return Err(HjsonEditError::InvalidPath {
                path: s.to_string(),
            });
        }

        pos += 1;
        if pos == chars.len() {
            return Err(HjsonEditError::InvalidPath {
                path: s.to_string(),
            });
        }
    }

    Ok(segments)
}

fn parse_index_segment(
    chars: &[char],
    start: usize,
    raw: &str,
) -> Result<(usize, usize), HjsonEditError> {
    let mut pos = start + 1;
    let digit_start = pos;

    while pos < chars.len() && chars[pos].is_ascii_digit() {
        pos += 1;
    }

    if digit_start == pos || pos >= chars.len() || chars[pos] != ']' {
        return Err(HjsonEditError::InvalidPath {
            path: raw.to_string(),
        });
    }

    let index = chars[digit_start..pos]
        .iter()
        .collect::<String>()
        .parse::<usize>()
        .map_err(|_| HjsonEditError::InvalidPath {
            path: raw.to_string(),
        })?;

    Ok((index, pos + 1))
}

pub(crate) fn parse_path_for_format(s: &str) -> Result<Vec<PathSegment>, HjsonEditError> {
    parse_dot_path(s)
}

pub(crate) fn navigate_from_node(start: &RedNode, path: &[PathSegment]) -> Option<RedNode> {
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

pub(crate) fn resolve_object_child(node: &RedNode) -> Option<RedNode> {
    node.child_by_kind(SyntaxKind::Object).or_else(|| {
        node.child_by_kind(SyntaxKind::Value)?
            .child_by_kind(SyntaxKind::Object)
    })
}

pub(crate) fn resolve_array_child(node: &RedNode) -> Option<RedNode> {
    node.child_by_kind(SyntaxKind::Array).or_else(|| {
        node.child_by_kind(SyntaxKind::Value)?
            .child_by_kind(SyntaxKind::Array)
    })
}

pub(crate) fn find_pair_index_for_comments(obj: &RedNode, key: &str) -> Option<usize> {
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

pub(crate) fn find_pair_node_for_comments(obj: &RedNode, key: &str) -> Option<RedNode> {
    for child in obj.children_nodes() {
        if child.kind() == SyntaxKind::Pair {
            if let Some(key_node) = child.child_by_kind(SyntaxKind::Key) {
                if let Some(key_text) = extract_key_text(&key_node) {
                    if key_text == key {
                        return Some(child);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dot_path() {
        assert_eq!(
            parse_dot_path("server.host").unwrap(),
            vec![
                PathSegment::Key("server".to_string()),
                PathSegment::Key("host".to_string())
            ]
        );

        assert_eq!(
            parse_dot_path("items[0].name").unwrap(),
            vec![
                PathSegment::Key("items".to_string()),
                PathSegment::Index(0),
                PathSegment::Key("name".to_string())
            ]
        );

        assert!(matches!(
            parse_dot_path("items[abc]"),
            Err(HjsonEditError::InvalidPath { .. })
        ));
        assert!(matches!(
            parse_dot_path("items[1x]"),
            Err(HjsonEditError::InvalidPath { .. })
        ));
    }
}
