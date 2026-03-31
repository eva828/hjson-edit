use rowan::{GreenNode, GreenToken, Language, NodeOrToken};

use crate::{
    format::{FormatConfig, Formatter},
    high_level::{
        ast::{AstNode, Pair},
        error::HjsonEditError,
        path::{ArrayPathSegment, HjsonPath, ObjectPathSegment, PathSegment},
        style::DocumentStyle,
        value::HjsonValue,
        HjsonDocument,
    },
    syntax::HjsonLanguage,
    RedNode, SyntaxKind,
};

pub trait HjsonDocumentObjectEdit {
    fn get(&self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<HjsonValue, HjsonEditError>;
    fn set(
        self,
        path: impl TryInto<HjsonPath, Error = HjsonEditError>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringStyle {
    #[default]
    Preserve,
    DoubleQuoted,
    Block,
    Unquoted,
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(&self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<HjsonValue, HjsonEditError> {
        let path = path.try_into()?;
        let node = self.get_internal(&path)?;
        Ok(HjsonValue::from_rednode(node))
    }

    fn set(
        mut self,
        path: impl TryInto<HjsonPath, Error = HjsonEditError>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.try_into()?;
        let new_value = value.into();

        if path.segments.is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let last_segment = path.segments.last().unwrap();

        match last_segment {
            PathSegment::Array(ArrayPathSegment::Push) => {
                self.set_array_push(&path, new_value)?;
                let format_config = self.current_format_config();
                self.reformat_current_document(format_config);
            }
            PathSegment::Array(ArrayPathSegment::InsertAt(index)) => {
                self.set_array_insert_at(&path, *index, new_value)?;
                let format_config = self.current_format_config();
                self.reformat_current_document(format_config);
            }
            _ => match self.set_existing(&path, new_value.clone()) {
                Ok(()) => {}
                Err(HjsonEditError::PathNotFound(_)) => {
                    self.set_with_missing_parents(&path, new_value)?;
                }
                Err(err) => return Err(err),
            },
        }

        Ok(self)
    }

    fn remove(mut self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.try_into()?;

        if path.segments.is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let last_segment = path.segments.last().unwrap();

        match last_segment {
            PathSegment::Array(ArrayPathSegment::Index(index)) => {
                self.remove_array_at(&path, *index)?;
            }
            _ => {
                self.remove_pair(&path)?;
            }
        }
        Ok(self)
    }
}

impl HjsonDocument {
    fn current_format_config(&self) -> FormatConfig {
        let source = self.to_string();
        let style = DocumentStyle::new(&source);
        FormatConfig {
            indent_unit: style.indent_unit(),
            newline: style.newline().to_string(),
            space: " ".to_string(),
        }
    }

    fn reformat_current_document(&mut self, config: FormatConfig) {
        let root_green: GreenNode = self.root.green().into();
        let formatted = Formatter::format(root_green, config);
        self.root = RedNode::new_root(formatted);
    }

    fn set_existing(&mut self, path: &HjsonPath, value: HjsonValue) -> Result<(), HjsonEditError> {
        let existing_style = self.get_internal(path).ok().and_then(|node| {
            let mut current = node;
            while current.kind() != SyntaxKind::Value {
                current = current.parent()?;
            }

            current
                .children_with_tokens()
                .next()
                .and_then(|child| match child.kind() {
                    SyntaxKind::String => Some(StringStyle::DoubleQuoted),
                    SyntaxKind::BlockString => Some(StringStyle::Block),
                    SyntaxKind::UnquotedString => Some(StringStyle::Unquoted),
                    _ => None,
                })
        });
        let new_value =
            value.into_greennode_with_style(self.current_string_style(), existing_style);

        let target = self.get_internal(path)?;
        let mut pair = target;
        while pair.kind() != SyntaxKind::Pair {
            pair = pair
                .parent()
                .ok_or_else(|| HjsonEditError::ParentNotFound)?;
        }

        let value_index = pair
            .green()
            .children()
            .enumerate()
            .find_map(|(idx, child)| {
                child
                    .as_node()
                    .filter(|node| HjsonLanguage::kind_from_raw(node.kind()) == SyntaxKind::Value)
                    .map(|_| idx)
            })
            .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Value,
                found: pair.kind(),
            })?;

        // Replace the Value child inside the Pair, then propagate the change all the way
        // to the root using replace_with so that arbitrary nesting depths work correctly.
        let updated_pair = pair
            .green()
            .splice_children(value_index..value_index + 1, vec![new_value.into()]);
        let new_root = pair.replace_with(updated_pair);
        self.root = RedNode::new_root(new_root);
        Ok(())
    }

    fn set_with_missing_parents(
        &mut self,
        path: &HjsonPath,
        value: HjsonValue,
    ) -> Result<(), HjsonEditError> {
        // For any path whose leaf segment is an object key, try a text-based insertion that
        // preserves all comments and formatting.  The insertion point is chosen just before
        // the line that contains the closing `}` of the enclosing object so that:
        //   • the closing-brace indentation is inspected to derive the child indentation
        //     (one extra indent_unit deeper), and
        //   • the existing content (comments, other keys) is left untouched.
        if let Some(PathSegment::Object(ObjectPathSegment::Key(key))) = path.segments.last() {
            let source = self.to_string();
            let style = DocumentStyle::new(&source);
            let newline = style.newline();
            let indent_unit = style.indent_unit();

            // Locate the closing `}` of the target object and the start of its line.
            // Returns (insert_at, brace_indent) where `insert_at` is the byte offset of
            // the first character of the `}` line (i.e. its indentation prefix).
            let location: Option<(usize, String)> = if path.segments.len() == 1 {
                // Root object: search in the whole document.
                source.rfind('}').map(|abs_close| {
                    let before = &source[..abs_close];
                    find_brace_line_start(before, abs_close)
                })
            } else {
                // Nested: resolve the parent node and search only within it.
                let parent_path = HjsonPath {
                    segments: path.segments[..path.segments.len() - 1].to_vec(),
                };
                self.get_internal(&parent_path).ok().and_then(|parent_node| {
                    let start: usize = parent_node.text_range().start().into();
                    let end: usize = parent_node.text_range().end().into();
                    let parent_text = &source[start..end];
                    parent_text.rfind('}').and_then(|brace_rel| {
                        let abs_close = start + brace_rel;
                        let before_in_parent = &source[start..abs_close];
                        let (insert_at, brace_indent) =
                            find_brace_line_start(before_in_parent, abs_close);
                        // For an inline `{}` there is no newline → fall through to
                        // the semantic path.
                        if insert_at == abs_close {
                            None
                        } else {
                            Some((insert_at, brace_indent))
                        }
                    })
                })
            };

            if let Some((insert_at, brace_indent)) = location {
                let child_indent = format!("{}{}", brace_indent, indent_unit);
                let before_content = &source[..insert_at];
                let value_text = value.to_pretty_hjson_string_with_style(
                    &indent_unit,
                    newline,
                    0,
                    self.current_string_style(),
                );
                let insertion_prefix = if before_content.ends_with(newline) {
                    ""
                } else {
                    newline
                };
                let insertion = format!(
                    "{}{}{}: {}{}",
                    insertion_prefix,
                    child_indent,
                    Self::render_object_key_for_insert(key),
                    value_text,
                    newline
                );
                let text =
                    format!("{}{}{}", before_content, insertion, &source[insert_at..]);
                self.replace_document_from_text(&text)?;
                return Ok(());
            }
        }

        // Fallback: parent path does not yet exist.  Reconstruct the whole document from
        // semantic values (comments in affected subtrees are lost).
        let root_value = self.semantic_root_value()?;
        let updated = self.apply_path_to_value(root_value, &path.segments, value);
        let config = self.current_format_config();
        let text = updated.to_pretty_hjson_string_with_style(
            &config.indent_unit,
            &config.newline,
            0,
            self.current_string_style(),
        );
        self.replace_document_from_text(&text)?;
        Ok(())
    }

    // Render an object key in HJSON syntax.
    fn render_object_key_for_insert(key: &str) -> String {
        if key
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            key.to_string()
        } else {
            format!("\"{}\"", key.replace('\\', "\\\\").replace('"', "\\\""))
        }
    }

    fn semantic_root_value(&self) -> Result<HjsonValue, HjsonEditError> {
        let root_child = self
            .root()
            .children()
            .next()
            .ok_or_else(|| HjsonEditError::PathNotFound(HjsonPath { segments: vec![] }))?;

        Ok(HjsonValue::from_rednode(root_child))
    }

    fn replace_document_from_text(&mut self, text: &str) -> Result<(), HjsonEditError> {
        let parsed = crate::HjsonDocument::parse(text)?;
        self.root = parsed.root().clone();
        Ok(())
    }

    fn apply_path_to_value(
        &self,
        mut current: HjsonValue,
        segments: &[PathSegment],
        leaf: HjsonValue,
    ) -> HjsonValue {
        if segments.is_empty() {
            return leaf;
        }

        match &segments[0] {
            PathSegment::Object(ObjectPathSegment::Key(key)) => {
                if !matches!(current, HjsonValue::Object(_)) {
                    current = HjsonValue::Object(Vec::new());
                }

                let HjsonValue::Object(entries) = &mut current else {
                    unreachable!()
                };

                if segments.len() == 1 {
                    if let Some((_, value)) = entries.iter_mut().find(|(k, _)| k == key) {
                        *value = leaf;
                    } else {
                        entries.push((key.clone(), leaf));
                    }
                    current
                } else {
                    let index = if let Some(index) = entries.iter().position(|(k, _)| k == key) {
                        index
                    } else {
                        entries.push((key.clone(), HjsonValue::Null));
                        entries.len() - 1
                    };
                    let next = entries[index].1.clone();
                    entries[index].1 = self.apply_path_to_value(next, &segments[1..], leaf);
                    current
                }
            }
            PathSegment::Array(segment) => {
                if !matches!(current, HjsonValue::Array(_)) {
                    current = HjsonValue::Array(Vec::new());
                }

                let HjsonValue::Array(items) = &mut current else {
                    unreachable!()
                };

                match segment {
                    ArrayPathSegment::Push => {
                        if segments.len() == 1 {
                            items.push(leaf);
                        } else {
                            items.push(HjsonValue::Null);
                            let last = items.len() - 1;
                            let next = items[last].clone();
                            items[last] = self.apply_path_to_value(next, &segments[1..], leaf);
                        }
                    }
                    ArrayPathSegment::Index(index) => {
                        if *index >= items.len() {
                            items.resize_with(*index + 1, || HjsonValue::Null);
                        }
                        if segments.len() == 1 {
                            items[*index] = leaf;
                        } else {
                            let next = items[*index].clone();
                            items[*index] = self.apply_path_to_value(next, &segments[1..], leaf);
                        }
                    }
                    ArrayPathSegment::InsertAt(index) => {
                        let insert_at = (*index).min(items.len());
                        if segments.len() == 1 {
                            items.insert(insert_at, leaf);
                        } else {
                            items.insert(insert_at, HjsonValue::Null);
                            let next = items[insert_at].clone();
                            items[insert_at] = self.apply_path_to_value(next, &segments[1..], leaf);
                        }
                    }
                }

                current
            }
        }
    }

    fn set_array_push(
        &mut self,
        path: &HjsonPath,
        value: HjsonValue,
    ) -> Result<(), HjsonEditError> {
        if path.segments.len() < 2 {
            return Err(HjsonEditError::PathNotFound(path.clone()));
        }

        let array_path = HjsonPath {
            segments: path.segments[..path.segments.len() - 1].to_vec(),
        };

        let mut array_node = self.get_internal(&array_path)?;
        if array_node.kind() == SyntaxKind::Value {
            array_node = array_node
                .children()
                .find(|c| c.kind() == SyntaxKind::Array)
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Array,
                    found: array_node.kind(),
                })?;
        }
        if array_node.kind() != SyntaxKind::Array {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: array_node.kind(),
            });
        }

        let new_element = self.create_value_element(value);
        let new_green = array_node.green().splice_children(
            (array_node.green().children().count() - 1)
                ..(array_node.green().children().count() - 1),
            vec![new_element],
        );
        let new_root = array_node.replace_with(new_green);
        self.root = RedNode::new_root(new_root);
        Ok(())
    }

    fn set_array_insert_at(
        &mut self,
        path: &HjsonPath,
        index: usize,
        value: HjsonValue,
    ) -> Result<(), HjsonEditError> {
        if path.segments.len() < 2 {
            return Err(HjsonEditError::PathNotFound(path.clone()));
        }

        let array_path = HjsonPath {
            segments: path.segments[..path.segments.len() - 1].to_vec(),
        };

        let mut array_node = self.get_internal(&array_path)?;
        if array_node.kind() == SyntaxKind::Value {
            array_node = array_node
                .children()
                .find(|c| c.kind() == SyntaxKind::Array)
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Array,
                    found: array_node.kind(),
                })?;
        }
        if array_node.kind() != SyntaxKind::Array {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: array_node.kind(),
            });
        }

        let value_indices: Vec<usize> = array_node
            .green()
            .children()
            .enumerate()
            .filter(|(_, child)| {
                child
                    .as_node()
                    .map(|n| HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Value)
                    .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect();

        let green_index = if index < value_indices.len() {
            value_indices[index]
        } else {
            array_node.green().children().count() - 1
        };

        let new_element = self.create_value_element(value);
        let new_green = array_node
            .green()
            .splice_children(green_index..green_index, vec![new_element]);
        let new_root = array_node.replace_with(new_green);
        self.root = RedNode::new_root(new_root);
        Ok(())
    }

    fn create_value_element(&self, value: HjsonValue) -> NodeOrToken<GreenNode, GreenToken> {
        let value_node = value.into_greennode_with_style(self.current_string_style(), None);
        value_node.into()
    }

    fn remove_pair(&mut self, path: &HjsonPath) -> Result<(), HjsonEditError> {
        let mut target = self.get_internal(path)?;

        while target.kind() != SyntaxKind::Pair {
            target = target
                .parent()
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Pair,
                    found: target.kind(),
                })?;
        }

        let target_parent = target
            .parent()
            .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Object,
                found: target.kind(),
            })?;

        let target_index = target.index();
        let pair = Pair::cast(target)?;

        let start = pair
            .leading_trivia()?
            .first()
            .map(|t| t.index())
            .unwrap_or(target_index);
        let end = pair
            .trailing_trivia()?
            .last()
            .map(|t| t.index())
            .unwrap_or(target_index);

        target_parent.splice_children(start..end + 1, Vec::new());
        let new_root = target_parent.ancestors().last().unwrap();
        self.root = new_root;
        Ok(())
    }

    fn remove_array_at(&mut self, path: &HjsonPath, index: usize) -> Result<(), HjsonEditError> {
        if path.segments.len() < 2 {
            return Err(HjsonEditError::PathNotFound(path.clone()));
        }

        let array_path = HjsonPath {
            segments: path.segments[..path.segments.len() - 1].to_vec(),
        };

        let mut array_node = self.get_internal(&array_path)?;
        if array_node.kind() == SyntaxKind::Value {
            array_node = array_node
                .children()
                .find(|c| c.kind() == SyntaxKind::Array)
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Array,
                    found: array_node.kind(),
                })?;
        }
        if array_node.kind() != SyntaxKind::Array {
            return Err(HjsonEditError::UnexpectedStructure {
                expected: SyntaxKind::Array,
                found: array_node.kind(),
            });
        }

        let value_indices: Vec<usize> = array_node
            .green()
            .children()
            .enumerate()
            .filter(|(_, child)| {
                child
                    .as_node()
                    .map(|n| HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Value)
                    .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect();

        if index >= value_indices.len() {
            return Err(HjsonEditError::PathNotFound(path.clone()));
        }

        let green_index = value_indices[index];
        let new_green = array_node
            .green()
            .splice_children(green_index..green_index + 1, vec![]);
        let new_root = array_node.replace_with(new_green);
        self.root = RedNode::new_root(new_root);
        let format_config = self.current_format_config();
        self.reformat_current_document(format_config);
        Ok(())
    }
}

/// Given a `before_brace` slice (everything in source before the `}` character)
/// and `abs_close` (absolute byte offset of `}` in the full source), returns
/// `(insert_at, brace_indent)` where:
///   - `insert_at` is the absolute offset of the first character on the `}` line
///     (i.e. immediately after the newline preceding that line).
///   - `brace_indent` is the whitespace between `insert_at` and `abs_close`.
///
/// When there is no preceding newline (inline `{}`), returns
/// `(abs_close, String::new())` as a sentinel so callers can fall back.
fn find_brace_line_start(before_brace: &str, abs_close: usize) -> (usize, String) {
    if let Some(nl_rel) = before_brace.rfind('\n') {
        let line_start = nl_rel + 1; // offset within `before_brace` of the first char on the `}` line
        // `before_brace` starts at `abs_close - before_brace.len()`.
        let base = abs_close - before_brace.len();
        let abs_line_start = base + line_start;
        let brace_indent: String = before_brace[line_start..]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        (abs_line_start, brace_indent)
    } else {
        // No newline found — the `}` is on the same line as `{` (inline object).
        (abs_close, String::new())
    }
}
