//! 部分整形: 文書全体ではなく、指定したコンテナ（オブジェクト・配列）だけを整形する。
//!
//! 編集の核心（`set`/`remove` など）は最小限のテキスト変更に留め、インデントなどの
//! 体裁はこの層で整える。これにより「構造編集と整形を分離する」という設計を保つ。

use crate::ast::{Array as AstArray, AstNode, DelimiterStyle, Object as AstObject, Value};
use crate::format::value::multiline_block_node;
use crate::format::{Format, FormatConfig};
use crate::high_level::error::HjsonEditError;
use crate::high_level::indent::{is_at_line_start, line_indent};
use crate::high_level::ops::receiving_container_depth;
use crate::high_level::path::{HjsonObjectPath, PathSegment};
use crate::high_level::resolve::{array_slot_index, member_index, resolve_body, PathResolver};
use crate::high_level::style::{detect_indent_from_ast, detect_newline_from_ast};
use crate::high_level::verify;
use crate::high_level::HjsonDocument;
use crate::syntax::{RedNode, SyntaxKind};

impl HjsonDocument {
    /// 文書で使われている改行コードとインデントを推定した整形設定を返す。
    ///
    /// インデントを判定できない文書（1段もインデントされていない）では
    /// `IndentStyle::default()`（2スペース）を返す。
    pub fn detected_format_config(&self) -> FormatConfig {
        let root: &RedNode = self.syntax_root();
        FormatConfig::default()
            .with_newline(detect_newline_from_ast(root).as_str())
            .with_indent(detect_indent_from_ast(root))
    }

    /// 指定したパスのコンテナだけを整形した文書を返す。
    ///
    /// 空のパスは文書全体を整形する。整形の深さは文書内の位置から決まるため、
    /// 呼び出し側で深さを指定する必要はない。
    ///
    /// # エラー
    ///
    /// - パスが存在しない場合は [`HjsonEditError::PathNotFound`]
    /// - パスが指す先がコンテナでない場合は [`HjsonEditError::UnexpectedStructure`]
    pub fn format_container(
        &self,
        path: &HjsonObjectPath,
        config: &FormatConfig,
    ) -> Result<Self, HjsonEditError> {
        // 文書全体は文書のトリビア（前後の改行）も含めて整形する
        if path.depth() == 0 {
            let ast = self.ast()?;
            let document = self.clone().root(ast.format(config));
            if self.current_verify() {
                verify::values_unchanged(self.syntax_root(), document.syntax_root())?;
            }

            return Ok(document);
        }

        let resolved = PathResolver::new(self.syntax_root().clone()).resolve_node(path)?;
        let container = match resolved.kind() {
            SyntaxKind::Object | SyntaxKind::Array => resolved,
            found => resolve_body(&resolved).map_err(|_| HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found,
            })?,
        };
        // 行の途中にあるコンテナを単独で整形すると、親のインライン配置と混ざって崩れる。
        // 行頭になる祖先コンテナまで遡って整形する。
        let container = line_aligned_container(self.syntax_root(), &container);

