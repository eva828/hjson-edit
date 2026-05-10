use rowan::{GreenNode, NodeOrToken};

use crate::{
    ast::{AstNode, CommentKind, Document, Member, Trivia},
    high_level::{
        error::HjsonEditError, path::HjsonCommentPath, resolve::PathResolver, Prefer,
        HjsonDocument,
    },
    syntax::{GreenElement, RedNode, SyntaxKind},
};

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
        match resolve_comment_target(&path) {
            CommentTarget::Document { header } => {
                let ast = self.ast().ok();
                let trivia = if header {
                    ast.as_ref().map(|a| &*a.leading_trivia)
                } else {
                    ast.as_ref().map(|a| &*a.trailing_trivia)
                };
                Ok(trivia.and_then(join_comment_bodies))
            }
            CommentTarget::Member { path, location } => {
                let member =
                    PathResolver::new(self.syntax_root().clone()).resolve_member(path)?;
                let trivia = match location {
                    MemberCommentLocation::Before => &member.leading_trivia,
                    MemberCommentLocation::Inline => &member.trailing_trivia,
                };
                Ok(join_comment_bodies(trivia))
            }
        }
    }

    fn set_comment(
        self,
        path: HjsonCommentPath,
        comment: impl Into<String>,
    ) -> Result<Self, HjsonEditError> {
        let updated = update_comment(&self, path, Some(comment.into()))?;
        Ok(self.root(updated))
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
        let updated = update_comment(&self, path, None)?;
        Ok(self.root(updated))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberCommentLocation {
    Before,
    Inline,
}

enum CommentTarget<'a> {
    Document { header: bool },
    Member {
        path: &'a crate::high_level::path::HjsonObjectPath,
        location: MemberCommentLocation,
    },
}

fn resolve_comment_target(path: &HjsonCommentPath) -> CommentTarget<'_> {
    match path {
        HjsonCommentPath::Header => CommentTarget::Document { header: true },
        HjsonCommentPath::Footer => CommentTarget::Document { header: false },
        HjsonCommentPath::Before(path) => CommentTarget::Member {
            path,
            location: MemberCommentLocation::Before,
        },
        HjsonCommentPath::Inline(path) => CommentTarget::Member {
            path,
            location: MemberCommentLocation::Inline,
        },
    }
}

fn extract_comment_bodies(trivia: &[Trivia]) -> Vec<String> {
    trivia
        .iter()
        .filter_map(|elem| match elem {
            Trivia::Comment(comment) => Some(comment.body.clone()),
            _ => None,
        })
        .collect()
}

fn join_comment_bodies(trivia: &[Trivia]) -> Option<String> {
    let bodies = extract_comment_bodies(trivia);
    if bodies.is_empty() {
        None
    } else {
        Some(bodies.join("\n"))
    }
}

fn make_comment_node(style: CommentKind, text: &str) -> rowan::GreenNode {
    SyntaxKind::Comment.create_node(vec![kind_from_style(style)
        .create_token(render_comment_text(style, text))
        .into()])
}

