use crate::edit::api::{create_node, create_token, RedNodeEdit};
use crate::edit::high_level::{
    build_array_items, build_object_items, clone_element, detect_array_entry_indent,
    detect_object_entry_indent, is_token_of_kind, ArrayItem, CommentStyle, EntryTerminator,
    FormatError, HjsonDocument, ObjectItem, StandaloneTriviaBlock,
};
use crate::format::FormatTarget;
use crate::syntax::green::GreenElement;
use crate::syntax::kind::{HjsonLanguage, SyntaxKind};
use crate::syntax::red::{RedNode, RedNodeExt};
use rowan::Language;
use rowan::{GreenNode, NodeOrToken};

pub(crate) fn format_document(
    document: &HjsonDocument,
    targets: &[FormatTarget],
) -> Result<HjsonDocument, FormatError> {
    if targets.is_empty() {
        return Ok(document.clone());
    }

    let mut current = document.clone();
    for target in targets {
        current = match target {
            FormatTarget::Document => format_root_target(&current)?,
            FormatTarget::Node(path) => format_node_target(&current, path)?,
            FormatTarget::Path(path) => format_path_target(&current, path)?,
        };
    }

    Ok(current)
}

fn format_root_target(document: &HjsonDocument) -> Result<HjsonDocument, FormatError> {
    let source = document.to_string();
    let newline = preferred_newline(&source);
    let indent_unit = detect_default_indent_unit(&source);
    let root = RedNode::new_root(document.green_node().clone());
    let new_root = format_root_node(&root, newline, &indent_unit)?;
    Ok(HjsonDocument::from_green(new_root))
}

fn format_node_target(
    document: &HjsonDocument,
    path_from_root: &[usize],
) -> Result<HjsonDocument, FormatError> {
    let source = document.to_string();
    let newline = preferred_newline(&source);
    let indent_unit = detect_default_indent_unit(&source);
    let node =
        red_node_at_path(document, path_from_root).ok_or_else(|| FormatError::InvalidTarget {
            target: FormatTarget::node(path_from_root.to_vec()),
        })?;
    let line_indent = line_indent_for_offset(&source, node.text_range().start().into());
    let new_node = match node.kind() {
        SyntaxKind::Object => {
            format_object_node_partial(&node, &line_indent, newline, &indent_unit)?
        }
        SyntaxKind::Array => format_array_node_partial(&node, &line_indent, newline, &indent_unit)?,
        _ => format_red_node(&node, &line_indent, newline, &indent_unit)?,
    };
    let updated = document.replace_green_node_internal(path_from_root, new_node)?;
    Ok(updated)
}

fn format_path_target(document: &HjsonDocument, path: &str) -> Result<HjsonDocument, FormatError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(path);
    if path_segments.is_empty() {
        return format_root_target(document);
    }

    let target_node = document
        .navigate_to_node_internal(&path_segments)
        .ok_or_else(|| FormatError::InvalidTarget {
            target: FormatTarget::path(path),
        })?;

    let container = resolve_object_child(&target_node)
        .or_else(|| resolve_array_child(&target_node))
        .unwrap_or(target_node);

    format_node_target(document, &container.path_from_root())
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

pub(crate) fn preferred_newline(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn detect_default_indent_unit(source: &str) -> String {
    let mut space_indents = Vec::new();
    let mut tab_count = 0usize;

    for line in source.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let indent: String = line
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        if indent.is_empty() {
            continue;
        }
        if indent.contains('\t') {
            tab_count += 1;
        } else {
            space_indents.push(indent.len());
        }
    }

    if tab_count > 0 {
        return "\t".to_string();
    }

    let width = space_indents
        .into_iter()
        .filter(|len| *len > 0)
        .min()
        .unwrap_or(2);
    " ".repeat(width)
}

