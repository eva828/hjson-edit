use crate::edit::api::{create_node, create_token};
use crate::edit::high_level::structure::{
    indentation_from_standalone_block, whitespace_after_last_newline,
};
use crate::edit::high_level::{
    build_object_items, clone_element, create_pair_node, extract_key_text, is_token_of_kind,
    rebuild_object_children_for_format, replace_pair_value, EntryTerminator, HjsonEditError,
    HjsonValue, ObjectEntry, ObjectItem, StringStyle,
};
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

pub(crate) struct ObjectEditor;

impl ObjectEditor {
    pub(crate) fn set_entry(
        obj_node: &RedNode,
        key: &str,
        value: &HjsonValue,
        string_style: StringStyle,
        base_indent: &str,
        indent_unit: &str,
        newline: &str,
    ) -> Result<GreenNode, HjsonEditError> {
        let mut context = ObjectLayoutContext::from_node(obj_node);
        let entry_indent = context.entry_indent(base_indent, indent_unit);
        let has_braces = context.has_braces();

        let mut replaced = false;
        let mut new_items = Vec::new();

        for item in std::mem::take(&mut context.items) {
            match item {
                ObjectItem::Entry(mut entry) => {
                    if Self::pair_matches_key(&entry, key) {
                        entry.pair = NodeOrToken::Node(replace_pair_value(
                            Self::pair_green(&entry)?,
                            value,
                            string_style,
                        )?);
                        replaced = true;
                    }
                    new_items.push(ObjectItem::Entry(entry));
                }
                ObjectItem::Standalone(block) => new_items.push(ObjectItem::Standalone(block)),
            }
        }

        if !replaced {
            ObjectInsertPlanner::new(&InsertFormat {
                has_braces,
                newline,
                entry_indent: &entry_indent,
                closing_indent: base_indent,
            })
            .insert_entry(&mut new_items, key, value, string_style)?;
        }

        Ok(context.rebuild(new_items))
    }

    pub(crate) fn remove_entry(obj_node: &RedNode, key: &str) -> Result<GreenNode, HjsonEditError> {
        let mut context = ObjectLayoutContext::from_node(obj_node);
        let mut found = false;
        let mut pending_terminator: Option<EntryTerminator> = None;
        let mut has_surviving_entry = false;
        let mut new_items = Vec::new();

        for item in std::mem::take(&mut context.items) {
            match item {
                ObjectItem::Entry(entry) if Self::pair_matches_key(&entry, key) => {
                    found = true;
                    if !has_surviving_entry {
                        pending_terminator = Some(entry.terminator.clone());
                    }
                }
                ObjectItem::Entry(entry) => {
                    let entry = PendingTerminator::apply(entry, &mut pending_terminator);
                    new_items.push(ObjectItem::Entry(entry));
                    has_surviving_entry = true;
                }
                ObjectItem::Standalone(block) => new_items.push(ObjectItem::Standalone(block)),
            }
        }

        if !found {
            return Err(HjsonEditError::InvalidPath {
                path: key.to_string(),
            });
        }

        Ok(context.rebuild(new_items))
    }

    fn pair_matches_key(entry: &ObjectEntry, key: &str) -> bool {
        let Some(pair_green) = entry.pair.as_node() else {
            return false;
        };
        let pair_red = RedNode::new_root(pair_green.clone());
        let Some(key_node) = pair_red.child_by_kind(SyntaxKind::Key) else {
            return false;
        };
        extract_key_text(&key_node).as_deref() == Some(key)
    }

    fn pair_green(entry: &ObjectEntry) -> Result<&GreenNode, HjsonEditError> {
        entry.pair.as_node().ok_or(HjsonEditError::InvalidNodeKind {
            expected: SyntaxKind::Pair,
            found: SyntaxKind::Error,
        })
    }
}

struct ObjectLayoutContext {
    lbrace: Option<GreenElement>,
    rbrace: Option<GreenElement>,
    items: Vec<ObjectItem>,
}

impl ObjectLayoutContext {
    fn from_node(obj_node: &RedNode) -> Self {
        let obj_green: GreenNode = obj_node.to_green();
        let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();
        Self {
            lbrace: children
                .iter()
                .find(|elem| is_token_of_kind(elem, SyntaxKind::LBrace))
                .cloned(),
            rbrace: children
                .iter()
                .find(|elem| is_token_of_kind(elem, SyntaxKind::RBrace))
                .cloned(),
            items: build_object_items(&children),
        }
    }

    fn has_braces(&self) -> bool {
        self.lbrace.is_some() && self.rbrace.is_some()
    }

    fn entry_indent(&self, base_indent: &str, indent_unit: &str) -> String {
        detect_object_entry_indent(&self.items).unwrap_or_else(|| {
            if self.has_braces() {
                format!("{}{}", base_indent, indent_unit)
            } else {
                String::new()
            }
        })
    }

    fn rebuild(self, items: Vec<ObjectItem>) -> GreenNode {
        create_node(
            SyntaxKind::Object,
            rebuild_object_children_for_format(self.lbrace, items, self.rbrace),
        )
    }
}

