use rowan::{GreenToken, NodeOrToken};

use crate::{
    high_level::{
        ast::{AstNode, Pair},
        path::{HjsonPath, ObjectPathSegment, PathSegment},
        style::DocumentStyle,
        HjsonDocument,
    },
    HjsonEditError, RedNode, RedToken, SyntaxKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentStyle {
    #[default]
    Preserve,
    Line,
    Hash,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommentPath {
    Key { dot_path: String },
    Inline { dot_path: String },
    Header,
    Footer,
}

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

        if is_document_comment_path(&path, ":header") {
            return Ok(extract_document_comment(self, true));
        }

        if is_document_comment_path(&path, ":footer") {
            return Ok(extract_document_comment(self, false));
        }

        let pair = self.resolve_pair(&path)?;
        let comments = pair
            .leading_trivia()?
            .into_iter()
            .filter_map(|elem| comment_body_from_element(&elem))
            .collect::<Vec<_>>();

        Ok(if comments.is_empty() {
            None
        } else {
            Some(comments.into_iter().rev().collect::<Vec<_>>().join("\n"))
        })
    }

    fn set(
        mut self,
        path: impl Into<HjsonPath>,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let path = path.into();
        let comment = comment.into();

        if is_document_comment_path(&path, ":header") {
            let existing_style = extract_document_comment_style(&self, true);
            remove_document_comment(&mut self, true)?;
            return insert_document_comment(self, true, comment, existing_style);
        }

        if is_document_comment_path(&path, ":footer") {
            let existing_style = extract_document_comment_style(&self, false);
            remove_document_comment(&mut self, false)?;
            return insert_document_comment(self, false, comment, existing_style);
        }

        let existing_style = self
            .resolve_pair(&path)?
            .leading_trivia()?
            .into_iter()
            .find_map(|elem| comment_style_from_element(&elem));

        self = self.remove(path.clone())?;

        let resolved_style =
            resolve_comment_style(self.current_comment_style(), &comment, existing_style);
        let rendered = render_comment_text(resolved_style, &comment);
        let pair = self.resolve_pair(&path)?;
        let parent = pair
            .node()
            .parent()
            .ok_or_else(|| HjsonEditError::ParentNotFound)?;

        let source = self.to_string();
        let style = DocumentStyle::new(&source);
        let newline = SyntaxKind::Newline.create_token(style.newline());
        let indent = SyntaxKind::Whitespace.create_token(&style.indent_unit().repeat(path.depth()));
        let comment_token = GreenToken::new(kind_from_style(resolved_style).into(), &rendered);

        let index = pair.node().index();
        let new_green = parent.green().splice_children(
            index..index,
            vec![comment_token.into(), newline.into(), indent.into()],
        );
        let new_root = parent.replace_with(new_green);
        self.root = RedNode::new_root(new_root);

        Ok(self)
    }

    fn remove(mut self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError> {
        let path = path.into();

        if is_document_comment_path(&path, ":header") {
            remove_document_comment(&mut self, true)?;
            return Ok(self);
        }

        if is_document_comment_path(&path, ":footer") {
            remove_document_comment(&mut self, false)?;
            return Ok(self);
        }

        let pair = self.resolve_pair(&path)?;
        let parent = pair
            .node()
            .parent()
            .ok_or_else(|| HjsonEditError::ParentNotFound)?;

        let trivia = pair.leading_trivia()?;
        let start = trivia
            .iter()
            .filter(|elem| matches!(elem, NodeOrToken::Token(token) if token.kind().is_comment()))
            .map(|elem| elem.index())
            .min();

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

impl HjsonDocument {
    fn resolve_pair(&self, path: &HjsonPath) -> Result<Pair, HjsonEditError> {
        let mut target = self.get_internal(path)?;
        while target.kind() != SyntaxKind::Pair {
            target = target
                .parent()
                .ok_or_else(|| HjsonEditError::UnexpectedStructure {
                    expected: SyntaxKind::Pair,
                    found: target.kind(),
                })?;
        }

        Pair::cast(target)
    }
}

fn is_document_comment_path(path: &HjsonPath, needle: &str) -> bool {
    matches!(
        path.segments.as_slice(),
        [PathSegment::Object(ObjectPathSegment::Key(key))] if key == needle
    )
}

fn insert_document_comment(
    mut doc: HjsonDocument,
    header: bool,
    comment: String,
    existing_style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let style = resolve_comment_style(doc.current_comment_style(), &comment, existing_style);
    let rendered = render_comment_text(style, &comment);
    let source = doc.to_string();
    let doc_style = DocumentStyle::new(&source);

    let root = doc.root().clone();
    let comment_token = GreenToken::new(kind_from_style(style).into(), &rendered);
    let newline = SyntaxKind::Newline.create_token(doc_style.newline());

    let insert_at = if header {
        0
    } else {
        root.children_with_tokens().count()
    };
    let new_green = root.green().splice_children(
        insert_at..insert_at,
        vec![comment_token.into(), newline.into()],
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
                NodeOrToken::Token(token)
                    if token.kind().is_comment() || token.kind().is_trivia() =>
                {
                    end += 1;
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
                NodeOrToken::Token(token)
                    if token.kind().is_comment() || token.kind().is_trivia() =>
                {
                    start -= 1;
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
    let children: Vec<_> = doc.root().children_with_tokens().collect();

    let mut bodies = Vec::new();

    if header {
        for elem in children {
            match elem {
                NodeOrToken::Token(token) if token.kind().is_comment() => {
                    bodies.push(strip_comment_body(token.kind(), token.text()));
                }
                NodeOrToken::Token(token) if token.kind().is_trivia() => {}
                NodeOrToken::Node(_) => break,
                _ => break,
            }
        }
    } else {
        for elem in children.into_iter().rev() {
            match elem {
                NodeOrToken::Token(token) if token.kind().is_comment() => {
                    bodies.push(strip_comment_body(token.kind(), token.text()));
                }
                NodeOrToken::Token(token) if token.kind().is_trivia() => {}
                NodeOrToken::Node(_) => break,
                _ => break,
            }
        }
        bodies.reverse();
    }

    if bodies.is_empty() {
        None
    } else {
        Some(bodies.join("\n"))
    }
}

fn extract_document_comment_style(doc: &HjsonDocument, header: bool) -> Option<CommentStyle> {
    let children: Vec<_> = doc.root().children_with_tokens().collect();

    if header {
        for elem in children {
            match elem {
                NodeOrToken::Token(token) if token.kind().is_comment() => {
                    return comment_style_from_kind(token.kind());
                }
                NodeOrToken::Token(token) if token.kind().is_trivia() => {}
                NodeOrToken::Node(_) => break,
                _ => break,
            }
        }
    } else {
        for elem in children.into_iter().rev() {
            match elem {
                NodeOrToken::Token(token) if token.kind().is_comment() => {
                    return comment_style_from_kind(token.kind());
                }
                NodeOrToken::Token(token) if token.kind().is_trivia() => {}
                NodeOrToken::Node(_) => break,
                _ => break,
            }
        }
    }

    None
}

fn comment_body_from_element(elem: &NodeOrToken<RedNode, RedToken>) -> Option<String> {
    match elem {
        NodeOrToken::Token(token) if token.kind().is_comment() => {
            Some(strip_comment_body(token.kind(), token.text()))
        }
        _ => None,
    }
}

fn comment_style_from_element(elem: &NodeOrToken<RedNode, RedToken>) -> Option<CommentStyle> {
    match elem {
        NodeOrToken::Token(token) => comment_style_from_kind(token.kind()),
        NodeOrToken::Node(_) => None,
    }
}

fn comment_style_from_kind(kind: SyntaxKind) -> Option<CommentStyle> {
    match kind {
        SyntaxKind::LineComment => Some(CommentStyle::Line),
        SyntaxKind::HashComment => Some(CommentStyle::Hash),
        SyntaxKind::BlockComment => Some(CommentStyle::Block),
        _ => None,
    }
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
    let multiline = comment.contains('\n') || comment.contains('\r');

    if multiline {
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

fn strip_comment_body(kind: SyntaxKind, text: &str) -> String {
    match kind {
        SyntaxKind::LineComment => text
            .strip_prefix("//")
            .unwrap_or(text)
            .trim_start()
            .to_string(),
        SyntaxKind::HashComment => text
            .strip_prefix('#')
            .unwrap_or(text)
            .trim_start()
            .to_string(),
        SyntaxKind::BlockComment => text
            .strip_prefix("/*")
            .and_then(|s| s.strip_suffix("*/"))
            .unwrap_or(text)
            .trim()
            .to_string(),
        _ => text.to_string(),
    }
}