        let depth = format_depth(&container)?;
        let formatted = match container.kind() {
            SyntaxKind::Object => AstObject::cast(container.clone())?.format_at(config, depth),
            SyntaxKind::Array => AstArray::cast(container.clone())?.format_at(config, depth),
            found => {
                return Err(HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Object,
                    found,
                })
            }
        };

        let document = self.clone().root(container.replace_with(formatted));
        if self.current_verify() {
            verify::values_unchanged(self.syntax_root(), document.syntax_root())?;
        }

        Ok(document)
    }

    /// 指定したパスの要素（メンバーまたは配列要素）の中で、複数行の Block 文字列を
    /// その位置の字下げに合わせて書き直した文書を返す。
    ///
    /// 値は変えない。行頭にない要素と、複数行の Block 文字列を持たない要素は
    /// そのまま返す（インラインの配置を崩さない）。
    ///
    /// # エラー
    ///
    /// - パスが存在しない場合は [`HjsonEditError::PathNotFound`]
    /// - パスが空の場合は [`HjsonEditError::InvalidPath`]
    pub fn format_slot(
        &self,
        path: &HjsonObjectPath,
        config: &FormatConfig,
    ) -> Result<Self, HjsonEditError> {
        let (parent_path, last) = path.split_last().ok_or_else(|| {
            HjsonEditError::InvalidPath("format_slot needs an element path".to_string())
        })?;
        let parent = PathResolver::new(self.syntax_root().clone()).resolve_node(&parent_path)?;
        let container = resolve_body(&parent)?;
        let element = match (container.kind(), &last) {
            (SyntaxKind::Object, PathSegment::Key(key)) => {
                let index = member_index(&container, key)
                    .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;
                container
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Member)
                    .nth(index)
                    .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?
            }
            (SyntaxKind::Array, PathSegment::Array(segment)) => {
                let index = array_slot_index(&container, segment)
                    .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;
                container
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Value)
                    .nth(index)
                    .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?
            }
            _ => {
                return Err(HjsonEditError::InvalidTarget {
                    path: path.clone(),
                    reason: "the parent is not a container".to_string(),
                })
            }
        };

        let root = self.syntax_root();
        let start: usize = element.text_range().start().into();
        // 行頭にない要素は、複数行にすると親のレイアウトと混ざるため触らない
        if !is_at_line_start(root, start) {
            return Ok(self.clone());
        }

        let indent = line_indent(root, start);
        let Some(value) = element
            .children()
            .find(|child| child.kind() == SyntaxKind::Value)
        else {
            return Ok(self.clone());
        };
        let Some(formatted) = multiline_block_node(
            &Value::cast(value.clone())?,
            &format!("{indent}{}", crate::high_level::indent::indent_unit(root)),
            start + prefix_width(&element, &value),
            &indent,
            config.newline(),
        ) else {
            return Ok(self.clone());
        };

        let value_index = value.index();
        let updated = element
            .green()
            .splice_children(value_index..value_index + 1, vec![formatted.into()]);
        let document = self.clone().root(element.replace_with(updated));
        if self.current_verify() {
            verify::values_unchanged(self.syntax_root(), document.syntax_root())?;
        }

        Ok(document)
    }
}

/// 要素の中で、`value` より前にある子要素の幅（`value` の開始桁を求めるのに使う）。
fn prefix_width(element: &RedNode, value: &RedNode) -> usize {
    element
        .children_with_tokens()
        .take_while(|child| child.text_range().start() < value.text_range().start())
        .map(|child| match child {
            rowan::NodeOrToken::Node(node) => node.text().to_string().chars().count(),
            rowan::NodeOrToken::Token(token) => token.text().to_string().chars().count(),
        })
        .sum()
}

/// 行頭から始まる要素に属する、最も内側のコンテナ。
///
/// インラインの配列・オブジェクトの中にあるコンテナは、それだけを整形すると
/// 親のレイアウトと混ざるため、行頭の要素に属する祖先まで遡る。
fn line_aligned_container(root: &RedNode, container: &RedNode) -> RedNode {
    let mut target = container.clone();
    while let Some(element) = containing_element(&target) {
        if is_at_line_start(root, element.text_range().start().into()) {
            break;
        }

        let Some(parent) = target
            .ancestors()
            .skip(1)
            .find(|node| matches!(node.kind(), SyntaxKind::Object | SyntaxKind::Array))
        else {
            break;
        };
        target = parent;
    }

    target
}

/// 子孫のコンテナを含む要素（メンバー、または配列の要素）。
fn containing_element(node: &RedNode) -> Option<RedNode> {
    for ancestor in node.ancestors().skip(1) {
        match ancestor.kind() {
            SyntaxKind::Member => return Some(ancestor),
            SyntaxKind::Value
                if ancestor
                    .parent()
                    .is_some_and(|parent| parent.kind() == SyntaxKind::Array) =>
            {
                return Some(ancestor)
            }
            _ => {}
        }
    }

    None
}

