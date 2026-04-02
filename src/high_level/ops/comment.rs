use crate::{
    high_level::{
        path::{HjsonCommentPath, HjsonObjectPath, ObjectPathSegment, PathSegment},
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
    fn get(
        &self,
        path: impl TryInto<HjsonCommentPath, Error = HjsonEditError>,
    ) -> Result<Option<String>, HjsonEditError>;
    fn set(
        self,
        path: impl TryInto<HjsonCommentPath, Error = HjsonEditError>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(
        self,
        path: impl TryInto<HjsonCommentPath, Error = HjsonEditError>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get(
        &self,
        path: impl TryInto<HjsonCommentPath, Error = HjsonEditError>,
    ) -> Result<Option<String>, HjsonEditError> {
        let path = path.try_into()?;

        match path {
            HjsonCommentPath::Header => {
                // Return the header comment (before the root value).
                unimplemented!()
            }
            HjsonCommentPath::Footer => {
                // Return the footer comment (after the root value).
                unimplemented!()
            }
            HjsonCommentPath::Before(path) => {
                unimplemented!()
            }
            HjsonCommentPath::Inline(path) => {
                unimplemented!()
            }
        }
    }

    fn set(
        self,
        path: impl TryInto<HjsonCommentPath, Error = HjsonEditError>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.try_into()?;
        let _comment = comment.into();

        match path {
            HjsonCommentPath::Header => {
                // Return the header comment (before the root value).
                unimplemented!()
            }
            HjsonCommentPath::Footer => {
                // Return the footer comment (after the root value).
                unimplemented!()
            }
            HjsonCommentPath::Before(path) => {
                unimplemented!()
            }
            HjsonCommentPath::Inline(path) => {
                unimplemented!()
            }
        }
    }

    fn remove(
        self,
        path: impl TryInto<HjsonCommentPath, Error = HjsonEditError>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.try_into()?;

        match path {
            HjsonCommentPath::Header => {
                // Return the header comment (before the root value).
                unimplemented!()
            }
            HjsonCommentPath::Footer => {
                // Return the footer comment (after the root value).
                unimplemented!()
            }
            HjsonCommentPath::Before(path) => {
                unimplemented!()
            }
            HjsonCommentPath::Inline(path) => {
                unimplemented!()
            }
        }
    }
}
