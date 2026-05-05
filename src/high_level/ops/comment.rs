use rowan::NodeOrToken;

use crate::{
    ast::{AstNode, CommentKind, Member, Trivia},
    high_level::{
        error::HjsonEditError, path::HjsonCommentPath, resolve::PathResolver, HjsonDocument,
    },
    syntax::{GreenElement, RedNode, SyntaxKind},
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
    fn get_comment(&self, path: HjsonCommentPath) -> Result<Option<String>, HjsonEditError>;
    fn set_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn append_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove_comment(self, path: HjsonCommentPath) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

impl HjsonDocumentCommentEdit for HjsonDocument {
    fn get_comment(&self, path: HjsonCommentPath) -> Result<Option<String>, HjsonEditError> {
        Ok(match path {
            HjsonCommentPath::Header => extract_document_comment(self, true),
            HjsonCommentPath::Footer => extract_document_comment(self, false),
            HjsonCommentPath::Before(path) => {
                let member = PathResolver::new(self.syntax_root().clone()).resolve_member(&path)?;
                extract_comment_text(&member.leading_trivia)
            }
            HjsonCommentPath::Inline(path) => {
                let member = PathResolver::new(self.syntax_root().clone()).resolve_member(&path)?;
                extract_comment_text(&member.trailing_trivia)
            }
        })
    }

    fn set_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let comment = comment.into();
        match path {
            HjsonCommentPath::Header => set_document_comment(self, true, Some(comment)),
            HjsonCommentPath::Footer => set_document_comment(self, false, Some(comment)),
            HjsonCommentPath::Before(path) => {
                set_member_comment(self, &path, MemberCommentLocation::Before, Some(comment))
            }
            HjsonCommentPath::Inline(path) => {
                set_member_comment(self, &path, MemberCommentLocation::Inline, Some(comment))
            }
        }
    }

    fn append_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let comment = comment.into();
        let existing = self.get_comment(path.clone())?;
        let new_comment = match existing {
            Some(existing) => format!("{}\n{}", existing, comment),
            None => comment,
        };
        self.set_comment(path, new_comment)
    }

    fn remove_comment(self, path: HjsonCommentPath) -> Result<Self, HjsonEditError> {
        match path {
            HjsonCommentPath::Header => set_document_comment(self, true, None),
            HjsonCommentPath::Footer => set_document_comment(self, false, None),
            HjsonCommentPath::Before(path) => {
                set_member_comment(self, &path, MemberCommentLocation::Before, None)
            }
            HjsonCommentPath::Inline(path) => {
                set_member_comment(self, &path, MemberCommentLocation::Inline, None)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberCommentLocation {
    Before,
    Inline,
}

fn make_comment_node(style: CommentStyle, text: &str) -> rowan::GreenNode {
    SyntaxKind::Comment.create_node(vec![kind_from_style(style)
        .create_token(&render_comment_text(style, text))
        .into()])
}

fn trivia_comment_style(trivia: &[Trivia]) -> Option<CommentStyle> {
    trivia.iter().find_map(|elem| match elem {
        Trivia::Comment(comment) => Some(match comment.kind {
            CommentKind::Line => CommentStyle::Line,
            CommentKind::Hash => CommentStyle::Hash,
            CommentKind::Block => CommentStyle::Block,
        }),
        _ => None,
    })
}

fn member_comment_range(
    member: &Member,
    location: MemberCommentLocation,
) -> Option<(usize, usize)> {
    let parent_index = member.node().index();
    match location {
        MemberCommentLocation::Before => {
            let trivia = &member.leading_trivia;
            let start_rel = trivia.iter().position(|elem| matches!(elem, Trivia::Comment(_)))?;
            let base = parent_index.saturating_sub(trivia.len());
            Some((base + start_rel, parent_index))
        }
        MemberCommentLocation::Inline => {
            let trivia = &member.trailing_trivia;
            let comment_rel =
                trivia.iter().position(|elem| matches!(elem, Trivia::Comment(_)))?;
            let mut start_rel = comment_rel;
            while start_rel > 0 && matches!(trivia[start_rel - 1], Trivia::Whitespace(_)) {
                start_rel -= 1;
            }
            let start = parent_index + 1 + start_rel;
            let end = parent_index + 1 + trivia.len();
            Some((start, end))
        }
    }
}

fn document_comment_range(
    doc: &HjsonDocument,
    header: bool,
) -> Option<std::ops::Range<usize>> {
    let ast = doc.ast().ok()?;
    let root = doc.syntax_root();
    let children: Vec<_> = root.children_with_tokens().collect();

    if header {
        let leading = &ast.leading_trivia;
        let start = leading.iter().position(|elem| matches!(elem, Trivia::Comment(_)))?;
        Some(start..leading.len())
    } else {
        let trailing = &ast.trailing_trivia;
        let start = trailing.iter().position(|elem| matches!(elem, Trivia::Comment(_)))?;
        let mut end = trailing
            .iter()
            .rposition(|elem| matches!(elem, Trivia::Comment(_)))
            .unwrap()
            + 1;
        // Include a single trailing newline after the last comment as part of the block.
        if end < trailing.len() && matches!(trailing[end], Trivia::Newline { .. }) {
            end += 1;
        }
        let root_start = children.len().saturating_sub(trailing.len());
        Some((root_start + start)..(root_start + end))
    }
}

fn set_document_comment(
    mut doc: HjsonDocument,
    header: bool,
    comment: Option<String>,
) -> Result<HjsonDocument, HjsonEditError> {
    let existing_style = if header {
        doc.ast()
            .ok()
            .and_then(|ast| trivia_comment_style(&ast.leading_trivia))
    } else {
        doc.ast()
            .ok()
            .and_then(|ast| trivia_comment_style(&ast.trailing_trivia))
    };

    let root = doc.syntax_root().clone();

    let insert_at = if let Some(range) = document_comment_range(&doc, header) {
        let new_green = root.green().splice_children(range.clone(), vec![]);
        doc.root = RedNode::new_root(new_green);
        range.start
    } else if header {
        doc.ast().ok().map(|ast| ast.leading_trivia.len()).unwrap_or(0)
    } else {
        root.children_with_tokens().count()
    };

    if let Some(comment) = comment {
        let resolved_style =
            resolve_comment_style(doc.current_comment_style(), &comment, existing_style);
        let comment_node = make_comment_node(resolved_style, &comment);
        let newline = doc.current_newline().to_string();

        let root = doc.syntax_root().clone();
        let inserted = if header {
            vec![
                comment_node.into(),
                SyntaxKind::Newline.create_token(&newline).into(),
            ]
        } else {
            let needs_leading_newline = if insert_at > 0 {
                match root.children_with_tokens().nth(insert_at - 1) {
                    Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::Newline => {
                        false
                    }
                    _ => true,
                }
            } else {
                true
            };
            if needs_leading_newline {
                vec![
                    SyntaxKind::Newline.create_token(&newline).into(),
                    comment_node.into(),
                    SyntaxKind::Newline.create_token(&newline).into(),
                ]
            } else {
                vec![
                    comment_node.into(),
                    SyntaxKind::Newline.create_token(&newline).into(),
                ]
            }
        };
        let new_green = root.green().splice_children(insert_at..insert_at, inserted);
        doc.root = RedNode::new_root(new_green);
    }

    Ok(doc)
}

fn set_member_comment(
    mut doc: HjsonDocument,
    path: &crate::high_level::path::HjsonObjectPath,
    location: MemberCommentLocation,
    comment: Option<String>,
) -> Result<HjsonDocument, HjsonEditError> {
    let member = PathResolver::new(doc.syntax_root().clone()).resolve_member(path)?;
    let parent = member
        .node()
        .parent()
        .ok_or(HjsonEditError::ParentNotFound)?;

    if let Some((start, end)) = member_comment_range(&member, location) {
        let new_green = parent.green().splice_children(start..end, vec![]);
        let new_root = parent.replace_with(new_green);
        doc.root = RedNode::new_root(new_root);
    }

    if let Some(comment) = comment {
        let member = PathResolver::new(doc.syntax_root().clone()).resolve_member(path)?;
        let parent = member
            .node()
            .parent()
            .ok_or(HjsonEditError::ParentNotFound)?;
        let existing_style = trivia_comment_style(match location {
            MemberCommentLocation::Before => &member.leading_trivia,
            MemberCommentLocation::Inline => &member.trailing_trivia,
        });
        let resolved_style =
            resolve_comment_style(doc.current_comment_style(), &comment, existing_style);
        let comment_node = make_comment_node(resolved_style, &comment);

        let (index, inserted): (usize, Vec<GreenElement>) = match location {
            MemberCommentLocation::Before => {
                let newline = SyntaxKind::Newline.create_token(doc.current_newline());
                let indent =
                    SyntaxKind::Whitespace.create_token(&detect_indent_for_member(&member));
                (
                    member.node().index(),
                    vec![comment_node.into(), newline.into(), indent.into()],
                )
            }
            MemberCommentLocation::Inline => {
                let space = SyntaxKind::Whitespace.create_token(" ");
                (
                    member.node().index() + 1,
                    vec![space.into(), comment_node.into()],
                )
            }
        };

        let new_green = parent.green().splice_children(index..index, inserted);
        let new_root = parent.replace_with(new_green);
        doc.root = RedNode::new_root(new_root);
    }

    Ok(doc)
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

fn extract_comment_text(trivia: &[Trivia]) -> Option<String> {
    let comments = trivia
        .iter()
        .filter_map(|elem| match elem {
            Trivia::Comment(comment) => Some(comment.body.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if comments.is_empty() {
        None
    } else {
        Some(comments.into_iter().rev().collect::<Vec<_>>().join("\n"))
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

fn detect_indent_for_member(member: &Member) -> String {
    let mut indent = String::new();
    for elem in member.leading_trivia.iter().rev() {
        match elem {
            Trivia::Whitespace(text) => {
                if indent.is_empty() {
                    indent = text.clone();
                }
            }
            Trivia::Newline { .. } => break,
            _ => {}
        }
    }

    if indent.is_empty() {
        "  ".repeat(member.layout.depth)
    } else {
        indent
    }
}

fn render_comment_text(style: CommentStyle, comment: &str) -> String {
    match style {
        CommentStyle::Line => format!("// {}", comment),
        CommentStyle::Hash => format!("# {}", comment),
        CommentStyle::Block | CommentStyle::Preserve => format!("/* {} */", comment),
    }
}
