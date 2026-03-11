use crate::edit::api::{create_node, create_token};

use crate::edit::high_level::{
    build_object_items, clone_element, create_pair_node, extract_key_text, is_token_of_kind,
    rebuild_object_children_for_format, replace_pair_value, EntryTerminator, HjsonEditError,
    HjsonValue, ObjectEntry, ObjectItem, StringStyle,
};
use crate::edit::high_level::structure::{indentation_from_standalone_block, whitespace_after_last_newline};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::{GreenNode, NodeOrToken};

pub(crate) struct InsertFormat<'a> {
    pub has_braces: bool,
    pub newline: &'a str,
    pub entry_indent: &'a str,
    pub closing_indent: &'a str,
}

pub(crate) fn insert_object_entry_without_reformat(
    items: &mut Vec<ObjectItem>,
    key: &str,
    value: &HjsonValue,
    string_style: StringStyle,
    format: &InsertFormat<'_>,
) -> Result<(), HjsonEditError> {
    let InsertFormat { has_braces, newline, entry_indent, closing_indent } = *format;
    let pair = NodeOrToken::Node(create_pair_node(key, value, string_style, None)?);

    let has_existing_entries = items
        .iter()
        .any(|item| matches!(item, ObjectItem::Entry(_)));
    let last_entry_ends_with_newline = items.iter().rev().find_map(|item| match item {
        ObjectItem::Entry(entry) => Some(matches!(entry.terminator, EntryTerminator::Newline(_))),
        ObjectItem::Standalone(_) => None,
    });

    let leading_line_trivia = if has_existing_entries {
        if last_entry_ends_with_newline.unwrap_or(false) {
            if entry_indent.is_empty() {
                Vec::new()
            } else {
                vec![NodeOrToken::Token(create_token(
                    SyntaxKind::Whitespace,
                    entry_indent,
                ))]
            }
        } else {
            let mut trivia = vec![NodeOrToken::Token(create_token(
                SyntaxKind::Newline,
                newline,
            ))];
            if !entry_indent.is_empty() {
                trivia.push(NodeOrToken::Token(create_token(
                    SyntaxKind::Whitespace,
                    entry_indent,
                )));
            }
            trivia
        }
    } else if has_braces {
        let mut trivia = vec![NodeOrToken::Token(create_token(
            SyntaxKind::Newline,
            newline,
        ))];
        if !entry_indent.is_empty() {
            trivia.push(NodeOrToken::Token(create_token(
                SyntaxKind::Whitespace,
                entry_indent,
            )));
        }
        trivia
    } else {
        Vec::new()
    };

    let mut terminator_suffix = Vec::new();
    if has_braces {
        terminator_suffix.push(NodeOrToken::Token(create_token(
            SyntaxKind::Newline,
            newline,
        )));
        if !closing_indent.is_empty() {
            terminator_suffix.push(NodeOrToken::Token(create_token(
                SyntaxKind::Whitespace,
                closing_indent,
            )));
        }
    }

    let new_entry = ObjectItem::Entry(ObjectEntry {
        leading_line_trivia,
        leading_comments: Vec::new(),
        pair,
        trailing_inline_trivia: Vec::new(),
        terminator: if has_braces {
            EntryTerminator::Newline(terminator_suffix)
        } else {
            EntryTerminator::EndOfContainer
        },
    });

    let mut trailing_standalone = Vec::new();
    while matches!(items.last(), Some(ObjectItem::Standalone(_))) {
        trailing_standalone.push(items.pop().unwrap());
    }
    trailing_standalone.reverse();

    items.push(new_entry);
    items.extend(trailing_standalone);
    Ok(())
}

