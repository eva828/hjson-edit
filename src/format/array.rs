use crate::edit::api::{create_node, create_token};
use crate::edit::high_level::{
    build_array_items, clone_element, detect_array_entry_indent, is_token_of_kind, ArrayItem,
    EntryTerminator, FormatError,
};
use crate::format::core::{
    child_indent, entry_indent_matches_context, format_value_node, newline_indent_elements,
    normalize_standalone_block,
};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::RedNode;
use crate::syntax::red::RedNodeCoreExt;
use rowan::{GreenNode, NodeOrToken};

pub(crate) fn format_array_node(
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
    let default_entry_indent = child_indent(indent, indent_unit);
    let entry_indent = detect_array_entry_indent(&items)
        .filter(|detected| entry_indent_matches_context(detected, indent))
        .unwrap_or_else(|| default_entry_indent.clone());

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
                formatted_items.push(ArrayItem::Entry(entry));
            }
            ArrayItem::Standalone(block) => formatted_items.push(ArrayItem::Standalone(block)),
        }
    }

    for window in entry_positions.windows(2) {
        if let ArrayItem::Entry(entry) = &mut formatted_items[window[0]] {
            if matches!(entry.terminator, EntryTerminator::EndOfContainer) {
                entry.terminator = EntryTerminator::Newline(vec![NodeOrToken::Token(
                    create_token(SyntaxKind::Newline, newline),
                )]);
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

pub(crate) fn format_array_node_partial(
    array: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    let formatted = format_array_node(array, indent, newline, indent_unit)?;
    crate::format::core::adjust_closing_indent(&formatted, indent, SyntaxKind::RBracket)
}