/// コンテナを整形する際の深さ。文書全体の整形（`Document::format_at`）と同じ値になる。
///
/// ブレースレスオブジェクトは文書直下にしか現れず、深さ 0 で整形される。
fn format_depth(container: &RedNode) -> Result<usize, HjsonEditError> {
    let is_document_body = container
        .parent()
        .is_some_and(|parent| parent.kind() == SyntaxKind::Root);

    if is_document_body && container.kind() == SyntaxKind::Object {
        let object = AstObject::cast(container.clone())?;
        if object.delimiter_style == DelimiterStyle::Braceless {
            return Ok(0);
        }
    }

    Ok(receiving_container_depth(container))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> HjsonObjectPath {
        HjsonObjectPath::parse(raw).unwrap()
    }

    fn document_format(doc: &HjsonDocument, config: &FormatConfig) -> String {
        RedNode::new_root(doc.ast().unwrap().format(config))
            .text()
            .to_string()
    }

    #[test]
    fn detected_format_config_keeps_existing_style() {
        let input = "{\r\n    a: 1\r\n    b: {\r\n        c: 1\r\n    }\r\n}\r\n";
        let doc = HjsonDocument::parse(input).unwrap();
        let config = doc.detected_format_config();
        assert_eq!(document_format(&doc, &config), input);
    }

    #[test]
    fn detected_format_config_finds_tabs() {
        let input = "{\n\ta: 1\n\tb: {\n\t\tc: 1\n\t}\n}\n";
        let doc = HjsonDocument::parse(input).unwrap();
        let config = doc.detected_format_config();
        assert_eq!(document_format(&doc, &config), input);
    }

    #[test]
    fn detected_format_config_without_indent_uses_default() {
        let doc = HjsonDocument::parse("a: 1\n").unwrap();
        let config = doc.detected_format_config();
        assert_eq!(document_format(&doc, &config), "a: 1\n");
    }

    #[test]
    fn format_root_container_matches_document_format() {
        let doc = HjsonDocument::parse("{\n  a: 1\n  b: {c:2}\n}\n").unwrap();
        let config = FormatConfig::default();
        let formatted = doc.format_container(&path(""), &config).unwrap();
        assert_eq!(
            formatted.to_string(),
            document_format(&doc, &config),
            "ルートコンテナの整形は文書全体の整形と一致する"
        );
    }

    #[test]
    fn format_container_leaves_siblings_untouched() {
        let doc = HjsonDocument::parse("{\n  o: {a:1}\n  keep:   [1,2]\n}\n").unwrap();
        let config = doc.detected_format_config();
        let formatted = doc.format_container(&path("o"), &config).unwrap();
        assert_eq!(
            formatted.to_string(),
            "{\n  o: {\n    a: 1\n  }\n  keep:   [1,2]\n}\n"
        );
    }

    #[test]
    fn format_container_handles_braceless_root() {
        let input = "a: 1\nb: {\n  c: 1\n}\n";
        let doc = HjsonDocument::parse(input).unwrap();
        let config = doc.detected_format_config();
        let formatted = doc.format_container(&path(""), &config).unwrap();
        assert_eq!(formatted.to_string(), input);
        assert_eq!(formatted.to_string(), document_format(&doc, &config));
    }

    #[test]
    fn format_container_handles_comment_only_document() {
        let doc = HjsonDocument::parse("// just a comment\n").unwrap();
        let config = doc.detected_format_config();
        let formatted = doc.format_container(&path(""), &config).unwrap();
        assert_eq!(formatted.to_string(), "// just a comment\n");
    }

    #[test]
    fn format_container_handles_root_array() {
        let doc = HjsonDocument::parse("[1,{a:2}]\n").unwrap();
        let config = FormatConfig::default();
        let formatted = doc.format_container(&path(""), &config).unwrap();
        assert_eq!(formatted.to_string(), "[\n  1\n  {\n    a: 2\n  }\n]\n");
    }

    #[test]
    fn format_container_rejects_missing_path() {
        let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
        let result = doc.format_container(&path("missing"), &FormatConfig::default());
        assert!(matches!(result, Err(HjsonEditError::PathNotFound(_))));
    }

    #[test]
    fn format_container_rejects_non_container() {
        let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
        let result = doc.format_container(&path("a"), &FormatConfig::default());
        assert!(matches!(
            result,
            Err(HjsonEditError::UnexpectedStructure { .. })
        ));
    }
}
