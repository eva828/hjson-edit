use crate::document_style::{DetectNewline, DocumentStyle};

use crate::edit::high_level::navigation::RedNodeNavigateExt;
use crate::edit::high_level::{
    clone_element, contextual_newline_kind, create_multiline_string_literal, is_token_of_kind,
    FormatError, HjsonDocument, StandaloneTriviaBlock,
};
use crate::format::array::{format_array_node, format_array_node_partial};
use crate::format::object::{format_object_node, format_object_node_partial};
use crate::format::FormatTarget;
use crate::syntax::green::GreenElement;
use crate::syntax::kind::{HjsonLanguage, SyntaxKind};
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::Language;
use rowan::{GreenNode, NodeOrToken};

struct FormatContext<'a> {
    document: &'a HjsonDocument,
    source: String,
    indent_unit: String,
}

impl<'a> FormatContext<'a> {
    fn new(document: &'a HjsonDocument) -> Self {
        let source = document.to_string();
        let style = DocumentStyle::new(&source);
        Self {
            document,
            indent_unit: style.indent_unit(),
            source,
        }
    }

    fn line_indent_for_offset(&self, offset: usize) -> String {
        DocumentStyle::new(&self.source).line_indent_for_offset(offset)
    }

    fn format_root_target(&self) -> Result<HjsonDocument, FormatError> {
        let root = RedNode::new_root(self.document.green_node().clone());
        let newline = self
            .document
            .try_detect_newline()
            .unwrap_or_default()
            .as_str();
        let new_root = format_root_node(&root, newline, &self.indent_unit)?;
        Ok(HjsonDocument::from_green(new_root))
    }

    fn format_node_target(&self, path_from_root: &[usize]) -> Result<HjsonDocument, FormatError> {
        let node = red_node_at_path(self.document, path_from_root).ok_or_else(|| {
            FormatError::InvalidTarget {
                target: FormatTarget::node(path_from_root.to_vec()),
            }
        })?;
        let line_indent = self.line_indent_for_offset(node.text_range().start().into());
        let newline = contextual_newline_kind(self.document, &node).as_str();
        let new_node = match node.kind() {
            SyntaxKind::Object => {
                format_object_node_partial(&node, &line_indent, newline, &self.indent_unit)?
            }
            SyntaxKind::Array => {
                format_array_node_partial(&node, &line_indent, newline, &self.indent_unit)?
            }
            _ => format_red_node(&node, &line_indent, newline, &self.indent_unit)?,
        };
        self.document
            .replace_green_node_internal(path_from_root, new_node)
            .map_err(FormatError::from)
    }

    fn format_path_target(&self, path: &str) -> Result<HjsonDocument, FormatError> {
        let path_segments = crate::edit::high_level::parse_path_for_format(path).map_err(|_| {
            FormatError::InvalidTarget {
                target: FormatTarget::path(path),
            }
        })?;
        if path_segments.is_empty() {
            return self.format_root_target();
        }

        let target_node = self
            .document
            .navigate_to_node_internal(&path_segments)
            .ok_or_else(|| FormatError::InvalidTarget {
                target: FormatTarget::path(path),
            })?;

        let container = target_node
            .resolve_object_child()
            .or_else(|| target_node.resolve_array_child())
            .unwrap_or(target_node);

        self.format_node_target(&container.path_from_root())
    }
}

pub(crate) fn format_document(
    document: &HjsonDocument,
    targets: &[FormatTarget],
) -> Result<HjsonDocument, FormatError> {
    if targets.is_empty() {
        return Ok(document.clone());
    }

    let mut current = document.clone();
    for target in targets {
        let target_context = FormatContext::new(&current);
        current = match target {
            FormatTarget::Document => target_context.format_root_target()?,
            FormatTarget::Node(path) => target_context.format_node_target(path)?,
            FormatTarget::Path(path) => target_context.format_path_target(path)?,
        };
    }

    Ok(current)
}

fn red_node_at_path(document: &HjsonDocument, path_from_root: &[usize]) -> Option<RedNode> {
    let mut current = RedNode::new_root(document.green_node().clone());
    for &idx in path_from_root {
        match current.children_with_tokens().nth(idx)? {
            NodeOrToken::Node(node) => current = node,
            NodeOrToken::Token(_) => return None,
        }
    }
    Some(current)
}

pub(crate) fn child_indent(base_indent: &str, indent_unit: &str) -> String {
    format!("{}{}", base_indent, indent_unit)
}

pub(crate) fn entry_indent_matches_context(entry_indent: &str, base_indent: &str) -> bool {
    if entry_indent.is_empty() {
        return false;
    }

    if base_indent.is_empty() {
        return true;
    }

    entry_indent.starts_with(base_indent) && entry_indent != base_indent
}

