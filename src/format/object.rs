use crate::edit::api::{create_node, create_token};
use crate::syntax::red::RedNodeCoreExt;
use crate::edit::high_level::{
    build_object_items, clone_element, detect_object_entry_indent, is_token_of_kind,
    EntryTerminator, FormatError, ObjectItem,
};
use crate::format::core::{
    adjust_closing_indent, child_indent, entry_indent_matches_context, format_pair_node,
    newline_indent_elements, normalize_standalone_block,
};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::RedNode;
use rowan::{GreenNode, NodeOrToken};

pub(crate) fn format_object_node(
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
    let default_entry_indent = child_indent(indent, indent_unit);
    let entry_indent = detect_object_entry_indent(&items)
        .filter(|detected| entry_indent_matches_context(detected, indent))
        .unwrap_or_else(|| default_entry_indent.clone());
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
                    if entry_positions.is_empty() {
                        entry.leading_line_trivia = newline_indent_elements(newline, &entry_indent);
                    } else if !entry_indent.is_empty() {
                        entry.leading_line_trivia = vec![NodeOrToken::Token(create_token(
                            SyntaxKind::Whitespace,
                            &entry_indent,
                        ))];
                    }
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
                entry.terminator = EntryTerminator::Newline(vec![NodeOrToken::Token(create_token(
                    SyntaxKind::Newline,
                    newline,
                ))]);
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

pub(crate) fn format_object_node_partial(
    object: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let formatted = format_object_node(object, indent, newline, indent_unit)?;
    adjust_closing_indent(&formatted, indent, SyntaxKind::RBrace)
}




