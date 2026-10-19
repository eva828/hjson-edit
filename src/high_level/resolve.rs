use crate::{
    ast::{Array as AstArray, AstNode, Key, Object as AstObject},
    high_level::{
        error::HjsonEditError,
        path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
    },
    RedNode, SyntaxKind,
};

#[derive(Debug, Clone)]
pub(crate) struct ParentResolution {
    pub parent: RedNode,
    pub last: PathSegment,
}

#[derive(Debug, Clone)]
pub(crate) struct ExistingPrefix {
    pub ancestor: RedNode,
    pub consumed: usize,
}

/// パスの末端が指す、コンテナ内の要素位置。
///
/// メンバー（オブジェクト）でも要素（配列）でも同じ形で扱うため、編集とコメント操作は
/// この位置を経由して対象を決める。
#[derive(Debug, Clone)]
pub(crate) struct Slot {
    container: RedNode,
    index: usize,
}

impl Slot {
    /// 対象を含むコンテナ（`Object` または `Array`）。
    pub(crate) fn container(&self) -> &RedNode {
        &self.container
    }

    /// コンテナ内での要素 index。
    pub(crate) fn index(&self) -> usize {
        self.index
    }
}

/// オブジェクト内のメンバー位置を key で探す。
///
/// 比較は key の raw text ではなく body text（引用符を外した中身）で行う。
pub(crate) fn member_index(container: &RedNode, key: &str) -> Option<usize> {
    let object = AstObject::cast(container.clone()).ok()?;
    object
        .members()
        .iter()
        .position(|member| member.member_key().as_deref() == Some(key))
}

/// 配列内でセグメントが指す要素位置。
///
/// 読み取り位置は `Index(n)` / `InsertAt(n)` が `n`、`Push` が末尾要素。
/// 範囲外のときは `None`。
pub(crate) fn array_slot_index(container: &RedNode, segment: &ArrayPathSegment) -> Option<usize> {
    let array = AstArray::cast(container.clone()).ok()?;
    let len = array.slots().len();
    match segment {
        ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => {
            (*index < len).then_some(*index)
        }
        ArrayPathSegment::Push => len.checked_sub(1),
    }
}

/// Root または Value ノードから、実質的な中身のノードを取り出す。
/// Root に有効な子がない場合と、Value に中身がない場合はエラーを返す。
pub(crate) fn resolve_body(node: &RedNode) -> Result<RedNode, HjsonEditError> {
    match node.kind() {
        SyntaxKind::Root => node
            .children()
            .find(|child| {
                matches!(
                    child.kind(),
                    SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value
                )
            })
            .ok_or(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: node.kind(),
            }),
        SyntaxKind::Value => node
            .children()
            .find(|child| !child.kind().is_trivia())
            .ok_or(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: node.kind(),
            }),
        _ => Ok(node.clone()),
    }
}

pub(crate) struct PathResolver {
    root: RedNode,
}

impl PathResolver {
    pub(crate) fn new(root: RedNode) -> Self {
        Self { root }
    }

    /// path 全体を解決して最終 node を返す。
    /// 空 path は document body 自体を指す。
    pub(crate) fn resolve_node(&self, path: &HjsonObjectPath) -> Result<RedNode, HjsonEditError> {
        let mut current = resolve_body(&self.root)?;
        for segment in path.segments() {
            current = self
                .resolve_one(&current, segment)
                .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;
        }
        Ok(current)
    }

    /// set / remove 用に、親 node と最後の segment を返す。
    /// 空 path は親がないため InvalidPath を返す。
    pub(crate) fn resolve_parent(
        &self,
        path: &HjsonObjectPath,
    ) -> Result<ParentResolution, HjsonEditError> {
        let (parent_path, last) = path
            .split_last()
            .ok_or_else(|| HjsonEditError::InvalidPath("empty path".to_string()))?;
        let parent = self.resolve_node(&parent_path)?;
        Ok(ParentResolution { parent, last })
    }

