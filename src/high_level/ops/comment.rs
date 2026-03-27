use rowan::{GreenNode, GreenNodeBuilder, GreenToken, SyntaxElement, SyntaxToken};

use crate::{
    high_level::{
        ast::{AstNode, Pair},
        path::HjsonPath,
        HjsonDocument,
    },
    FormatConfig, Formatter, HjsonEditError, RedNode, RedToken, SyntaxKind,
};

pub trait HjsonDocumentCommentEdit {
    fn get(&self, path: impl Into<HjsonPath>) -> Result<Option<String>, HjsonEditError>;
    fn set(
        self,
        path: impl Into<HjsonPath>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get(&self, path: impl Into<HjsonPath>) -> Result<Option<String>, HjsonEditError> {
        let path = path.into();
        let mut target = self.get_internal(&path)?;
        while target.kind() != SyntaxKind::Pair {
            target = target
                .parent()
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Pair,
                    found: target.kind(),
                })?;
        }

        let pair = Pair::cast(target)?;

        let trivia = pair.leading_trivia()?;
        let comment = trivia
            .iter()
            .filter(|t| t.kind().is_comment())
            .map(|t| t.to_string())
            .collect::<String>();

        println!("Debug: Found comment for path {:?}: {:?}", path, comment);

        Ok(if comment.is_empty() {
            None
        } else {
            Some(comment)
        })
    }

    fn set(
        mut self,
        path: impl Into<HjsonPath>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.into();
        let mut target = self.get_internal(&path)?;
        while target.kind() != SyntaxKind::Pair {
            target = target
                .parent()
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Pair,
                    found: target.kind(),
                })?;
        }

        let format_config = FormatConfig::default();
        let newline = format_config.create_newline();
        let indent = format_config.create_indent(path.depth());
        let comment = GreenToken::new(
            SyntaxKind::BlockComment.into(),
            &format!("/* {} */", comment.into()),
        );

        let index = target.index();
        let new_green = target.parent().unwrap().green().splice_children(
            index..index,
            vec![comment.into(), newline.into(), indent.into()],
        );
        let new_root = target.parent().unwrap().replace_with(new_green);
        self.root = RedNode::new_root(new_root);

        Ok(self)
    }

    fn remove(mut self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError> {
        let path = path.into();
        let mut target = self.get_internal(&path)?;
        while target.kind() != SyntaxKind::Pair {
            target = target
                .parent()
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Pair,
                    found: target.kind(),
                })?;
        }

        let pair = Pair::cast(target)?;

        let trivia = pair.leading_trivia()?;

        for t in trivia {
            if t.kind().is_comment() {
                let index = t.index();
                let new_green = pair
                    .node()
                    .parent()
                    .unwrap()
                    .green()
                    .splice_children(index..=index, vec![]);
                let new_root = pair.node().parent().unwrap().replace_with(new_green);
                self.root = RedNode::new_root(new_root);
            }
        }

        Ok(self)
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
