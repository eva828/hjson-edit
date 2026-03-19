use crate::document_style::DetectNewline;

use crate::edit::high_level::{
    build_array_items, clone_element, create_value_node, is_token_of_kind, ArrayEntry, ArrayItem,
    EntryTerminator, HjsonDocument, HjsonEditError, HjsonValue, StringStyle,
};
use crate::syntax::ext::GreenElement;
use crate::syntax::ext::{RedNode, RedNodeExt};
use crate::syntax::kind::SyntaxKind;
use rowan::{GreenNode, NodeOrToken};

pub(crate) struct ArrayEditor;

impl ArrayEditor {
    pub(crate) fn remove_entry(
        arr_node: &RedNode,
        index: usize,
    ) -> Result<GreenNode, HjsonEditError> {
        let mut context = ArrayLayoutContext::from_node(arr_node);
        let mut found = false;
        let mut entry_index = 0usize;
        let mut pending_leading = Vec::new();
        let mut new_items = Vec::new();

        for item in std::mem::take(&mut context.items) {
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

        Ok(context.rebuild(new_items))
    }

    pub(crate) fn insert_entry(
        document: &HjsonDocument,
        arr_node: &RedNode,
        index: usize,
        value: &HjsonValue,
        string_style: StringStyle,
    ) -> Result<GreenNode, HjsonEditError> {
        let mut context = ArrayLayoutContext::from_node(arr_node);
        let newline = arr_node
            .try_detect_newline()
            .unwrap_or_else(|| document.try_detect_newline().unwrap_or_default())
            .as_str();
        let mut planner = ArrayInsertPlanner::new(index, newline);
        let mut new_items = Vec::new();

        for item in std::mem::take(&mut context.items) {
            match item {
                ArrayItem::Entry(entry) => {
                    planner.record_indent(&entry);
                    planner.insert_before_if_needed(&mut new_items, &entry, value, string_style)?;
                    new_items.push(ArrayItem::Entry(entry));
                    planner.entry_index += 1;
                }
                ArrayItem::Standalone(block) => planner.route_standalone(&mut new_items, block),
            }
        }

        planner.finish(&mut new_items, value, string_style, &context)?;
        Ok(context.rebuild(new_items))
    }
}

struct ArrayLayoutContext {
    lbracket: Option<GreenElement>,
    rbracket: Option<GreenElement>,
    items: Vec<ArrayItem>,
}

impl ArrayLayoutContext {
    fn from_node(arr_node: &RedNode) -> Self {
        let arr_green: GreenNode = arr_node.to_green();
        let children: Vec<GreenElement> = arr_green.children().map(clone_element).collect();
        Self {
            lbracket: children
                .iter()
                .find(|elem| is_token_of_kind(elem, SyntaxKind::LBracket))
                .cloned(),
            rbracket: children
                .iter()
                .find(|elem| is_token_of_kind(elem, SyntaxKind::RBracket))
                .cloned(),
            items: build_array_items(&children),
        }
    }

    fn rebuild(self, items: Vec<ArrayItem>) -> GreenNode {
        SyntaxKind::Array.create_node(crate::edit::high_level::rebuild_array_children_for_format(
            self.lbracket,
            items,
            self.rbracket,
        ))
    }

    fn has_closing_bracket(&self) -> bool {
        self.rbracket
            .as_ref()
            .is_some_and(|elem| is_token_of_kind(elem, SyntaxKind::RBracket))
    }
}

struct ArrayInsertPlanner {
    insert_at: usize,
    is_append: bool,
    inserted: bool,
    entry_index: usize,
    newline: String,
    last_entry_indent: Option<GreenElement>,
    trailing_standalone: Vec<ArrayItem>,
}

impl ArrayInsertPlanner {
    fn new(index: usize, newline: &str) -> Self {
        Self {
            insert_at: index,
            is_append: index == usize::MAX,
            inserted: false,
            entry_index: 0,
            newline: newline.to_string(),
            last_entry_indent: None,
            trailing_standalone: Vec::new(),
        }
    }

    fn record_indent(&mut self, entry: &ArrayEntry) {
        if let Some(indent) = entry
            .leading_line_trivia
            .iter()
            .rev()
            .find(|elem| is_token_of_kind(elem, SyntaxKind::Whitespace))
            .cloned()
        {
            self.last_entry_indent = Some(indent);
        }
    }

    fn insert_before_if_needed(
        &mut self,
        new_items: &mut Vec<ArrayItem>,
        entry: &ArrayEntry,
        value: &HjsonValue,
        string_style: StringStyle,
    ) -> Result<(), HjsonEditError> {
        if self.is_append || self.inserted || self.entry_index != self.insert_at {
            return Ok(());
        }

        new_items.push(self.create_entry(
            entry.leading_line_trivia.clone(),
            EntryTerminator::Newline(vec![NodeOrToken::Token(
                SyntaxKind::Newline.create_token(&self.newline),
            )]),
            value,
            string_style,
        )?);
        self.inserted = true;
        Ok(())
    }

    fn route_standalone(
        &mut self,
        new_items: &mut Vec<ArrayItem>,
        block: crate::edit::high_level::StandaloneTriviaBlock,
    ) {
        if self.is_append {
            self.trailing_standalone.push(ArrayItem::Standalone(block));
        } else {
            new_items.push(ArrayItem::Standalone(block));
        }
    }

    fn finish(
        mut self,
        new_items: &mut Vec<ArrayItem>,
        value: &HjsonValue,
        string_style: StringStyle,
        context: &ArrayLayoutContext,
    ) -> Result<(), HjsonEditError> {
        if !self.inserted {
            new_items.push(self.create_entry(
                self.leading_line_trivia(new_items),
                self.terminator(context),
                value,
                string_style,
            )?);
        }

        new_items.append(&mut self.trailing_standalone);
        Ok(())
    }

    fn leading_line_trivia(&self, items: &[ArrayItem]) -> Vec<GreenElement> {
        if items.iter().any(|item| matches!(item, ArrayItem::Entry(_))) {
            self.last_entry_indent.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    fn terminator(&self, context: &ArrayLayoutContext) -> EntryTerminator {
        if self.trailing_standalone.is_empty() {
            if context.has_closing_bracket() {
                return EntryTerminator::Newline(vec![NodeOrToken::Token(
                    SyntaxKind::Newline.create_token(&self.newline),
                )]);
            }
            return EntryTerminator::EndOfContainer;
        }

        EntryTerminator::Newline(vec![NodeOrToken::Token(
            SyntaxKind::Newline.create_token(&self.newline),
        )])
    }

    fn create_entry(
        &self,
        leading_line_trivia: Vec<GreenElement>,
        terminator: EntryTerminator,
        value: &HjsonValue,
        string_style: StringStyle,
    ) -> Result<ArrayItem, HjsonEditError> {
        Ok(ArrayItem::Entry(ArrayEntry {
            leading_line_trivia,
            value: NodeOrToken::Node(create_value_node(value, string_style, None)?),
            trailing_inline_trivia: Vec::new(),
            terminator,
        }))
    }
}

pub(crate) fn remove_array_entry(
    arr_node: &RedNode,
    index: usize,
) -> Result<GreenNode, HjsonEditError> {
    ArrayEditor::remove_entry(arr_node, index)
}

pub(crate) fn insert_array_entry_structural(
    document: &HjsonDocument,
    arr_node: &RedNode,
    index: usize,
    value: &HjsonValue,
    string_style: StringStyle,
) -> Result<GreenNode, HjsonEditError> {
    ArrayEditor::insert_entry(document, arr_node, index, value, string_style)
}
