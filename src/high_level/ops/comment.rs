use crate::high_level::{path::HjsonPath, HjsonDocument};

pub trait HjsonDocumentCommentEdit {
    fn get(&self, path: impl Into<HjsonPath>) -> Option<String>;
    fn set(self, path: impl Into<HjsonPath>, comment: impl Into<String>) -> Self;
    fn remove(self, path: impl Into<HjsonPath>) -> Self;
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get(&self, path: impl Into<HjsonPath>) -> Option<String> {
        unimplemented!()
    }

    fn set(self, path: impl Into<HjsonPath>, comment: impl Into<String>) -> Self {
        unimplemented!()
    }

    fn remove(self, path: impl Into<HjsonPath>) -> Self {
        unimplemented!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CommentStyle {
    #[default]
    Preserve,
    Auto,
    Line,
    Hash,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommentPath {
    /// キーの直前コメント
    Key { dot_path: String },
    /// キーのインラインコメント
    Inline { dot_path: String },
    /// ドキュメント先頭コメント
    Header,
    /// ドキュメント末尾コメント
    Footer,
}