fn line_indent_for_offset(source: &str, offset: usize) -> String {
    let start = source[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    source[start..offset]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn child_indent(base_indent: &str, indent_unit: &str) -> String {
    format!("{}{}", base_indent, indent_unit)
}

fn newline_indent_elements(newline: &str, indent: &str) -> Vec<GreenElement> {
    let mut elements = vec![NodeOrToken::Token(create_token(
        SyntaxKind::Newline,
        newline,
    ))];
    if !indent.is_empty() {
        elements.push(NodeOrToken::Token(create_token(
            SyntaxKind::Whitespace,
            indent,
        )));
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
    Ok(create_node(SyntaxKind::Root, children))
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

fn format_pair_node(
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
    children.push(NodeOrToken::Token(create_token(SyntaxKind::Colon, ":")));

    let value_indent = child_indent(indent, indent_unit);
    let formatted_value = format_value_node(&value, indent, newline, indent_unit)?;
    if pair_uses_multiline_value_layout(pair) {
        children.push(NodeOrToken::Token(create_token(
            SyntaxKind::Newline,
            newline,
        )));
        children.push(NodeOrToken::Token(create_token(
            SyntaxKind::Whitespace,
            &value_indent,
        )));
    } else {
        children.push(NodeOrToken::Token(create_token(
            SyntaxKind::Whitespace,
            " ",
        )));
    }
    children.push(NodeOrToken::Node(formatted_value));
    children.extend(inline_comment_elements(pair));

    Ok(create_node(SyntaxKind::Pair, children))
}

fn format_value_node(
    value: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let mut children = Vec::new();
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
                children.push(clone_red_element(NodeOrToken::Token(token)));
            }
        }
    }
    Ok(create_node(SyntaxKind::Value, children))
}

fn format_object_node(
    object: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let object_green = object.to_green();
    let children: Vec<GreenElement> = object_green.children().map(clone_element).collect();
    let lbrace = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::LBrace))
        .cloned();
    let rbrace = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::RBrace))
        .cloned();
    let items = build_object_items(&children);
    let entry_indent =
        detect_object_entry_indent(&items).unwrap_or_else(|| child_indent(indent, indent_unit));
    let is_braceless = lbrace.is_none() && rbrace.is_none();

    let mut formatted_items = Vec::new();
    let mut entry_positions = Vec::new();

    for item in items {
        match item {
            ObjectItem::Entry(mut entry) => {
                if let NodeOrToken::Node(pair_green) = &entry.pair {
                    let pair_red = RedNode::new_root(pair_green.clone());
                    entry.pair = NodeOrToken::Node(format_pair_node(
                        &pair_red,
                        &entry_indent,
                        newline,
                        indent_unit,
                    )?);
                }
                if !is_braceless && entry.leading_line_trivia.is_empty() {
                    entry.leading_line_trivia = newline_indent_elements(newline, &entry_indent);
                }
                entry_positions.push(formatted_items.len());
                formatted_items.push(ObjectItem::Entry(entry));
            }
            ObjectItem::Standalone(block) => formatted_items.push(ObjectItem::Standalone(block)),
        }
    }

    for window in entry_positions.windows(2) {
        if let ObjectItem::Entry(entry) = &mut formatted_items[window[0]] {
            if matches!(entry.terminator, EntryTerminator::EndOfContainer) {
                entry.terminator =
                    EntryTerminator::Newline(newline_indent_elements(newline, &entry_indent));
            }
        }
    }

    if let Some(last_idx) = entry_positions.last().copied() {
        let has_trailing_standalone = formatted_items
            .iter()
            .skip(last_idx + 1)
            .any(|item| matches!(item, ObjectItem::Standalone(_)));
        if !has_trailing_standalone && !is_braceless {
            if let ObjectItem::Entry(last_entry) = &mut formatted_items[last_idx] {
                if matches!(last_entry.terminator, EntryTerminator::EndOfContainer) {
                    last_entry.terminator =
                        EntryTerminator::Newline(newline_indent_elements(newline, indent));
                }
            }
        }
    }

    for item in &mut formatted_items {
        if let ObjectItem::Standalone(block) = item {
            normalize_standalone_block(block, &entry_indent, newline);
        }
    }

    Ok(create_node(
        SyntaxKind::Object,
        crate::edit::high_level::rebuild_object_children_for_format(
            lbrace,
            formatted_items,
            rbrace,
        ),
    ))
}

fn format_array_node(
    array: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let array_green = array.to_green();
    let children: Vec<GreenElement> = array_green.children().map(clone_element).collect();
    let lbracket = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::LBracket))
        .cloned();
    let rbracket = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::RBracket))
        .cloned();
    let items = build_array_items(&children);
    let entry_indent =
        detect_array_entry_indent(&items).unwrap_or_else(|| child_indent(indent, indent_unit));

    let mut formatted_items = Vec::new();
    let mut entry_positions = Vec::new();

    for item in items {
        match item {
            ArrayItem::Entry(mut entry) => {
                if let NodeOrToken::Node(value_green) = &entry.value {
                    let value_red = RedNode::new_root(value_green.clone());
                    entry.value = NodeOrToken::Node(format_value_node(
                        &value_red,
                        &entry_indent,
                        newline,
                        indent_unit,
                    )?);
                }
                if entry.leading_line_trivia.is_empty() {
                    entry.leading_line_trivia = newline_indent_elements(newline, &entry_indent);
                }
                entry_positions.push(formatted_items.len());
                formatted_items.push(ArrayItem::Entry(entry));
            }
            ArrayItem::Standalone(block) => formatted_items.push(ArrayItem::Standalone(block)),
        }
    }

    for window in entry_positions.windows(2) {
        if let ArrayItem::Entry(entry) = &mut formatted_items[window[0]] {
            if matches!(entry.terminator, EntryTerminator::EndOfContainer) {
                entry.terminator =
                    EntryTerminator::Newline(newline_indent_elements(newline, &entry_indent));
            }
        }
    }

    if let Some(last_idx) = entry_positions.last().copied() {
        let has_trailing_standalone = formatted_items
            .iter()
            .skip(last_idx + 1)
            .any(|item| matches!(item, ArrayItem::Standalone(_)));
        if let ArrayItem::Entry(last_entry) = &mut formatted_items[last_idx] {
            if !has_trailing_standalone {
                last_entry.terminator = EntryTerminator::Newline(vec![NodeOrToken::Token(
                    create_token(SyntaxKind::Newline, newline),
                )]);
            }
        }
    }

    for item in &mut formatted_items {
        if let ArrayItem::Standalone(block) = item {
            normalize_standalone_block(block, &entry_indent, newline);
        }
    }

    Ok(create_node(
        SyntaxKind::Array,
        crate::edit::high_level::rebuild_array_children_for_format(
            lbracket,
            formatted_items,
            rbracket,
        ),
    ))
}

