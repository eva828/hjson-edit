use crate::{
    ast::{AstNode, Key, Member},
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
        let mut current = self.normalized_start();
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
        let mut current = self.normalized_start();
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

    /// comment API 用に、path が属する Member を返す。
    /// 空 path は comment target として無効（InvalidPath）。
    pub(crate) fn resolve_member(&self, path: &HjsonObjectPath) -> Result<Member, HjsonEditError> {
        if path.depth() == 0 {
            return Err(HjsonEditError::InvalidPath("empty path".to_string()));
        }
        let target = self.resolve_node(path)?;
        let mut current = target;
        while current.kind() != SyntaxKind::Member {
            current = current.parent().ok_or(HjsonEditError::ParentNotFound)?;
        }
        Member::cast(current)
    }

    /// 解決開始点を正規化する。
    /// Root が渡された場合は document body の先頭非 trivia node を開始点にする。
    /// Object / Array / Value が渡された場合はその node 自体を開始点にする。
    /// 空 document など body が見つからない場合は Root 自身を fallback にする。
    fn normalized_start(&self) -> RedNode {
        match self.root.kind() {
            SyntaxKind::Root => self
                .root
                .children()
                .find(|child| !child.kind().is_trivia())
                .unwrap_or_else(|| self.root.clone()),
            _ => self.root.clone(),
        }
    }

    /// 1 segment 解決ルールの集約。
    fn resolve_one(&self, node: &RedNode, segment: &PathSegment) -> Option<RedNode> {
        match node.kind() {
            SyntaxKind::Root | SyntaxKind::Value => {
                let child = node.children().find(|child| !child.kind().is_trivia())?;
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
        let root = parse("{\n  server: { host: localhost }\n}\n");
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
        let root = parse("{\n  server: { host: localhost }\n}\n");
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
        let root = parse("{\n  server: { host: localhost }\n}\n");
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
        let root = parse("{\n  server: { host: localhost }\n}\n");
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
        let root = parse("{\n  items: [{ name: a }, { name: b }]\n}\n");
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
    // resolve_member
    // ------------------------------------------------------------------

    #[test]
    fn resolve_member_simple_key() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let member = resolver
            .resolve_member(&HjsonObjectPath::parse("name").unwrap())
            .unwrap();
        assert_eq!(member.syntax().kind(), SyntaxKind::Member);
    }

    #[test]
    fn resolve_member_nested() {
        let root = parse("{\n  server: { host: localhost }\n}\n");
        let resolver = PathResolver::new(root);
        let member = resolver
            .resolve_member(&HjsonObjectPath::parse("server.host").unwrap())
            .unwrap();
        assert_eq!(member.syntax().kind(), SyntaxKind::Member);
        assert_eq!(member.member_key(), Some("host".to_string()));
    }

    #[test]
    fn resolve_member_array_then_key() {
        let root = parse("{\n  items: [{ name: a }]\n}\n");
        let resolver = PathResolver::new(root);
        let member = resolver
            .resolve_member(&HjsonObjectPath::parse("items[0].name").unwrap())
            .unwrap();
        assert_eq!(member.syntax().kind(), SyntaxKind::Member);
        assert_eq!(member.member_key(), Some("name".to_string()));
    }

    #[test]
    fn resolve_member_empty_path_is_invalid() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_member(&HjsonObjectPath::parse("").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::InvalidPath(_)));
    }

    #[test]
    fn resolve_member_not_found() {
        let root = parse("{\n  name: app\n}\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_member(&HjsonObjectPath::parse("missing").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::PathNotFound(_)));
    }

    #[test]
    fn resolve_member_root_scalar_is_not_member() {
        let root = parse("123\n");
        let resolver = PathResolver::new(root);
        let err = resolver
            .resolve_member(&HjsonObjectPath::parse("").unwrap())
            .unwrap_err();
        assert!(matches!(err, HjsonEditError::InvalidPath(_)));
    }
}
