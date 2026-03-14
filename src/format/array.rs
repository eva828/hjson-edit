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
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::{GreenNode, NodeOrToken};

pub(crate) struct ArrayFormatPass<'a> {
    array: &'a RedNode,
    indent: &'a str,
    newline: &'a str,
    indent_unit: &'a str,
}

impl<'a> ArrayFormatPass<'a> {
    fn new(array: &'a RedNode, indent: &'a str, newline: &'a str, indent_unit: &'a str) -> Self {
        Self {
            array,
            indent,
            newline,
            indent_unit,
        }
    }

    fn format(&self) -> Result<GreenNode, FormatError> {
        let context = ArrayFormatContext::from_node(self.array, self.indent, self.indent_unit);
        let mut formatted_items = self.format_items(&context)?;
        self.normalize_terminators(&mut formatted_items);
        self.normalize_standalone_blocks(&mut formatted_items, &context.entry_indent);
        Ok(SyntaxKind::Array.create_node(
            crate::edit::high_level::rebuild_array_children_for_format(
                context.lbracket,
                formatted_items,
                context.rbracket,
            ),
        ))
    }

    fn format_items(&self, context: &ArrayFormatContext) -> Result<Vec<ArrayItem>, FormatError> {
        let mut formatted_items = Vec::new();
        let mut entry_positions = Vec::new();

        for item in context.items.clone() {
            match item {
                ArrayItem::Entry(mut entry) => {
                    entry = self.format_entry(
                        entry,
                        &context.entry_indent,
                        entry_positions.is_empty(),
                    )?;
                    entry_positions.push(formatted_items.len());
                    formatted_items.push(ArrayItem::Entry(entry));
                }
                ArrayItem::Standalone(block) => formatted_items.push(ArrayItem::Standalone(block)),
            }
        }

        Ok(formatted_items)
    }

    fn format_entry(
        &self,
        mut entry: crate::edit::high_level::ArrayEntry,
        entry_indent: &str,
        is_first_entry: bool,
    ) -> Result<crate::edit::high_level::ArrayEntry, FormatError> {
        if let Some(value_green) = entry.value.as_node() {
            let value_red = RedNode::new_root(value_green.clone());
            entry.value = NodeOrToken::Node(format_value_node(
                &value_red,
                entry_indent,
                self.newline,
                self.indent_unit,
            )?);
        }

        if entry.leading_line_trivia.is_empty() {
            entry.leading_line_trivia = if is_first_entry {
                newline_indent_elements(self.newline, entry_indent)
            } else if !entry_indent.is_empty() {
                vec![NodeOrToken::Token(
                    SyntaxKind::Whitespace.create_token(entry_indent),
                )]
            } else {
                Vec::new()
            };
        }

        Ok(entry)
    }

    fn normalize_terminators(&self, items: &mut [ArrayItem]) {
        let entry_positions: Vec<usize> = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                ArrayItem::Entry(_) => Some(index),
                ArrayItem::Standalone(_) => None,
            })
            .collect();

        for window in entry_positions.windows(2) {
            if let ArrayItem::Entry(entry) = &mut items[window[0]] {
                if matches!(entry.terminator, EntryTerminator::EndOfContainer) {
                    entry.terminator = EntryTerminator::Newline(vec![NodeOrToken::Token(
                        SyntaxKind::Newline.create_token(self.newline),
                    )]);
                }
            }
        }

        if let Some(last_idx) = entry_positions.last().copied() {
            let has_trailing_standalone = items
                .iter()
                .skip(last_idx + 1)
                .any(|item| matches!(item, ArrayItem::Standalone(_)));
            if let ArrayItem::Entry(last_entry) = &mut items[last_idx] {
                if !has_trailing_standalone {
                    last_entry.terminator = EntryTerminator::Newline(vec![NodeOrToken::Token(
                        SyntaxKind::Newline.create_token(self.newline),
                    )]);
                }
            }
        }
    }

    fn normalize_standalone_blocks(&self, items: &mut [ArrayItem], entry_indent: &str) {
        for item in items {
            if let ArrayItem::Standalone(block) = item {
                normalize_standalone_block(block, entry_indent, self.newline);
            }
        }
    }
}

#[derive(Clone)]
struct ArrayFormatContext {
    lbracket: Option<GreenElement>,
    rbracket: Option<GreenElement>,
    items: Vec<ArrayItem>,
    entry_indent: String,
}

impl ArrayFormatContext {
    fn from_node(array: &RedNode, indent: &str, indent_unit: &str) -> Self {
        let array_green = array.to_green();
        let children: Vec<GreenElement> = array_green.children().map(clone_element).collect();
        let lbracket = children
            .iter()
            .find(|elem| is_token_of_kind(elem, SyntaxKind::LBracket))
            .cloned();
        let rbracket = children
            .iter()
            .find(|elem| is_token_of_kind(elem, SyntaxKind::RBracket))
            .cloned();
        let items = build_array_items(&children);
        let default_entry_indent = child_indent(indent, indent_unit);
        let entry_indent = detect_array_entry_indent(&items)
            .filter(|detected| entry_indent_matches_context(detected, indent))
            .unwrap_or(default_entry_indent);

        Self {
            lbracket,
            rbracket,
            items,
            entry_indent,
        }
    }
}

pub(crate) fn format_array_node(
    array: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    ArrayFormatPass::new(array, indent, newline, indent_unit).format()
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