fn format_object_node_partial(
    object: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let formatted = format_object_node(object, indent, newline, indent_unit)?;
    adjust_closing_indent(&formatted, indent, SyntaxKind::RBrace)
}

fn format_array_node_partial(
    array: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let formatted = format_array_node(array, indent, newline, indent_unit)?;
    adjust_closing_indent(&formatted, indent, SyntaxKind::RBracket)
}

fn adjust_closing_indent(
    node: &GreenNode,
    indent: &str,
    closing_kind: SyntaxKind,
) -> Result<GreenNode, FormatError> {
    let mut children: Vec<GreenElement> = node.children().map(clone_element).collect();
    let closing_pos = children
        .iter()
        .rposition(|elem| is_token_of_kind(elem, closing_kind))
        .ok_or_else(|| FormatError::InvalidTarget {
            target: FormatTarget::Document,
        })?;

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
            NodeOrToken::Token(create_token(SyntaxKind::Whitespace, indent)),
        );
    }

    Ok(create_node(
        HjsonLanguage::kind_from_raw(node.kind()),
        children,
    ))
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
                    result.push(NodeOrToken::Token(create_token(
                        SyntaxKind::Newline,
                        newline,
                    )));
                    result.push(NodeOrToken::Token(create_token(
                        SyntaxKind::Whitespace,
                        indent,
                    )));
                }
                result.push(NodeOrToken::Token(create_token(token.kind(), token.text())));
                result.push(NodeOrToken::Token(create_token(
                    SyntaxKind::Newline,
                    newline,
                )));
                result.push(NodeOrToken::Token(create_token(
                    SyntaxKind::Whitespace,
                    indent,
                )));
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
                        result.push(NodeOrToken::Token(create_token(
                            SyntaxKind::Whitespace,
                            "  ",
                        )));
                    }
                    result.push(NodeOrToken::Token(create_token(token.kind(), token.text())));
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

fn normalize_standalone_block(block: &mut StandaloneTriviaBlock, indent: &str, newline: &str) {
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
                            new_elements.push(NodeOrToken::Token(create_token(
                                SyntaxKind::Newline,
                                newline,
                            )));
                        }
                        for _ in 1..pending_blank {
                            new_elements.push(NodeOrToken::Token(create_token(
                                SyntaxKind::Newline,
                                newline,
                            )));
                        }
                        if (pending_blank > 0 || !new_elements.is_empty()) && !indent.is_empty() {
                            new_elements.push(NodeOrToken::Token(create_token(
                                SyntaxKind::Whitespace,
                                indent,
                            )));
                        }
                        new_elements.push(NodeOrToken::Token(create_token(kind, token.text())));
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

    block.elements = new_elements;
}

fn clone_red_element(
    elem: rowan::NodeOrToken<RedNode, crate::syntax::red::RedToken>,
) -> GreenElement {
    match elem {
        NodeOrToken::Node(node) => NodeOrToken::Node(node.to_green()),
        NodeOrToken::Token(token) => NodeOrToken::Token(create_token(token.kind(), token.text())),
    }
}

fn resolve_object_child(node: &RedNode) -> Option<RedNode> {
    node.child_by_kind(SyntaxKind::Object).or_else(|| {
        node.child_by_kind(SyntaxKind::Value)?
            .child_by_kind(SyntaxKind::Object)
    })
}

fn resolve_array_child(node: &RedNode) -> Option<RedNode> {
    node.child_by_kind(SyntaxKind::Array).or_else(|| {
        node.child_by_kind(SyntaxKind::Value)?
            .child_by_kind(SyntaxKind::Array)
    })
}

#[allow(dead_code)]
fn _keep_comment_style(_style: CommentStyle) {}