struct ObjectInsertPlanner<'a> {
    has_braces: bool,
    newline: &'a str,
    entry_indent: &'a str,
    closing_indent: &'a str,
}

impl<'a> ObjectInsertPlanner<'a> {
    fn new(format: &'a InsertFormat<'a>) -> Self {
        Self {
            has_braces: format.has_braces,
            newline: format.newline,
            entry_indent: format.entry_indent,
            closing_indent: format.closing_indent,
        }
    }

    fn insert_entry(
        &self,
        items: &mut Vec<ObjectItem>,
        key: &str,
        value: &HjsonValue,
        string_style: StringStyle,
    ) -> Result<(), HjsonEditError> {
        let new_entry = ObjectItem::Entry(ObjectEntry {
            leading_line_trivia: self.leading_line_trivia(items),
            leading_comments: Vec::new(),
            pair: NodeOrToken::Node(create_pair_node(key, value, string_style, None)?),
            trailing_inline_trivia: Vec::new(),
            terminator: self.terminator(),
        });

        let mut trailing_standalone = Vec::new();
        while matches!(items.last(), Some(ObjectItem::Standalone(_))) {
            trailing_standalone.push(items.pop().expect("standalone item exists"));
        }
        trailing_standalone.reverse();

        items.push(new_entry);
        items.extend(trailing_standalone);
        Ok(())
    }

    fn leading_line_trivia(&self, items: &[ObjectItem]) -> Vec<GreenElement> {
        let has_existing_entries = items
            .iter()
            .any(|item| matches!(item, ObjectItem::Entry(_)));
        let last_entry_terminator = items.iter().rev().find_map(|item| match item {
            ObjectItem::Entry(entry) => Some(entry.terminator.clone()),
            ObjectItem::Standalone(_) => None,
        });

        if has_existing_entries {
            if let Some(EntryTerminator::Newline(suffix)) = last_entry_terminator {
                return self.leading_trivia_after_newline_suffix(&suffix);
            }
            return self.newline_with_indent();
        }

        if self.has_braces {
            self.newline_with_indent()
        } else {
            Vec::new()
        }
    }

    fn terminator(&self) -> EntryTerminator {
        if !self.has_braces {
            return EntryTerminator::EndOfContainer;
        }

        let mut suffix = vec![NodeOrToken::Token(create_token(
            SyntaxKind::Newline,
            self.newline,
        ))];
        if !self.closing_indent.is_empty() {
            suffix.push(NodeOrToken::Token(create_token(
                SyntaxKind::Whitespace,
                self.closing_indent,
            )));
        }
        EntryTerminator::Newline(suffix)
    }

    fn newline_with_indent(&self) -> Vec<GreenElement> {
        let mut trivia = vec![NodeOrToken::Token(create_token(
            SyntaxKind::Newline,
            self.newline,
        ))];
        trivia.extend(self.indent_only());
        trivia
    }

    fn leading_trivia_after_newline_suffix(&self, suffix: &[GreenElement]) -> Vec<GreenElement> {
        let mut saw_newline = false;
        let mut trailing_whitespace = Vec::new();

        for elem in suffix {
            if is_token_of_kind(elem, SyntaxKind::Newline) {
                saw_newline = true;
                trailing_whitespace.clear();
                continue;
            }

            if saw_newline && is_token_of_kind(elem, SyntaxKind::Whitespace) {
                trailing_whitespace.push(elem.clone());
            }
        }

        // If a newline was seen and there is trailing whitespace after it, use that whitespace.
        // If a newline was seen but no whitespace follows it, return only the indent (no extra newline)
        // so the existing newline from the previous terminator is kept and the new entry is
        // indented correctly. If no newline was seen at all, return newline + indent.
        if saw_newline {
            if !trailing_whitespace.is_empty() {
                trailing_whitespace
            } else {
                self.indent_only()
            }
        } else {
            self.newline_with_indent()
        }
    }

    fn indent_only(&self) -> Vec<GreenElement> {
        if self.entry_indent.is_empty() {
            Vec::new()
        } else {
            vec![NodeOrToken::Token(create_token(
                SyntaxKind::Whitespace,
                self.entry_indent,
            ))]
        }
    }
}

struct PendingTerminator;

impl PendingTerminator {
    fn apply(entry: ObjectEntry, pending: &mut Option<EntryTerminator>) -> ObjectEntry {
        let Some(EntryTerminator::Newline(suffix)) = pending.take() else {
            return entry;
        };

        let mut modified_entry = entry;
        if !modified_entry
            .leading_line_trivia
            .iter()
            .any(|elem| is_token_of_kind(elem, SyntaxKind::Newline))
        {
            let mut merged = suffix;
            merged.extend(modified_entry.leading_line_trivia);
            modified_entry.leading_line_trivia = merged;
        }
        modified_entry
    }
}

pub(crate) fn remove_object_entry(
    obj_node: &RedNode,
    key: &str,
) -> Result<GreenNode, HjsonEditError> {
    ObjectEditor::remove_entry(obj_node, key)
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
    ObjectEditor::set_entry(
        obj_node,
        key,
        value,
        string_style,
        base_indent,
        indent_unit,
        newline,
    )
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
