use crate::{
    high_level::{
        path::{HjsonPath, ObjectPathSegment, PathSegment},
        HjsonDocument,
    },
    HjsonEditError,
};

/// Style to use when writing comments into the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentStyle {
    /// Preserve whatever style the comment already has; default to block when creating new.
    #[default]
    Preserve,
    /// `// …`
    Line,
    /// `# …`
    Hash,
    /// `/* … */`
    Block,
}

/// Identifies the target of a comment operation.
#[derive(Debug, Clone, PartialEq)]
pub enum CommentPath {
    /// The leading comment of the key at the given dot-path.
    Key { dot_path: String },
    /// The inline (trailing) comment of the value at the given dot-path.
    Inline { dot_path: String },
    /// The document-level header comment (before the root value).
    Header,
    /// The document-level footer comment (after the root value).
    Footer,
}

pub trait HjsonDocumentCommentEdit {
    fn get(&self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<Option<String>, HjsonEditError>;
    fn set(
        self,
        path: impl TryInto<HjsonPath, Error = HjsonEditError>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get(&self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<Option<String>, HjsonEditError> {
        let path = path.try_into()?;

        if is_document_comment_path(&path, ":header") {
            // Return the header comment (before the root value).
            unimplemented!()
        }

        if is_document_comment_path(&path, ":footer") {
            // Return the footer comment (after the root value).
            unimplemented!()
        }

        // Return the leading comment of the key at `path`.
        unimplemented!()
    }

    fn set(
        self,
        path: impl TryInto<HjsonPath, Error = HjsonEditError>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.try_into()?;
        let _comment = comment.into();

        if is_document_comment_path(&path, ":header") {
            // Replace or insert the header comment.
            unimplemented!()
        }

        if is_document_comment_path(&path, ":footer") {
            // Replace or insert the footer comment.
            unimplemented!()
        }

        // Set the leading comment of the key at `path`.
        unimplemented!()
    }

    fn remove(self, path: impl TryInto<HjsonPath, Error = HjsonEditError>) -> Result<Self, HjsonEditError> {
        let path = path.try_into()?;

        if is_document_comment_path(&path, ":header") {
            // Remove the header comment.
            unimplemented!()
        }

        if is_document_comment_path(&path, ":footer") {
            // Remove the footer comment.
            unimplemented!()
        }

        // Remove the leading comment of the key at `path`.
        unimplemented!()
    }
}

/// Returns `true` when `path` is a single-segment object path matching `needle`
/// (e.g. `":header"` or `":footer"`).
fn is_document_comment_path(path: &HjsonPath, needle: &str) -> bool {
    matches!(
        path.segments.as_slice(),
        [PathSegment::Object(ObjectPathSegment::Key(key))] if key == needle
    )
}
