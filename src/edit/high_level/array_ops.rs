use crate::edit::api::{create_node, create_token};
use crate::edit::high_level::{
    build_array_items, clone_element, create_value_node, is_token_of_kind, ArrayEntry, ArrayItem,
    EntryTerminator, HjsonEditError, HjsonValue, StringStyle,
};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::RedNode;
use crate::syntax::red::RedNodeCoreExt;
use rowan::{GreenNode, NodeOrToken};

pub(crate) fn remove_array_entry(
    arr_node: &RedNode,
    index: usize,
) -> Result<GreenNode, HjsonEditError> {
    let arr_green: GreenNode = arr_node.to_green();
    let children: Vec<GreenElement> = arr_green.children().map(clone_element).collect();
    let lbracket = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::LBracket))
        .cloned();
    let rbracket = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::RBracket))
        .cloned();
    let items = build_array_items(&children);

    let mut found = false;
    let mut entry_index = 0usize;
    let mut new_items: Vec<ArrayItem> = Vec::new();
    let mut pending_leading: Vec<GreenElement> = Vec::new();

    for item in items {
        match item {
            ArrayItem::Entry(mut entry) => {
                if entry_index == index {
                    found = true;
                    if new_items
                        .iter()
                        .all(|item| !matches!(item, ArrayItem::Entry(_)))
                    {
                        pending_leading = entry.leading_line_trivia.clone();
                    }
                    entry_index += 1;
                    continue;
                }

                if !pending_leading.is_empty() {
                    let mut merged = pending_leading;
                    merged.extend(entry.leading_line_trivia);
                    entry.leading_line_trivia = merged;
                    pending_leading = Vec::new();
                }

                new_items.push(ArrayItem::Entry(entry));
                entry_index += 1;
            }
            ArrayItem::Standalone(block) => new_items.push(ArrayItem::Standalone(block)),
        }
    }

    if !found {
        return Err(HjsonEditError::InvalidIndex {
            index,
            count: entry_index,
        });
    }

    Ok(create_node(
        SyntaxKind::Array,
        crate::edit::high_level::rebuild_array_children_for_format(lbracket, new_items, rbracket),
    ))
}

pub(crate) fn insert_array_entry_structural(
    arr_node: &RedNode,
    index: usize,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    let arr_green: GreenNode = arr_node.to_green();
    let children: Vec<GreenElement> = arr_green.children().map(clone_element).collect();
    let lbracket = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::LBracket))
        .cloned();
    let rbracket = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::RBracket))
        .cloned();
    let items = build_array_items(&children);

    let mut new_items = Vec::new();
    let mut entry_index = 0usize;
    let insert_at = index;
    let is_append = index == usize::MAX;
    let mut inserted = false;
    let mut last_entry_indent: Option<GreenElement> = None;
    let mut trailing_standalone = Vec::new();

    for item in items {
        match item {
            ArrayItem::Entry(entry) => {
                if let Some(indent) = entry
                    .leading_line_trivia
                    .iter()
                    .rev()
                    .find(|elem| is_token_of_kind(elem, SyntaxKind::Whitespace))
                    .cloned()
                {
                    last_entry_indent = Some(indent);
                }
                if !is_append && !inserted && entry_index == insert_at {
                    new_items.push(ArrayItem::Entry(ArrayEntry {
                        leading_line_trivia: entry.leading_line_trivia.clone(),
                        value: NodeOrToken::Node(create_value_node(value, string_style, None)?),
                        trailing_inline_trivia: Vec::new(),
                        terminator: EntryTerminator::Newline(vec![NodeOrToken::Token(
                            create_token(SyntaxKind::Newline, "\n"),
                        )]),
                    }));
                    inserted = true;
                }

                entry_index += 1;
                new_items.push(ArrayItem::Entry(entry));
            }
            ArrayItem::Standalone(block) => {
                if is_append {
                    trailing_standalone.push(ArrayItem::Standalone(block));
                } else {
                    new_items.push(ArrayItem::Standalone(block));
                }
            }
        }
    }

    if !inserted {
        let terminator = if trailing_standalone.is_empty() {
            match rbracket.as_ref() {
                Some(elem) if is_token_of_kind(elem, SyntaxKind::RBracket) => {
                    EntryTerminator::Newline(vec![NodeOrToken::Token(create_token(
                        SyntaxKind::Newline,
                        "\n",
                    ))])
                }
                _ => EntryTerminator::EndOfContainer,
            }
        } else {
            EntryTerminator::Newline(vec![NodeOrToken::Token(create_token(
                SyntaxKind::Newline,
                "\n",
            ))])
        };
        let leading_line_trivia = if new_items
            .iter()
            .any(|item| matches!(item, ArrayItem::Entry(_)))
        {
            let mut trivia = Vec::new();
            if let Some(indent) = last_entry_indent {
                trivia.push(indent);
            }
            trivia
        } else {
            Vec::new()
        };
        new_items.push(ArrayItem::Entry(ArrayEntry {
            leading_line_trivia,
            value: NodeOrToken::Node(create_value_node(value, string_style, None)?),
            trailing_inline_trivia: Vec::new(),
            terminator,
        }));
    }

    new_items.extend(trailing_standalone);

    Ok(create_node(
        SyntaxKind::Array,
        crate::edit::high_level::rebuild_array_children_for_format(lbracket, new_items, rbracket),
    ))
}
