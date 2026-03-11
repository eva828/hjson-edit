use crate::edit::api::create_token;
use crate::edit::high_level::{create_pair_node, EntryTerminator, HjsonEditError, HjsonValue, ObjectEntry, ObjectItem, StringStyle};
use crate::edit::high_level::structure::{indentation_from_standalone_block, whitespace_after_last_newline};
use crate::syntax::kind::SyntaxKind;
use rowan::NodeOrToken;

pub(crate) fn insert_object_entry_without_reformat(
    items: &mut Vec<ObjectItem>,
    key: &str,
    value: &HjsonValue,
    string_style: StringStyle,
    has_braces: bool,
    newline: &str,
    entry_indent: &str,
    closing_indent: &str,
) -> Result<(), HjsonEditError> {
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