pub(crate) fn newline_indent_elements(newline: &str, indent: &str) -> Vec<GreenElement> {
    let mut elements = vec![NodeOrToken::Token(
        SyntaxKind::Newline.create_token(newline),
    )];
    if !indent.is_empty() {
        elements.push(NodeOrToken::Token(
            SyntaxKind::Whitespace.create_token(indent),
        ));
    }
    elements
}

fn format_root_node(
    root: &RedNode,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let mut children = Vec::new();
    for child in root.children_with_tokens() {
        match child {
            NodeOrToken::Node(node) => match node.kind() {
                SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Value => {
                    let formatted = format_red_node(&node, "", newline, indent_unit)?;
                    children.push(NodeOrToken::Node(formatted));
                }
                _ => children.push(clone_red_element(NodeOrToken::Node(node))),
            },
            NodeOrToken::Token(token) => {
                children.push(clone_red_element(NodeOrToken::Token(token)))
            }
        }
    }
    Ok(SyntaxKind::Root.create_node(children))
}

fn format_red_node(
    node: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    match node.kind() {
        SyntaxKind::Object => format_object_node(node, indent, newline, indent_unit),
        SyntaxKind::Array => format_array_node(node, indent, newline, indent_unit),
        SyntaxKind::Pair => format_pair_node(node, indent, newline, indent_unit),
        SyntaxKind::Value => format_value_node(node, indent, newline, indent_unit),
        _ => Ok(node.to_green()),
    }
}

pub(crate) fn format_pair_node(
    pair: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let mut children = Vec::new();
    let mut key_node = None;
    let mut value_node = None;

    for child in pair.children_with_tokens() {
        if let NodeOrToken::Node(node) = child {
            match node.kind() {
                SyntaxKind::Key => key_node = Some(node),
                SyntaxKind::Value => value_node = Some(node),
                _ => {}
            }
        }
    }

    let key = key_node.ok_or_else(|| FormatError::InvalidTarget {
        target: FormatTarget::Node(pair.path_from_root()),
    })?;
    let value = value_node.ok_or_else(|| FormatError::InvalidTarget {
        target: FormatTarget::Node(pair.path_from_root()),
    })?;

    children.extend(leading_comments_and_blank_lines(pair, indent, newline));
    children.push(NodeOrToken::Node(key.to_green()));
    children.push(NodeOrToken::Token(SyntaxKind::Colon.create_token(":")));

    let value_indent = child_indent(indent, indent_unit);
    let formatted_value = format_value_node(&value, indent, newline, indent_unit)?;
    if pair_uses_multiline_value_layout(pair) {
        children.push(NodeOrToken::Token(
            SyntaxKind::Newline.create_token(newline),
        ));
        children.push(NodeOrToken::Token(
            SyntaxKind::Whitespace.create_token(&value_indent),
        ));
    } else {
        children.push(NodeOrToken::Token(SyntaxKind::Whitespace.create_token(" ")));
    }
    children.push(NodeOrToken::Node(formatted_value));
    children.extend(inline_comment_elements(pair));

    Ok(SyntaxKind::Pair.create_node(children))
}

pub(crate) fn format_value_node(
    value: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let mut children = Vec::new();
    let multiline_indent = child_indent(indent, indent_unit);
    for child in value.children_with_tokens() {
        match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Object => children.push(
                NodeOrToken::Node(format_object_node(&node, indent, newline, indent_unit)?),
            ),
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Array => children.push(
                NodeOrToken::Node(format_array_node(&node, indent, newline, indent_unit)?),
            ),
            NodeOrToken::Node(node) => children.push(NodeOrToken::Node(node.to_green())),
            NodeOrToken::Token(token) => {
                let kind = token.kind();
                if matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline) {
                    continue;
                }
                if kind == SyntaxKind::BlockString {
                    let rendered = create_multiline_string_literal(
                        &crate::edit::high_level::parse_multiline_string(token.text(), 0),
                        &multiline_indent,
                        newline,
                    );
                    children.push(NodeOrToken::Token(
                        SyntaxKind::BlockString.create_token(rendered),
                    ));
                } else {
                    children.push(clone_red_element(NodeOrToken::Token(token)));
                }
            }
        }
    }
    Ok(SyntaxKind::Value.create_node(children))
}

pub(crate) fn adjust_closing_indent(
    node: &GreenNode,
    indent: &str,
    closing_kind: SyntaxKind,
) -> Result<GreenNode, FormatError> {
    let mut children: Vec<GreenElement> = node.children().map(clone_element).collect();
    let Some(closing_pos) = children
        .iter()
        .rposition(|elem| is_token_of_kind(elem, closing_kind))
    else {
        return Ok(node.clone());
    };

    let mut insert_pos = closing_pos;
    while insert_pos > 0 && is_token_of_kind(&children[insert_pos - 1], SyntaxKind::Whitespace) {
        children.remove(insert_pos - 1);
        insert_pos -= 1;
    }

    if insert_pos > 0
        && is_token_of_kind(&children[insert_pos - 1], SyntaxKind::Newline)
        && !indent.is_empty()
    {
        children.insert(
            insert_pos,
            NodeOrToken::Token(SyntaxKind::Whitespace.create_token(indent)),
        );
    }

    Ok(HjsonLanguage::kind_from_raw(node.kind()).create_node(children))
}

