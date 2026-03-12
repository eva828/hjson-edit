//! Diff utilities for detecting changed lines
//!
//! Provides functionality to detect which lines have changed between two versions
//! of HJSON content, supporting the --line-numbers CLI feature.

use rowan::GreenNode;

/// Information about a single line change
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeInfo {
    pub line: usize,
    pub change_type: ChangeType,
}

/// Type of change detected
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Modified,
    Added,
    Removed,
}

/// Simple line-by-line diff that returns line numbers (1-indexed) that differ
///
/// # Example
/// ```
/// use hjson_edit::diff::simple_line_diff;
///
/// let before = "line1\nline2\nline3";
/// let after = "line1\nmodified\nline3";
/// let changes = simple_line_diff(before, after);
/// assert_eq!(changes, vec![2]);
/// ```
pub fn simple_line_diff(before: &str, after: &str) -> Vec<usize> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    let max_len = before_lines.len().max(after_lines.len());
    let mut changed = Vec::new();

    for i in 0..max_len {
        let b = before_lines.get(i);
        let a = after_lines.get(i);

        if b != a {
            changed.push(i + 1); // 1-indexed
        }
    }

    changed
}

/// Convert a byte offset to a line number (1-indexed)
pub fn offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].lines().count().max(1)
}

/// Find the nearest valid UTF-8 character boundary at or before the given position
pub fn nearest_char_boundary_at_or_before(s: &str, pos: usize) -> usize {
    let pos = pos.min(s.len());
    // Go backwards from pos until we find a valid UTF-8 boundary
    for i in (0..=pos).rev() {
        if s.is_char_boundary(i) {
            return i;
        }
    }
    0
}

/// Detect changed lines between two GreenNode trees
///
/// Compares the text representation of two syntax trees and returns
/// line numbers (1-indexed) where changes occurred.
pub fn detect_changed_lines(before: &GreenNode, after: &GreenNode, _source: &str) -> Vec<usize> {
    use crate::syntax::green::GreenNodeExt;

    let before_text = before.text();
    let after_text = after.text();

    // If identical, no changes
    if before_text == after_text {
        return Vec::new();
    }

    // Simple line-by-line comparison
    let before_lines: Vec<&str> = before_text.lines().collect();
    let after_lines: Vec<&str> = after_text.lines().collect();

    let max_len = before_lines.len().max(after_lines.len());
    let mut changed = Vec::new();

    for i in 0..max_len {
        let b = before_lines.get(i);
        let a = after_lines.get(i);

        if b != a {
            changed.push(i + 1);
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Internal helper function tests - offset_to_line with multibyte characters
    #[test]
    fn offset_to_line_with_multibyte_chars() {
        let source = "{\n  key: \"\u{3042}\u{3044}\u{3046}\"\n}\n";
        let offset = source.find('\u{3042}').unwrap();
        assert_eq!(offset_to_line(source, offset), 2);
    }

    /// Internal helper function tests - nearest_char_boundary edge cases
    #[test]
    fn nearest_char_boundary_edge_cases() {
        // UTF-8 multi-byte character boundary handling
        let s = "\u{00e9}";
        assert_eq!(nearest_char_boundary_at_or_before(s, 1), 0);
        assert_eq!(nearest_char_boundary_at_or_before(s, 2), 2);
        
        // ASCII boundary
        let s = "abc";
        assert_eq!(nearest_char_boundary_at_or_before(s, 1), 1);
        assert_eq!(nearest_char_boundary_at_or_before(s, 0), 0);
    }
}