fn trivia_comment_style(trivia: &[Trivia]) -> Option<CommentKind> {
    trivia.iter().find_map(|elem| match elem {
        Trivia::Comment(comment) => Some(comment.kind),
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

fn update_comment(
    doc: &HjsonDocument,
    path: HjsonCommentPath,
    comment: Option<String>,
) -> Result<GreenNode, HjsonEditError> {
    let root = doc.syntax_root().clone();
    let preferred = doc.current_comment_style();
    let newline = doc.current_newline();

    match resolve_comment_target(&path) {
        CommentTarget::Document { header } => {
            build_document_comment(&root, header, comment, preferred, newline)
        }
        CommentTarget::Member { path, location } => {
            build_member_comment(root, path, location, comment, preferred, newline)
        }
    }
}

fn build_document_comment(
    root: &RedNode,
    header: bool,
    comment: Option<String>,
    preferred: Prefer<CommentKind>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let ast = Document::cast(root.clone())?;

    let existing_style = if header {
        trivia_comment_style(&ast.leading_trivia)
    } else {
        trivia_comment_style(&ast.trailing_trivia)
    };

    let children: Vec<_> = root.children_with_tokens().collect();

    let (mut green, insert_at) = if header {
        let leading = &ast.leading_trivia;
        if let Some(start) = leading.iter().position(|e| matches!(e, Trivia::Comment(_))) {
            let range = start..leading.len();
            let green = root.green().splice_children(range.clone(), vec![]);
            (green, range.start)
        } else {
            (root.green().clone().into_owned(), leading.len())
        }
    } else {
        let trailing = &ast.trailing_trivia;
        if let Some(start) = trailing.iter().position(|e| matches!(e, Trivia::Comment(_))) {
            let mut end = trailing
                .iter()
                .rposition(|e| matches!(e, Trivia::Comment(_)))
                .unwrap()
                + 1;
            if end < trailing.len() && matches!(trailing[end], Trivia::Newline { .. }) {
                end += 1;
            }
            let root_start = children.len().saturating_sub(trailing.len());
            let range = (root_start + start)..(root_start + end);
            let green = root.green().splice_children(range.clone(), vec![]);
            (green, range.start)
        } else {
            (root.green().clone().into_owned(), children.len())
        }
    };

    if let Some(comment) = comment {
        let resolved_style = resolve_comment_style(preferred, &comment, existing_style);
        let comment_node = make_comment_node(resolved_style, &comment);

        let inserted: Vec<GreenElement> = if header {
            vec![
                comment_node.into(),
                SyntaxKind::Newline.create_token(newline).into(),
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
                    SyntaxKind::Newline.create_token(newline).into(),
                    comment_node.into(),
                    SyntaxKind::Newline.create_token(newline).into(),
                ]
            } else {
                vec![
                    comment_node.into(),
                    SyntaxKind::Newline.create_token(newline).into(),
                ]
            }
        };
        green = green.splice_children(insert_at..insert_at, inserted);
    }

    Ok(green)
}

fn build_member_comment(
    root: RedNode,
    path: &crate::high_level::path::HjsonObjectPath,
    location: MemberCommentLocation,
    comment: Option<String>,
    preferred: Prefer<CommentKind>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let member = PathResolver::new(root.clone()).resolve_member(path)?;
    let parent = member
        .node()
        .parent()
        .ok_or(HjsonEditError::ParentNotFound)?;

    let green_after_remove = if let Some((start, end)) = member_comment_range(&member, location) {
        parent.green().splice_children(start..end, vec![])
    } else {
        parent.green().clone().into_owned()
    };

    if comment.is_none() {
        return if parent.kind() == SyntaxKind::Root {
            Ok(green_after_remove)
        } else {
            Ok(parent.replace_with(green_after_remove))
        };
    }

    let comment = comment.unwrap();

    let new_root = RedNode::new_root(if parent.kind() == SyntaxKind::Root {
        green_after_remove.clone()
    } else {
        parent.replace_with(green_after_remove.clone())
    });

    let member = PathResolver::new(new_root).resolve_member(path)?;
    let parent = member
        .node()
        .parent()
        .ok_or(HjsonEditError::ParentNotFound)?;
    let existing_style = trivia_comment_style(match location {
        MemberCommentLocation::Before => &member.leading_trivia,
        MemberCommentLocation::Inline => &member.trailing_trivia,
    });
    let resolved_style = resolve_comment_style(preferred, &comment, existing_style);
    let comment_node = make_comment_node(resolved_style, &comment);

    let (index, inserted): (usize, Vec<GreenElement>) = match location {
        MemberCommentLocation::Before => {
            let nl = SyntaxKind::Newline.create_token(newline);
            let indent = SyntaxKind::Whitespace.create_token(detect_indent_for_member(&member));
            (
                member.node().index(),
                vec![comment_node.into(), nl.into(), indent.into()],
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

    let new_parent = parent.green().splice_children(index..index, inserted);
    if parent.kind() == SyntaxKind::Root {
        Ok(new_parent)
    } else {
        Ok(parent.replace_with(new_parent))
    }
}

fn kind_from_style(style: CommentKind) -> SyntaxKind {
    match style {
        CommentKind::Block => SyntaxKind::BlockComment,
        CommentKind::Line => SyntaxKind::LineComment,
        CommentKind::Hash => SyntaxKind::HashComment,
    }
}

fn resolve_comment_style(
    preferred: Prefer<CommentKind>,
    comment: &str,
    existing_style: Option<CommentKind>,
) -> CommentKind {
    if comment.contains('\n') || comment.contains('\r') {
        return CommentKind::Block;
    }

    match preferred {
        Prefer::Preserve => existing_style.unwrap_or(CommentKind::Block),
        Prefer::Explicit(kind) => kind,
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

fn render_comment_text(style: CommentKind, comment: &str) -> String {
    match style {
        CommentKind::Line => format!("// {}", comment),
        CommentKind::Hash => format!("# {}", comment),
        CommentKind::Block => format!("/* {} */", comment),
    }
}
