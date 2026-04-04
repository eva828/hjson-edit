use rowan::NodeOrToken;

use crate::{
    ast::{AstNode, CommentKind, Trivia},
    high_level::{
        error::HjsonEditError,
        ext::RedNodeExt,
        path::{HjsonCommentPath, HjsonObjectPath},
        style::DetectNewline,
        HjsonDocument,
    },
    syntax::{RedNode, SyntaxKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentStyle {
    #[default]
    Preserve,
    Line,
    Hash,
    Block,
}

pub trait HjsonDocumentCommentEdit {
    fn get(&self, path: HjsonCommentPath) -> Result<Option<String>, HjsonEditError>;
    fn set(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: HjsonCommentPath) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get(&self, path: HjsonCommentPath) -> Result<Option<String>, HjsonEditError> {
        Ok(match path {
            HjsonCommentPath::Header => extract_document_comment(self, true),
            HjsonCommentPath::Footer => extract_document_comment(self, false),
            HjsonCommentPath::Before(path) | HjsonCommentPath::Inline(path) => {
                let pair = self.resolve_pair(&path)?;
                let comments = pair
                    .leading_trivia
                    .into_iter()
                    .filter_map(|elem| match elem {
                        Trivia::Comment(comment) => Some(comment.body),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                if comments.is_empty() {
                    None
                } else {
                    Some(comments.into_iter().rev().collect::<Vec<_>>().join("\n"))
                }
            }
        })
    }

    fn set(
        mut self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let comment = comment.into();

        match path {
            HjsonCommentPath::Header => {
                let existing_style = extract_document_comment_style(&self, true);
                remove_document_comment(&mut self, true)?;
                insert_document_comment(self, true, comment, existing_style)
            }
            HjsonCommentPath::Footer => {
                let existing_style = extract_document_comment_style(&self, false);
                remove_document_comment(&mut self, false)?;
                insert_document_comment(self, false, comment, existing_style)
            }
            HjsonCommentPath::Before(path) | HjsonCommentPath::Inline(path) => {
                let existing_style = self
                    .resolve_pair(&path)?
                    .leading_trivia
                    .into_iter()
                    .find_map(|elem| match elem {
                        Trivia::Comment(comment) => Some(match comment.kind {
                            CommentKind::Line => CommentStyle::Line,
                            CommentKind::Hash => CommentStyle::Hash,
                            CommentKind::Block => CommentStyle::Block,
                        }),
                        _ => None,
                    });

                self = self.remove_comment_by_path(HjsonCommentPath::Before(path.clone()))?;

                let resolved_style =
                    resolve_comment_style(self.current_comment_style(), &comment, existing_style);
                let rendered = render_comment_text(resolved_style, &comment);
                let pair = self.resolve_pair(&path)?;
                let parent = pair.node().parent().ok_or(HjsonEditError::ParentNotFound)?;

                let newline =
                    SyntaxKind::Newline.create_token(self.root().detect_newline().as_str());
                let indent = SyntaxKind::Whitespace.create_token(&"  ".repeat(path.depth()));
                let comment_node =
                    SyntaxKind::Comment.create_node(vec![kind_from_style(resolved_style)
                        .create_token(&rendered)
                        .into()]);

                let index = pair.node().index();
                let new_green = parent.green().splice_children(
                    index..index,
                    vec![comment_node.into(), newline.into(), indent.into()],
                );
                let new_root = parent.replace_with(new_green);
                self.root = RedNode::new_root(new_root);
                Ok(self)
            }
        }
    }

    fn remove(self, path: HjsonCommentPath) -> Result<Self, HjsonEditError> {
        self.remove_comment_by_path(path)
    }
}

impl HjsonDocument {
    fn resolve_pair(&self, path: &HjsonObjectPath) -> Result<crate::ast::Pair, HjsonEditError> {
        let mut target = self.root().clone().document_start_node();
        for segment in path.segments() {
            target = target
                .child(segment)
                .ok_or_else(|| HjsonEditError::PathNotFound(path.clone()))?;
        }

        while target.kind() != SyntaxKind::Pair {
            target = target.parent().ok_or(HjsonEditError::ParentNotFound)?;
        }

        crate::ast::Pair::cast(target)
    }

    fn remove_comment_by_path(mut self, path: HjsonCommentPath) -> Result<Self, HjsonEditError> {
        match path {
            HjsonCommentPath::Header => {
                remove_document_comment(&mut self, true)?;
                Ok(self)
            }
            HjsonCommentPath::Footer => {
                remove_document_comment(&mut self, false)?;
                Ok(self)
            }
            HjsonCommentPath::Before(path) | HjsonCommentPath::Inline(path) => {
                let pair = self.resolve_pair(&path)?;
                let parent = pair.node().parent().ok_or(HjsonEditError::ParentNotFound)?;
                let start = pair
                    .leading_trivia
                    .iter()
                    .position(|elem| matches!(elem, Trivia::Comment(_)));

                let Some(start) = start else {
                    return Ok(self);
                };

                let new_green = parent
                    .green()
                    .splice_children(start..pair.node().index(), vec![]);
                let new_root = parent.replace_with(new_green);
                self.root = RedNode::new_root(new_root);
                Ok(self)
            }
        }
    }
}

fn insert_document_comment(
    mut doc: HjsonDocument,
    header: bool,
    comment: String,
    existing_style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let style = resolve_comment_style(doc.current_comment_style(), &comment, existing_style);
    let rendered = render_comment_text(style, &comment);
    let newline = doc.root().detect_newline().as_str().to_string();

    let root = doc.root().clone();
    let comment_node = SyntaxKind::Comment
        .create_node(vec![kind_from_style(style).create_token(&rendered).into()]);
    let newline = SyntaxKind::Newline.create_token(&newline);

    let insert_at = if header {
        0
    } else {
        root.children_with_tokens().count()
    };
    let new_green = root.green().splice_children(
        insert_at..insert_at,
        vec![comment_node.into(), newline.into()],
    );
    doc.root = RedNode::new_root(new_green);
    Ok(doc)
}

fn remove_document_comment(doc: &mut HjsonDocument, header: bool) -> Result<(), HjsonEditError> {
    let root = doc.root().clone();
    let children: Vec<_> = root.children_with_tokens().collect();

    let range = if header {
        let mut end = 0usize;
        while end < children.len() {
            match &children[end] {
                NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => end += 1,
                NodeOrToken::Token(token)
                    if token.kind().is_comment() || token.kind().is_trivia() =>
                {
                    end += 1
                }
                NodeOrToken::Node(_) => break,
                _ => break,
            }
        }
        0..end
    } else {
        let mut start = children.len();
        while start > 0 {
            match &children[start - 1] {
                NodeOrToken::Node(node) if node.kind() == SyntaxKind::Comment => start -= 1,
                NodeOrToken::Token(token)
                    if token.kind().is_comment() || token.kind().is_trivia() =>
                {
                    start -= 1
                }
                NodeOrToken::Node(_) => break,
                _ => break,
            }
        }
        start..children.len()
    };

    if range.is_empty() {
        return Ok(());
    }

    let new_green = root.green().splice_children(range, vec![]);
    doc.root = RedNode::new_root(new_green);
    Ok(())
}

fn extract_document_comment(doc: &HjsonDocument, header: bool) -> Option<String> {
    let ast = doc.ast().ok()?;
    let mut bodies = Vec::new();

    if header {
        for elem in &ast.leading_trivia {
            if let Trivia::Comment(comment) = elem {
                bodies.push(comment.body.clone())
            }
        }
    } else {
        for elem in &ast.trailing_trivia {
            if let Trivia::Comment(comment) = elem {
                bodies.push(comment.body.clone())
            }
        }
    }

    if bodies.is_empty() {
        None
    } else {
        Some(bodies.join("\n"))
    }
}

fn extract_document_comment_style(doc: &HjsonDocument, header: bool) -> Option<CommentStyle> {
    let ast = doc.ast().ok()?;

    if header {
        for elem in &ast.leading_trivia {
            if let Trivia::Comment(comment) = elem {
                return Some(match comment.kind {
                    CommentKind::Line => CommentStyle::Line,
                    CommentKind::Hash => CommentStyle::Hash,
                    CommentKind::Block => CommentStyle::Block,
                });
            }
        }
    } else {
        for elem in &ast.trailing_trivia {
            if let Trivia::Comment(comment) = elem {
                return Some(match comment.kind {
                    CommentKind::Line => CommentStyle::Line,
                    CommentKind::Hash => CommentStyle::Hash,
                    CommentKind::Block => CommentStyle::Block,
                });
            }
        }
    }

    None
}

fn kind_from_style(style: CommentStyle) -> SyntaxKind {
    match style {
        CommentStyle::Preserve | CommentStyle::Block => SyntaxKind::BlockComment,
        CommentStyle::Line => SyntaxKind::LineComment,
        CommentStyle::Hash => SyntaxKind::HashComment,
    }
}

fn resolve_comment_style(
    preferred: CommentStyle,
    comment: &str,
    existing_style: Option<CommentStyle>,
) -> CommentStyle {
    if comment.contains('\n') || comment.contains('\r') {
        return CommentStyle::Block;
    }

    match preferred {
        CommentStyle::Preserve => existing_style.unwrap_or(CommentStyle::Block),
        CommentStyle::Line => CommentStyle::Line,
        CommentStyle::Hash => CommentStyle::Hash,
        CommentStyle::Block => CommentStyle::Block,
    }
}

fn render_comment_text(style: CommentStyle, comment: &str) -> String {
    match style {
        CommentStyle::Line => format!("// {}", comment),
        CommentStyle::Hash => format!("# {}", comment),
        CommentStyle::Block | CommentStyle::Preserve => format!("/* {} */", comment),
    }
}