pub(crate) fn remove_object_entry(
    obj_node: &RedNode,
    key: &str,
) -> Result<GreenNode, HjsonEditError> {
    let obj_green: GreenNode = obj_node.to_green();
    let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();
    let lbrace = children
        .iter()
        .find(|e| is_token_of_kind(e, SyntaxKind::LBrace))
        .cloned();
    let rbrace = children
        .iter()
        .find(|e| is_token_of_kind(e, SyntaxKind::RBrace))
        .cloned();
    let items = build_object_items(&children);

    let mut found = false;
    let mut new_items: Vec<ObjectItem> = Vec::new();
    let mut pending_terminator: Option<EntryTerminator> = None;
    let mut has_surviving_entry = false;

    for item in items {
        match &item {
            ObjectItem::Entry(entry) => {
                if let NodeOrToken::Node(pair_node) = &entry.pair {
                    let pair_red = RedNode::new_root(pair_node.clone());
                    if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                        if let Some(key_text) = extract_key_text(&key_node) {
                            if key_text == key {
                                found = true;
                                if !has_surviving_entry {
                                    pending_terminator = Some(entry.terminator.clone());
                                }
                                continue;
                            }
                        }
                    }
                }

                let mut modified_entry = entry.clone();
                if let Some(ref term) = pending_terminator {
                    if let EntryTerminator::Newline(suffix) = term {
                        let mut new_leading = entry.leading_line_trivia.clone();
                        if !new_leading
                            .iter()
                            .any(|elem| is_token_of_kind(elem, SyntaxKind::Newline))
                        {
                            let mut merged = suffix.clone();
                            merged.extend(new_leading);
                            new_leading = merged;
                        }
                        modified_entry.leading_line_trivia = new_leading;
                    }
                    pending_terminator = None;
                }

                new_items.push(ObjectItem::Entry(modified_entry));
                has_surviving_entry = true;
            }
            ObjectItem::Standalone(_) => {
                new_items.push(item);
            }
        }
    }

    if !found {
        return Err(HjsonEditError::InvalidPath { path: key.to_string() });
    }

    Ok(create_node(
        SyntaxKind::Object,
        rebuild_object_children_for_format(lbrace, new_items, rbrace),
    ))
}

pub(crate) fn set_object_entry(
    obj_node: &RedNode,
    key: &str,
    value: &HjsonValue,
    string_style: StringStyle,
    base_indent: &str,
    indent_unit: &str,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let obj_green: GreenNode = obj_node.to_green();
    let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();

    let lbrace = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::LBrace))
        .cloned();
    let rbrace = children
        .iter()
        .find(|c| is_token_of_kind(c, SyntaxKind::RBrace))
        .cloned();
    let items = build_object_items(&children);
    let mut found = false;
    let mut new_items = Vec::new();
    let has_braces = lbrace.is_some() && rbrace.is_some();
    let entry_indent = detect_object_entry_indent(&items).unwrap_or_else(|| {
        if has_braces {
            format!("{}{}", base_indent, indent_unit)
        } else {
            String::new()
        }
    });

    for item in items {
        match item {
            ObjectItem::Entry(mut entry) => {
                if let NodeOrToken::Node(pair_green) = &entry.pair {
                    let pair_red = RedNode::new_root(pair_green.clone());
                    if let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) {
                        if let Some(key_text) = extract_key_text(&key_node) {
                            if key_text == key {
                                entry.pair = NodeOrToken::Node(replace_pair_value(
                                    pair_green,
                                    value,
                                    string_style,
                                )?);
                                found = true;
                            }
                        }
                    }
                }
                new_items.push(ObjectItem::Entry(entry));
            }
            ObjectItem::Standalone(block) => new_items.push(ObjectItem::Standalone(block)),
        }
    }

    if !found {
        insert_object_entry_without_reformat(
            &mut new_items,
            key,
            value,
            string_style,
            &InsertFormat {
                has_braces,
                newline,
                entry_indent: &entry_indent,
                closing_indent: base_indent,
            },
        )?;
    }

    Ok(create_node(
        SyntaxKind::Object,
        rebuild_object_children_for_format(lbrace, new_items, rbrace),
    ))
}
pub(crate) fn detect_object_entry_indent(items: &[ObjectItem]) -> Option<String> {
    items.iter().find_map(|item| match item {
        ObjectItem::Entry(entry) => {
            let indent = whitespace_after_last_newline(&entry.leading_line_trivia);
            if indent.is_empty() {
                None
            } else {
                Some(indent)
            }
        }
        ObjectItem::Standalone(block) => {
            let indent = indentation_from_standalone_block(block);
            if indent.is_empty() {
                None
            } else {
                Some(indent)
            }
        }
    })
}