fn leading_comments_and_blank_lines(
    pair: &RedNode,
    indent: &str,
    newline: &str,
) -> Vec<GreenElement> {
    let mut result = Vec::new();
    let mut pending_blank = 0usize;
    for child in pair.children_with_tokens() {
        let Some(token) = child.as_token() else {
            break;
        };
        match token.kind() {
            SyntaxKind::Newline => pending_blank += 1,
            SyntaxKind::Whitespace => {}
            SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
                if pending_blank > 1 {
                    result.push(NodeOrToken::Token(
                        SyntaxKind::Newline.create_token(newline),
                    ));
                    result.push(NodeOrToken::Token(
                        SyntaxKind::Whitespace.create_token(indent),
                    ));
                }
                result.push(NodeOrToken::Token(token.kind().create_token(token.text())));
                result.push(NodeOrToken::Token(
                    SyntaxKind::Newline.create_token(newline),
                ));
                result.push(NodeOrToken::Token(
                    SyntaxKind::Whitespace.create_token(indent),
                ));
                pending_blank = 0;
            }
            _ => break,
        }
    }
    result
}

fn pair_uses_multiline_value_layout(pair: &RedNode) -> bool {
    let mut seen_colon = false;
    for child in pair.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) => match token.kind() {
                SyntaxKind::Colon => seen_colon = true,
                SyntaxKind::Whitespace if seen_colon => {}
                SyntaxKind::Newline if seen_colon => return true,
                _ => {}
            },
            NodeOrToken::Node(node) if seen_colon && node.kind() == SyntaxKind::Value => {
                return false;
            }
            _ => {}
        }
    }

    false
}

fn inline_comment_elements(pair: &RedNode) -> Vec<GreenElement> {
    let mut result = Vec::new();
    let mut seen_value = false;
    let mut last_was_space = false;

    for child in pair.children_with_tokens() {
        match child {
            NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value => {
                seen_value = true;
            }
            NodeOrToken::Token(token) if seen_value => match token.kind() {
                SyntaxKind::Whitespace => {
                    last_was_space = true;
                }
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
                    if !last_was_space {
                        result.push(NodeOrToken::Token(
                            SyntaxKind::Whitespace.create_token("  "),
                        ));
                    }
                    result.push(NodeOrToken::Token(token.kind().create_token(token.text())));
                    last_was_space = false;
                }
                SyntaxKind::Newline => break,
                _ => {}
            },
            _ => {}
        }
    }

    result
}

pub(crate) fn normalize_standalone_block(
    block: &mut StandaloneTriviaBlock,
    indent: &str,
    newline: &str,
) {
    let mut new_elements = Vec::new();
    let mut pending_blank = 0usize;

    for elem in &block.elements {
        match elem {
            NodeOrToken::Token(token) => {
                let kind = HjsonLanguage::kind_from_raw(token.kind());
                match kind {
                    SyntaxKind::Newline => pending_blank += 1,
                    SyntaxKind::Whitespace => {}
                    SyntaxKind::LineComment
                    | SyntaxKind::HashComment
                    | SyntaxKind::BlockComment => {
                        if pending_blank == 0 && !new_elements.is_empty() {
                            new_elements.push(NodeOrToken::Token(
                                SyntaxKind::Newline.create_token(newline),
                            ));
                        }
                        for _ in 0..pending_blank {
                            new_elements.push(NodeOrToken::Token(
                                SyntaxKind::Newline.create_token(newline),
                            ));
                        }
                        if (pending_blank > 0 || !new_elements.is_empty()) && !indent.is_empty() {
                            new_elements.push(NodeOrToken::Token(
                                SyntaxKind::Whitespace.create_token(indent),
                            ));
                        }
                        new_elements.push(NodeOrToken::Token(kind.create_token(token.text())));
                        pending_blank = 0;
                    }
                    _ => {
                        new_elements.push(elem.clone());
                        pending_blank = 0;
                    }
                }
            }
            NodeOrToken::Node(_) => new_elements.push(elem.clone()),
        }
    }

    for _ in 0..pending_blank {
        new_elements.push(NodeOrToken::Token(
            SyntaxKind::Newline.create_token(newline),
        ));
    }

    block.elements = new_elements;
}

fn clone_red_element(
    elem: rowan::NodeOrToken<RedNode, crate::syntax::red::RedToken>,
) -> GreenElement {
    match elem {
        NodeOrToken::Node(node) => NodeOrToken::Node(node.to_green()),
        NodeOrToken::Token(token) => NodeOrToken::Token(token.kind().create_token(token.text())),
    }
}
