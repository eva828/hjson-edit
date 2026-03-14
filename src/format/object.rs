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
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::{GreenNode, NodeOrToken};

pub(crate) struct ObjectFormatPass<'a> {
    object: &'a RedNode,
    indent: &'a str,
    newline: &'a str,
    indent_unit: &'a str,
}

impl<'a> ObjectFormatPass<'a> {
    fn new(object: &'a RedNode, indent: &'a str, newline: &'a str, indent_unit: &'a str) -> Self {
        Self {
            object,
            indent,
            newline,
            indent_unit,
        }
    }

    fn format(&self) -> Result<GreenNode, FormatError> {
        let context = ObjectFormatContext::from_node(self.object, self.indent, self.indent_unit);
        let mut formatted_items = self.format_items(&context)?;
        self.normalize_terminators(&mut formatted_items, context.is_braceless);
        self.normalize_standalone_blocks(&mut formatted_items, &context.entry_indent);
        Ok(SyntaxKind::Object.create_node(
            crate::edit::high_level::rebuild_object_children_for_format(
                context.lbrace,
                formatted_items,
                context.rbrace,
            ),
        ))
    }

    fn format_items(&self, context: &ObjectFormatContext) -> Result<Vec<ObjectItem>, FormatError> {
        let mut formatted_items = Vec::new();
        let mut entry_positions = Vec::new();

        for item in context.items.clone() {
            match item {
                ObjectItem::Entry(mut entry) => {
                    entry = self.format_entry(entry, context, entry_positions.is_empty())?;
                    entry_positions.push(formatted_items.len());
                    formatted_items.push(ObjectItem::Entry(entry));
                }
                ObjectItem::Standalone(block) => {
                    formatted_items.push(ObjectItem::Standalone(block))
                }
            }
        }

        Ok(formatted_items)
    }

    fn format_entry(
        &self,
        mut entry: crate::edit::high_level::ObjectEntry,
        context: &ObjectFormatContext,
        is_first_entry: bool,
    ) -> Result<crate::edit::high_level::ObjectEntry, FormatError> {
        if let Some(pair_green) = entry.pair.as_node() {
            let pair_red = RedNode::new_root(pair_green.clone());
            entry.pair = NodeOrToken::Node(format_pair_node(
                &pair_red,
                &context.entry_indent,
                self.newline,
                self.indent_unit,
            )?);
        }

        if !context.is_braceless && entry.leading_line_trivia.is_empty() {
            entry.leading_line_trivia = if is_first_entry {
                newline_indent_elements(self.newline, &context.entry_indent)
            } else if !context.entry_indent.is_empty() {
                vec![NodeOrToken::Token(
                    SyntaxKind::Whitespace.create_token(&context.entry_indent),
                )]
            } else {
                Vec::new()
            };
        }

        Ok(entry)
    }

    fn normalize_terminators(&self, items: &mut [ObjectItem], is_braceless: bool) {
        let entry_positions: Vec<usize> = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                ObjectItem::Entry(_) => Some(index),
                ObjectItem::Standalone(_) => None,
            })
            .collect();

        for window in entry_positions.windows(2) {
            if let ObjectItem::Entry(entry) = &mut items[window[0]] {
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
                .any(|item| matches!(item, ObjectItem::Standalone(_)));
            if !has_trailing_standalone && !is_braceless {
                if let ObjectItem::Entry(last_entry) = &mut items[last_idx] {
                    if matches!(last_entry.terminator, EntryTerminator::EndOfContainer) {
                        last_entry.terminator = EntryTerminator::Newline(newline_indent_elements(
                            self.newline,
                            self.indent,
                        ));
                    }
                }
            }
        }
    }

    fn normalize_standalone_blocks(&self, items: &mut [ObjectItem], entry_indent: &str) {
        for item in items {
            if let ObjectItem::Standalone(block) = item {
                normalize_standalone_block(block, entry_indent, self.newline);
            }
        }
    }
}

#[derive(Clone)]
struct ObjectFormatContext {
    lbrace: Option<GreenElement>,
    rbrace: Option<GreenElement>,
    items: Vec<ObjectItem>,
    entry_indent: String,
    is_braceless: bool,
}

impl ObjectFormatContext {
    fn from_node(object: &RedNode, indent: &str, indent_unit: &str) -> Self {
        let object_green = object.to_green();
        let children: Vec<GreenElement> = object_green.children().map(clone_element).collect();
        let lbrace = children
            .iter()
            .find(|elem| is_token_of_kind(elem, SyntaxKind::LBrace))
            .cloned();
        let rbrace = children
            .iter()
            .find(|elem| is_token_of_kind(elem, SyntaxKind::RBrace))
            .cloned();
        let items = build_object_items(&children);
        let default_entry_indent = child_indent(indent, indent_unit);
        let entry_indent = detect_object_entry_indent(&items)
            .filter(|detected| entry_indent_matches_context(detected, indent))
            .unwrap_or(default_entry_indent);
        let is_braceless = lbrace.is_none() && rbrace.is_none();

        Self {
            lbrace,
            rbrace,
            items,
            entry_indent,
            is_braceless,
        }
    }
}

pub(crate) fn format_object_node(
    object: &RedNode,
    indent: &str,
    newline: &str,
    indent_unit: &str,
) -> Result<GreenNode, FormatError> {
    ObjectFormatPass::new(object, indent, newline, indent_unit).format()
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