    /// set で missing parent を作るために、既に存在する最長 prefix を返す。
    /// 基本的に error を返さず、どこまで解決できたかを返す。
    pub(crate) fn resolve_existing_prefix(&self, path: &HjsonObjectPath) -> ExistingPrefix {
        let mut current = resolve_body(&self.root).unwrap_or_else(|_| self.root.clone());
        let mut consumed = 0usize;

        for segment in path.segments() {
            match self.resolve_one(&current, segment) {
                Some(next) => {
                    current = next;
                    consumed += 1;
                }
                None => break,
            }
        }

        ExistingPrefix {
            ancestor: current,
            consumed,
        }
    }

    /// comment API 用に、path が指すコンテナ内の要素位置を返す。
    ///
    /// メンバー（オブジェクト）でも要素（配列）でも同じ形で扱う。
    /// 空 path（文書全体）やスカラーは要素を持たないため `InvalidTarget` を返す。
    pub(crate) fn resolve_slot(&self, path: &HjsonObjectPath) -> Result<Slot, HjsonEditError> {
        if path.depth() == 0 {
            return Err(HjsonEditError::InvalidTarget {
                path: path.clone(),
                reason: "the document body is not an element of a container".to_string(),
            });
        }

        let ParentResolution { parent, last } = self.resolve_parent(path)?;
        let container = resolve_body(&parent).map_err(|_| HjsonEditError::InvalidTarget {
            path: path.clone(),
            reason: "the path points into a scalar value".to_string(),
        })?;

        let index = match (container.kind(), &last) {
            (SyntaxKind::Object, PathSegment::Key(key)) => member_index(&container, key),
            (SyntaxKind::Array, PathSegment::Array(segment)) => {
                array_slot_index(&container, segment)
            }
            _ => None,
        }
        .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;

        Ok(Slot { container, index })
    }

    /// 1 segment 解決ルールの集約。
    fn resolve_one(&self, node: &RedNode, segment: &PathSegment) -> Option<RedNode> {
        match node.kind() {
            SyntaxKind::Root | SyntaxKind::Value => {
                let child = resolve_body(node).ok()?;
                self.resolve_one(&child, segment)
            }
            SyntaxKind::Object => match segment {
                PathSegment::Key(key) => node
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Member)
                    .find(|member| self.match_key(member, key))
                    .and_then(|member| member.children().find(|c| c.kind() == SyntaxKind::Value)),
                _ => None,
            },
            SyntaxKind::Array => match segment {
                PathSegment::Array(seg) => {
                    let mut values = node
                        .children()
                        .filter(|child| child.kind() == SyntaxKind::Value);
                    match seg {
                        ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => {
                            values.nth(*index)
                        }
                        ArrayPathSegment::Push => values.last(),
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Object member の key matching。
    /// key の raw text ではなく body text で比較する。
    fn match_key(&self, member: &RedNode, key: &str) -> bool {
        member
            .children()
            .find(|child| child.kind() == SyntaxKind::Key)
            .and_then(|key_node| Key::cast(key_node).ok())
            .and_then(|k| k.body_text())
            .is_some_and(|member_key| member_key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{high_level::path::HjsonObjectPath, HjsonDocument};

    fn parse(input: &str) -> RedNode {
        HjsonDocument::parse(input).unwrap().syntax_root().clone()
    }

    // ------------------------------------------------------------------
    // resolve_node
    // ------------------------------------------------------------------

    #[test]
    fn resolve_node_empty_path_returns_body() {
        let root = parse("name: app\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Object);
    }

    #[test]
    fn resolve_node_simple_key() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("name").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_node_nested_key() {
        let root = parse("{\n  server: { host: localhost\n}\n}\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("server.host").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_node_array_index() {
        let root = parse("{\n  items: [1, 2, 3]\n}\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("items[0]").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_node_array_push_reads_last() {
        let root = parse("{\n  items: [1, 2, 3]\n}\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("items[-]").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
        assert_eq!(node.to_string(), "3");
    }

    #[test]
    fn resolve_node_array_insert_at_reads_as_index() {
        let root = parse("{\n  items: [1, 2, 3]\n}\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("items[+0]").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
        assert_eq!(node.to_string(), "1");
    }

    #[test]
    fn resolve_node_not_found_missing_key() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_node(&HjsonObjectPath::parse("missing").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::PathNotFound(_)));
    }

    #[test]
    fn resolve_node_not_found_out_of_range_index() {
        let root = parse("{\n  items: [1]\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_node(&HjsonObjectPath::parse("items[10]").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::PathNotFound(_)));
    }

    #[test]
    fn resolve_node_not_found_array_segment_on_object() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_node(&HjsonObjectPath::parse("name[0]").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::PathNotFound(_)));
    }

    #[test]
    fn resolve_node_not_found_key_segment_on_array() {
        let root = parse("[1, 2, 3]\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_node(&HjsonObjectPath::parse("name").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::PathNotFound(_)));
    }

    #[test]
    fn resolve_node_root_array() {
        let root = parse("[1, 2, 3]\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("[1]").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
        assert_eq!(node.to_string(), "2");
    }

    #[test]
    fn resolve_node_root_scalar() {
        let root = parse("123\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_node_braceless_root_object() {
        let root = parse("name: app\nport: 8080\n");
        let resolver = PathResolver::new(root);
        let node = resolver
            .resolve_node(&HjsonObjectPath::parse("port").unwrap())
            .unwrap();
        assert_eq!(node.kind(), SyntaxKind::Value);
        assert_eq!(node.to_string(), "8080");
    }

    // ------------------------------------------------------------------
    // resolve_parent
    // ------------------------------------------------------------------

    #[test]
    fn resolve_parent_nested_object() {
        let root = parse("{\n  server: { host: localhost\n}\n}\n");
        let resolver = PathResolver::new(root);
        let res = resolver
            .resolve_parent(&HjsonObjectPath::parse("server.host").unwrap())
            .unwrap();
        // "server" resolves to the Value node wrapping the inner object
        assert_eq!(res.parent.kind(), SyntaxKind::Value);
        assert_eq!(res.last, PathSegment::Key("host".to_string()));
    }

    #[test]
    fn resolve_parent_array_element() {
        let root = parse("{\n  items: [1, 2, 3]\n}\n");
        let resolver = PathResolver::new(root);
        let res = resolver
            .resolve_parent(&HjsonObjectPath::parse("items[0]").unwrap())
            .unwrap();
        // "items" resolves to the Value node wrapping the array
        assert_eq!(res.parent.kind(), SyntaxKind::Value);
        assert_eq!(res.last, PathSegment::Array(ArrayPathSegment::Index(0)));
    }

    #[test]
    fn resolve_parent_empty_path_is_invalid() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_parent(&HjsonObjectPath::parse("").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::InvalidPath(_)));
    }

    #[test]
    fn resolve_parent_partial_ok() {
        let root = parse("{\n  server: { host: localhost\n}\n}\n");
        let resolver = PathResolver::new(root);
        // parent path "server" resolves fine; only the final segment is "missing"
        let res = resolver
            .resolve_parent(&HjsonObjectPath::parse("server.missing").unwrap())
            .unwrap();
        assert_eq!(res.parent.kind(), SyntaxKind::Value);
        assert_eq!(res.last, PathSegment::Key("missing".to_string()));
    }

    // ------------------------------------------------------------------
    // resolve_existing_prefix
    // ------------------------------------------------------------------

    #[test]
    fn resolve_existing_prefix_fully_resolved() {
        let root = parse("{\n  server: { host: localhost\n}\n}\n");
        let resolver = PathResolver::new(root);
        let prefix =
            resolver.resolve_existing_prefix(&HjsonObjectPath::parse("server.host").unwrap());
        assert_eq!(prefix.consumed, 2);
        assert_eq!(prefix.ancestor.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_existing_prefix_partial() {
        let root = parse("{\n  root: { }\n}\n");
        let resolver = PathResolver::new(root);
        let prefix =
            resolver.resolve_existing_prefix(&HjsonObjectPath::parse("root.new.nested").unwrap());
        assert_eq!(prefix.consumed, 1);
        // "root" resolves to the Value node wrapping the inner object
        assert_eq!(prefix.ancestor.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_existing_prefix_none() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let prefix =
            resolver.resolve_existing_prefix(&HjsonObjectPath::parse("missing.path").unwrap());
        assert_eq!(prefix.consumed, 0);
        // empty prefix falls back to the document body (Value wrapping the object)
        assert_eq!(prefix.ancestor.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_existing_prefix_array_then_key() {
        let root = parse("{\n  items: [{ name: a\n}, { name: b\n}]\n}\n");
        let resolver = PathResolver::new(root);
        let prefix =
            resolver.resolve_existing_prefix(&HjsonObjectPath::parse("items[0].name").unwrap());
        // items -> Value(Array) -> [0] -> Value(Object) -> name -> Value
        assert_eq!(prefix.consumed, 3);
        assert_eq!(prefix.ancestor.kind(), SyntaxKind::Value);
    }

    #[test]
    fn resolve_existing_prefix_array_insert_at() {
        let root = parse("{\n  items: [1, 2]\n}\n");
        let resolver = PathResolver::new(root);
        let prefix =
            resolver.resolve_existing_prefix(&HjsonObjectPath::parse("items[+3].name").unwrap());
        // InsertAt(3) on a 2-element array resolves as Index(3) which is out of range,
        // so only "items" is consumed.
        assert_eq!(prefix.consumed, 1);
        // "items" resolves to the Value node wrapping the array
        assert_eq!(prefix.ancestor.kind(), SyntaxKind::Value);
    }

    // ------------------------------------------------------------------
    // resolve_slot
    // ------------------------------------------------------------------

    #[test]
    fn resolve_slot_simple_key() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let slot = resolver
            .resolve_slot(&HjsonObjectPath::parse("name").unwrap())
            .unwrap();
        assert_eq!(slot.container().kind(), SyntaxKind::Object);
        assert_eq!(slot.index(), 0);
    }

    #[test]
    fn resolve_slot_nested_key() {
        let root = parse("{\n  server: { host: localhost\n}\n}\n");
        let resolver = PathResolver::new(root);
        let slot = resolver
            .resolve_slot(&HjsonObjectPath::parse("server.host").unwrap())
            .unwrap();
        assert_eq!(slot.container().kind(), SyntaxKind::Object);
        assert_eq!(slot.index(), 0);
    }

    #[test]
    fn resolve_slot_array_element() {
        let root = parse("{\n  items: [1, 2]\n}\n");
        let resolver = PathResolver::new(root);
        let slot = resolver
            .resolve_slot(&HjsonObjectPath::parse("items[1]").unwrap())
            .unwrap();
        assert_eq!(slot.container().kind(), SyntaxKind::Array);
        assert_eq!(slot.index(), 1);
    }

    #[test]
    fn resolve_slot_root_array_element() {
        let root = parse("[1, 2, 3]\n");
        let resolver = PathResolver::new(root);
        let slot = resolver
            .resolve_slot(&HjsonObjectPath::parse("[2]").unwrap())
            .unwrap();
        assert_eq!(slot.container().kind(), SyntaxKind::Array);
        assert_eq!(slot.index(), 2);
    }

    #[test]
    fn resolve_slot_array_push_is_last_element() {
        let root = parse("{\n  items: [1, 2, 3]\n}\n");
        let resolver = PathResolver::new(root);
        let slot = resolver
            .resolve_slot(&HjsonObjectPath::parse("items[+]").unwrap())
            .unwrap();
        assert_eq!(slot.index(), 2);
    }

    #[test]
    fn resolve_slot_member_inside_array_element() {
        let root = parse("{\n  items: [{ name: a\n}]\n}\n");
        let resolver = PathResolver::new(root);
        let slot = resolver
            .resolve_slot(&HjsonObjectPath::parse("items[0].name").unwrap())
            .unwrap();
        assert_eq!(slot.container().kind(), SyntaxKind::Object);
        assert_eq!(slot.index(), 0);
    }

    #[test]
    fn resolve_slot_empty_path_is_invalid_target() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_slot(&HjsonObjectPath::parse("").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::InvalidTarget { .. }));
    }

    #[test]
    fn resolve_slot_scalar_root_is_invalid_target() {
        let root = parse("123\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_slot(&HjsonObjectPath::parse("name").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::InvalidTarget { .. }));
    }

    #[test]
    fn resolve_slot_not_found() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_slot(&HjsonObjectPath::parse("missing").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::PathNotFound(_)));
    }
}
