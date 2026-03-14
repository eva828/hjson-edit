use crate::document_style::{DetectNewline as _, NewlineKind};
use crate::edit::high_level::navigation::RedNodeNavigateExt;
use crate::edit::high_level::{
    clone_element, contextual_newline_kind, CommentPath, CommentStyle, HjsonDocument,
    HjsonEditError, PathSegment,
};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::Language;
use rowan::{GreenNode, NodeOrToken};

pub(crate) struct CommentEditor;

struct CommentTargetResolver<'a> {
    document: &'a HjsonDocument,
    dot_path: &'a str,
    path_segments: Vec<PathSegment>,
}

struct PairCommentEditor;

struct InlineCommentEditor;

struct RootCommentEditor;

struct PreparedPair {
    green: GreenNode,
    prepended_indent_len: usize,
}

impl CommentEditor {
    fn set(
        document: &HjsonDocument,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<HjsonDocument, HjsonEditError> {
        match path {
            CommentPath::Key { dot_path } => set_key_comment(document, dot_path, text, style),
            CommentPath::Inline { dot_path } => {
                set_inline_comment_for_path(document, dot_path, text, style)
            }
            CommentPath::Header => set_header_comment(document, text, style),
            CommentPath::Footer => set_footer_comment(document, text, style),
        }
    }

    fn append(
        document: &HjsonDocument,
        path: &CommentPath,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<HjsonDocument, HjsonEditError> {
        let current_comments = match path {
            CommentPath::Key { dot_path } => PairCommentEditor::get_for_path(document, dot_path)?,
            CommentPath::Inline { dot_path } => {
                InlineCommentEditor::get_for_path(document, dot_path)?
            }
            CommentPath::Header => RootCommentEditor::get_header(document),
            CommentPath::Footer => RootCommentEditor::get_footer(document),
        };

        if current_comments.is_empty() {
            return Self::set(document, path, text, style);
        }

        let append_style = style
            .unwrap_or_else(|| infer_comment_style_from_text(current_comments.last().unwrap()));
        if matches!(append_style, CommentStyle::Line | CommentStyle::Hash) && text.contains('\n') {
            return Self::set(document, path, text, Some(CommentStyle::Block));
        }

        let newline = comment_path_newline_kind(document, path)?.as_str();

        let existing = current_comments
            .iter()
            .map(|comment| strip_comment_prefix(comment))
            .collect::<Vec<_>>()
            .join(newline);
        let new_text = format!("{}{}{}", existing, newline, text);
        Self::set(document, path, &new_text, Some(append_style))
    }

    fn remove(
        document: &HjsonDocument,
        path: &CommentPath,
    ) -> Result<HjsonDocument, HjsonEditError> {
        match path {
            CommentPath::Key { dot_path } => remove_key_comment(document, dot_path),
            CommentPath::Inline { dot_path } => remove_inline_comment_for_path(document, dot_path),
            CommentPath::Header => remove_header_comment(document),
            CommentPath::Footer => remove_footer_comment(document),
        }
    }
}

impl<'a> CommentTargetResolver<'a> {
    fn new(document: &'a HjsonDocument, dot_path: &'a str) -> Result<Self, HjsonEditError> {
        let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
        if path_segments.is_empty() {
            return Err(HjsonEditError::InvalidPath {
                path: dot_path.to_string(),
            });
        }

        Ok(Self {
            document,
            dot_path,
            path_segments,
        })
    }

    fn key(&self) -> Result<&str, HjsonEditError> {
        expect_terminal_key(&self.path_segments, self.dot_path)
    }

    fn parent_path(&self) -> &[PathSegment] {
        &self.path_segments[..self.path_segments.len() - 1]
    }

    fn object(&self) -> Result<RedNode, HjsonEditError> {
        resolve_object_for_comment_path(self.document, self.parent_path(), self.dot_path)
    }

    fn pair_node(&self) -> Result<RedNode, HjsonEditError> {
        let key = self.key()?;
        self.object()?
            .find_pair_node(key)
            .ok_or_else(|| HjsonEditError::InvalidPath {
                path: self.dot_path.to_string(),
            })
    }

    fn pair_index(&self) -> Result<usize, HjsonEditError> {
        let key = self.key()?;
        self.object()?
            .find_pair_index(key)
            .ok_or_else(|| HjsonEditError::InvalidPath {
                path: self.dot_path.to_string(),
            })
    }
}

impl PairCommentEditor {
    fn set_for_path(
        document: &HjsonDocument,
        dot_path: &str,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<HjsonDocument, HjsonEditError> {
        let resolver = CommentTargetResolver::new(document, dot_path)?;
        let obj = resolver.object()?;
        let pair_node = resolver.pair_node()?;
        let prepared_pair = Self::prepare_pair(&obj, &pair_node);
        let newline = pair_comment_newline_kind(document, &obj, &pair_node).as_str();

        let comment_style = determine_comment_style(&prepared_pair.green, style)?;
        let new_comment = create_comment_token(text, comment_style, newline);
        let mut updated_pair =
            insert_comment_before_pair(&prepared_pair.green, new_comment, newline)?;

        if prepared_pair.prepended_indent_len > 0 {
            updated_pair =
                Self::strip_prepended_indent(&updated_pair, prepared_pair.prepended_indent_len);
        }

        document.replace_green_node_internal(&pair_node.path_from_root(), updated_pair)
    }

    fn remove_for_path(
        document: &HjsonDocument,
        dot_path: &str,
    ) -> Result<HjsonDocument, HjsonEditError> {
        let resolver = CommentTargetResolver::new(document, dot_path)?;
        let pair_node = resolver.pair_node()?;
        let new_pair = remove_comments_from_pair(&pair_node.to_green())?;
        document.replace_green_node_internal(&pair_node.path_from_root(), new_pair)
    }

    fn get_for_path(
        document: &HjsonDocument,
        dot_path: &str,
    ) -> Result<Vec<String>, HjsonEditError> {
        let resolver = CommentTargetResolver::new(document, dot_path)?;
        let pair_node = resolver.pair_node()?;

        let mut comments = Vec::new();
        for elem in pair_node.children_with_tokens() {
            if let rowan::NodeOrToken::Token(token) = elem {
                let kind = token.kind();
                if matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                ) {
                    comments.push(token.text().to_string());
                } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                    break;
                }
            }
        }
        Ok(comments)
    }

    fn prepare_pair(obj: &RedNode, pair_node: &RedNode) -> PreparedPair {
        let indentation = Self::object_indentation(obj, pair_node);
        let mut pair_green = pair_node.to_green();

        if !indentation.is_empty() && !Self::starts_with_whitespace(&pair_green) {
            let mut new_pair_children = indentation.clone();
            new_pair_children.extend(pair_green.children().map(clone_element));
            pair_green = SyntaxKind::Pair.create_node(new_pair_children);
        }

        PreparedPair {
            green: pair_green,
            prepended_indent_len: indentation.len(),
        }
    }

    fn object_indentation(obj: &RedNode, pair_node: &RedNode) -> Vec<GreenElement> {
        let obj_green = obj.to_green();
        let mut pair_idx_in_obj = 0;
        for (i, child) in obj_green.children().enumerate() {
            if let NodeOrToken::Node(n) = child {
                if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair
                    && *n == *pair_node.green()
                {
                    pair_idx_in_obj = i;
                    break;
                }
            }
        }

        if pair_idx_in_obj == 0 {
            return Vec::new();
        }

        let mut idx = pair_idx_in_obj - 1;
        let mut temp_indent = Vec::new();
        loop {
            if let Some(NodeOrToken::Token(t)) = obj_green.children().nth(idx) {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                if kind == SyntaxKind::Whitespace {
                    temp_indent.push(NodeOrToken::Token(kind.create_token(t.text())));
                } else if kind == SyntaxKind::Newline {
                    return temp_indent.into_iter().rev().collect();
                } else {
                    return Vec::new();
                }
            } else {
                return Vec::new();
            }

            if idx == 0 {
                return Vec::new();
            }
            idx -= 1;
        }
    }

    fn starts_with_whitespace(pair_green: &GreenNode) -> bool {
        matches!(
            pair_green.children().next(),
            Some(NodeOrToken::Token(t))
                if crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()) == SyntaxKind::Whitespace
        )
    }

    fn strip_prepended_indent(pair_green: &GreenNode, indent_len: usize) -> GreenNode {
        let mut final_children = Vec::new();
        let mut skip_count = indent_len;
        for child in pair_green.children() {
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }
            final_children.push(clone_element(child));
        }
        SyntaxKind::Pair.create_node(final_children)
    }
}

impl InlineCommentEditor {
    fn set_for_path(
        document: &HjsonDocument,
        dot_path: &str,
        text: &str,
        style: Option<CommentStyle>,
    ) -> Result<HjsonDocument, HjsonEditError> {
        let resolver = CommentTargetResolver::new(document, dot_path)?;
        let obj = resolver.object()?;
        let pair_idx = resolver.pair_index()?;
        let newline = contextual_newline_kind(document, &obj).as_str();
        let new_obj = set_inline_comment(&obj.to_green(), pair_idx, text, style, newline)?;
        document.replace_green_node_internal(&obj.path_from_root(), new_obj)
    }

    fn remove_for_path(
        document: &HjsonDocument,
        dot_path: &str,
    ) -> Result<HjsonDocument, HjsonEditError> {
        let resolver = CommentTargetResolver::new(document, dot_path)?;
        let obj = resolver.object()?;
        let pair_idx = resolver.pair_index()?;
        let new_obj = remove_inline_comment(&obj.to_green(), pair_idx)?;
        document.replace_green_node_internal(&obj.path_from_root(), new_obj)
    }

    fn get_for_path(
        document: &HjsonDocument,
        dot_path: &str,
    ) -> Result<Vec<String>, HjsonEditError> {
        let resolver = CommentTargetResolver::new(document, dot_path)?;
        let obj = resolver.object()?;
        let pair_idx = resolver.pair_index()?;
        Ok(extract_inline_comments(&obj.to_green(), pair_idx))
    }
}

impl RootCommentEditor {
    fn get_header(document: &HjsonDocument) -> Vec<String> {
        let red = RedNode::new_root(document.green_node().clone());
        let mut comments = Vec::new();

        for elem in red.children_with_tokens() {
            if let Some(token) = elem.as_token() {
                let kind = token.kind();
                if matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                ) {
                    comments.push(token.text().to_string());
                }
            } else {
                break;
            }
        }

        comments
    }

    fn get_footer(document: &HjsonDocument) -> Vec<String> {
        let red = RedNode::new_root(document.green_node().clone());
        let mut comments = Vec::new();
        let children: Vec<_> = red.children_with_tokens().collect();

        for elem in children.iter().rev() {
            if let Some(token) = elem.as_token() {
                let kind = token.kind();
                if matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                ) {
                    comments.push(token.text().to_string());
                } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                    break;
                }
            } else {
                break;
            }
        }

        comments.reverse();
        comments
    }
}

pub(crate) fn get_header_comment(document: &HjsonDocument) -> Vec<String> {
    RootCommentEditor::get_header(document)
}

pub(crate) fn get_footer_comment(document: &HjsonDocument) -> Vec<String> {
    RootCommentEditor::get_footer(document)
}

pub(crate) fn comment_set(
    document: &HjsonDocument,
    path: &CommentPath,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    CommentEditor::set(document, path, text, style)
}

pub(crate) fn comment_append(
    document: &HjsonDocument,
    path: &CommentPath,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    CommentEditor::append(document, path, text, style)
}

pub(crate) fn comment_remove(
    document: &HjsonDocument,
    path: &CommentPath,
) -> Result<HjsonDocument, HjsonEditError> {
    CommentEditor::remove(document, path)
}

fn set_key_comment(
    document: &HjsonDocument,
    dot_path: &str,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    PairCommentEditor::set_for_path(document, dot_path, text, style)
}

fn set_inline_comment_for_path(
    document: &HjsonDocument,
    dot_path: &str,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    InlineCommentEditor::set_for_path(document, dot_path, text, style)
}

fn set_header_comment(
    document: &HjsonDocument,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let style = style.unwrap_or(CommentStyle::Line);
    let newline = header_comment_newline_kind(document).as_str();
    let children: Vec<GreenElement> = RedNode::new_root(document.green_node().clone())
        .children_with_tokens()
        .map(clone_red_or_token)
        .collect();
    let split_idx = children
        .iter()
        .position(|child| !is_leading_comment_trivia(child))
        .unwrap_or(children.len());

    let mut new_children: Vec<GreenElement> = Vec::new();
    append_comment_tokens(&mut new_children, text, style, newline);
    if split_idx < children.len() {
        new_children.push(NodeOrToken::Token(
            SyntaxKind::Newline.create_token(newline),
        ));
        new_children.extend(children.into_iter().skip(split_idx));
    }

    Ok(HjsonDocument::from_green(
        SyntaxKind::Root.create_node(new_children),
    ))
}

fn set_footer_comment(
    document: &HjsonDocument,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let style = style.unwrap_or(CommentStyle::Line);
    let newline = footer_comment_newline_kind(document).as_str();
    let mut children: Vec<GreenElement> = RedNode::new_root(document.green_node().clone())
        .children_with_tokens()
        .map(clone_red_or_token)
        .collect();
    let mut footer_start = children.len();

    while footer_start > 0 {
        let elem = &children[footer_start - 1];
        match elem {
            NodeOrToken::Token(t) => {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                if matches!(
                    kind,
                    SyntaxKind::Whitespace
                        | SyntaxKind::Newline
                        | SyntaxKind::LineComment
                        | SyntaxKind::HashComment
                        | SyntaxKind::BlockComment
                ) {
                    footer_start -= 1;
                    continue;
                }
            }
            NodeOrToken::Node(_) => {}
        }
        break;
    }

    children.truncate(footer_start);

    if !children.is_empty()
        && !matches!(
            children.last(),
            Some(NodeOrToken::Token(t))
                if crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()) == SyntaxKind::Newline
        )
    {
        children.push(NodeOrToken::Token(
            SyntaxKind::Newline.create_token(newline),
        ));
    }
    append_comment_tokens(&mut children, text, style, newline);

    Ok(HjsonDocument::from_green(
        SyntaxKind::Root.create_node(children),
    ))
}

fn remove_key_comment(
    document: &HjsonDocument,
    dot_path: &str,
) -> Result<HjsonDocument, HjsonEditError> {
    PairCommentEditor::remove_for_path(document, dot_path)
}

fn remove_inline_comment_for_path(
    document: &HjsonDocument,
    dot_path: &str,
) -> Result<HjsonDocument, HjsonEditError> {
    InlineCommentEditor::remove_for_path(document, dot_path)
}

fn remove_header_comment(document: &HjsonDocument) -> Result<HjsonDocument, HjsonEditError> {
    let red = RedNode::new_root(document.green_node().clone());
    let mut new_children: Vec<GreenElement> = Vec::new();
    let mut skip = true;

    for elem in red.children_with_tokens() {
        match elem {
            NodeOrToken::Node(n) => {
                let node: GreenNode = n.green().clone().into_owned();
                new_children.push(NodeOrToken::Node(node));
                skip = false;
            }
            NodeOrToken::Token(t) => {
                let kind = t.kind();
                if skip {
                    if matches!(
                        kind,
                        SyntaxKind::LineComment
                            | SyntaxKind::HashComment
                            | SyntaxKind::BlockComment
                    ) {
                        continue;
                    } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                        skip = false;
                    }
                }
                new_children.push(NodeOrToken::Token(rowan::GreenToken::new(
                    kind.into(),
                    t.text(),
                )));
            }
        }
    }

    Ok(HjsonDocument::from_green(
        SyntaxKind::Root.create_node(new_children),
    ))
}

fn remove_footer_comment(document: &HjsonDocument) -> Result<HjsonDocument, HjsonEditError> {
    let red = RedNode::new_root(document.green_node().clone());
    let mut new_children: Vec<GreenElement> =
        red.children_with_tokens().map(clone_red_or_token).collect();

    let mut i = new_children.len();
    while i > 0 {
        i -= 1;
        if let NodeOrToken::Token(t) = &new_children[i] {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if matches!(
                kind,
                SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
            ) {
                new_children.remove(i);
            } else if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                break;
            }
        } else {
            break;
        }
    }

    Ok(HjsonDocument::from_green(
        SyntaxKind::Root.create_node(new_children),
    ))
}

fn resolve_object_for_comment_path(
    document: &HjsonDocument,
    path: &[PathSegment],
    raw: &str,
) -> Result<RedNode, HjsonEditError> {
    if path.is_empty() {
        let red = RedNode::new_root(document.green_node().clone());
        red.resolve_object_child()
            .ok_or(HjsonEditError::InvalidNodeKind {
                expected: SyntaxKind::Object,
                found: SyntaxKind::Root,
            })
    } else {
        document
            .navigate_to_node_internal(path)
            .and_then(|parent| parent.resolve_object_child())
            .ok_or(HjsonEditError::InvalidPath {
                path: raw.to_string(),
            })
    }
}

fn expect_terminal_key<'a>(path: &'a [PathSegment], raw: &str) -> Result<&'a str, HjsonEditError> {
    match path.last() {
        Some(PathSegment::Key(key)) => Ok(key),
        _ => Err(HjsonEditError::InvalidPath {
            path: raw.to_string(),
        }),
    }
}

fn clone_red_or_token(
    elem: rowan::NodeOrToken<RedNode, crate::syntax::red::RedToken>,
) -> GreenElement {
    match elem {
        NodeOrToken::Node(n) => NodeOrToken::Node(n.green().clone().into_owned()),
        NodeOrToken::Token(t) => {
            NodeOrToken::Token(rowan::GreenToken::new(t.kind().into(), t.text()))
        }
    }
}

fn strip_comment_prefix(comment: &str) -> String {
    if let Some(rest) = comment.strip_prefix("// ") {
        rest.to_string()
    } else if let Some(rest) = comment.strip_prefix('#') {
        rest.trim_start().to_string()
    } else if let Some(rest) = comment.strip_prefix("/* ") {
        rest.trim_end_matches(" */").to_string()
    } else {
        comment.to_string()
    }
}

fn infer_comment_style_from_text(comment: &str) -> CommentStyle {
    if comment.starts_with('#') {
        CommentStyle::Hash
    } else if comment.starts_with("/*") {
        CommentStyle::Block
    } else {
        CommentStyle::Line
    }
}

fn create_comment_token(text: &str, style: CommentStyle, newline: &str) -> rowan::GreenToken {
    let text = normalize_newlines_for_style(text, newline);
    let (kind, content) = match style {
        CommentStyle::Line => (SyntaxKind::LineComment, format!("// {}", text)),
        CommentStyle::Hash => (SyntaxKind::HashComment, format!("# {}", text)),
        CommentStyle::Block => (SyntaxKind::BlockComment, format!("/* {} */", text)),
    };
    kind.create_token(&content)
}

fn append_comment_tokens(
    children: &mut Vec<GreenElement>,
    text: &str,
    style: CommentStyle,
    newline: &str,
) {
    match style {
        CommentStyle::Block => {
            children.push(NodeOrToken::Token(create_comment_token(
                text, style, newline,
            )));
        }
        CommentStyle::Line | CommentStyle::Hash => {
            let normalized = normalize_newlines_for_style(text, newline);
            let mut lines = normalized.split(newline).peekable();
            while let Some(line) = lines.next() {
                children.push(NodeOrToken::Token(create_comment_token(
                    line, style, newline,
                )));
                if lines.peek().is_some() {
                    children.push(NodeOrToken::Token(
                        SyntaxKind::Newline.create_token(newline),
                    ));
                }
            }
        }
    }
}

fn is_leading_comment_trivia(elem: &GreenElement) -> bool {
    matches!(
        elem,
        NodeOrToken::Token(t)
            if matches!(
                crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()),
                SyntaxKind::Whitespace
                    | SyntaxKind::Newline
                    | SyntaxKind::LineComment
                    | SyntaxKind::HashComment
                    | SyntaxKind::BlockComment
            )
    )
}

fn determine_comment_style(
    pair_green: &GreenNode,
    style: Option<CommentStyle>,
) -> Result<CommentStyle, HjsonEditError> {
    if let Some(s) = style {
        return Ok(s);
    }

    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            match kind {
                SyntaxKind::LineComment => return Ok(CommentStyle::Line),
                SyntaxKind::HashComment => return Ok(CommentStyle::Hash),
                SyntaxKind::BlockComment => return Ok(CommentStyle::Block),
                _ => {}
            }
        }
    }

    Ok(CommentStyle::Line)
}

fn insert_comment_before_pair(
    pair_green: &GreenNode,
    comment: rowan::GreenToken,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();
    let mut prefix_trivia: Vec<GreenElement> = Vec::new();
    let mut line_indent: Vec<GreenElement> = Vec::new();

    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Whitespace || kind == SyntaxKind::Newline {
                let elem = NodeOrToken::Token(kind.create_token(t.text()));
                prefix_trivia.push(elem.clone());
                if kind == SyntaxKind::Newline {
                    line_indent.clear();
                } else if kind == SyntaxKind::Whitespace {
                    line_indent.push(elem);
                }
                continue;
            }
        }
        break;
    }

    // If no indentation was found in Pair, it might be in the ObjectEntry's leading_line_trivia.
    // However, since we are inside Pair, we should at least ensure the comment itself is indented
    // if the key was indented. The problem is that prefix_trivia might be empty if the
    // indentation is handled by the parent Object's formatting.

    children.extend(prefix_trivia.clone());
    children.push(NodeOrToken::Token(comment));
    children.push(NodeOrToken::Token(
        SyntaxKind::Newline.create_token(newline),
    ));
    // If prefix_trivia had no newline, it means it's just indentation.
    // We should use it for both the comment and the key.
    if !prefix_trivia.is_empty() && prefix_trivia.iter().all(|t| matches!(t.as_token(), Some(tok) if crate::syntax::kind::HjsonLanguage::kind_from_raw(tok.kind()) == SyntaxKind::Whitespace)) {
        // Current children: [indent, comment, newline]
        // We want: [indent, comment, newline, indent, key...]
        children.extend(prefix_trivia);
    } else {
        children.extend(line_indent);
    }

    let mut found_non_trivia = false;
    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if !found_non_trivia {
                if kind == SyntaxKind::Whitespace
                    || kind == SyntaxKind::Newline
                    || matches!(
                        kind,
                        SyntaxKind::LineComment
                            | SyntaxKind::HashComment
                            | SyntaxKind::BlockComment
                    )
                {
                    continue;
                }
                found_non_trivia = true;
            }
        } else {
            found_non_trivia = true;
        }
        children.push(clone_element(child));
    }

    Ok(SyntaxKind::Pair.create_node(children))
}

fn remove_comments_from_pair(pair_green: &GreenNode) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();
    let mut found_non_trivia = false;

    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if !found_non_trivia
                && matches!(
                    kind,
                    SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment
                )
            {
                continue;
            }
            if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Newline {
                found_non_trivia = true;
            }
        } else {
            found_non_trivia = true;
        }
        children.push(clone_element(child));
    }

    Ok(SyntaxKind::Pair.create_node(children))
}

fn set_inline_comment(
    obj_green: &GreenNode,
    pair_idx: usize,
    text: &str,
    style: Option<CommentStyle>,
    newline: &str,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();
    let mut pair_count = 0;
    let mut pair_end_pos = None;
    let mut comma_pos = None;

    for (i, child) in children.iter().enumerate() {
        if let NodeOrToken::Node(n) = child {
            if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair {
                if pair_count == pair_idx {
                    pair_end_pos = Some(i);
                }
                pair_count += 1;
            }
        }
        if pair_count == pair_idx + 1 {
            if let NodeOrToken::Token(t) = child {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                if kind == SyntaxKind::Comma {
                    comma_pos = Some(i);
                    break;
                }
            }
        }
    }

    if let Some(pair_end) = pair_end_pos {
        let style = style.unwrap_or(CommentStyle::Line);
        let style =
            if comma_pos.is_some() && matches!(style, CommentStyle::Line | CommentStyle::Hash) {
                CommentStyle::Block
            } else {
                style
            };
        let comment = create_comment_token(text, style, newline);
        let start = pair_end + 1;
        let mut end = start;
        while end < children.len() {
            match &children[end] {
                NodeOrToken::Token(t) => {
                    let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                    if matches!(
                        kind,
                        SyntaxKind::Whitespace
                            | SyntaxKind::LineComment
                            | SyntaxKind::HashComment
                            | SyntaxKind::BlockComment
                    ) {
                        end += 1;
                        continue;
                    }
                }
                NodeOrToken::Node(_) => {}
            }
            break;
        }
        let mut replacement = vec![
            NodeOrToken::Token(SyntaxKind::Whitespace.create_token("  ")),
            NodeOrToken::Token(comment),
        ];
        if let Some(comma_pos) = comma_pos {
            if comma_pos >= end {
                replacement.push(NodeOrToken::Token(SyntaxKind::Whitespace.create_token(" ")));
            }
        }
        children.splice(start..end, replacement);
    }

    Ok(SyntaxKind::Object.create_node(children))
}

fn normalize_newlines_for_style(text: &str, newline: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\r' && chars.peek() == Some(&'\n') {
            chars.next();
            result.push_str(newline);
        } else if ch == '\n' {
            result.push_str(newline);
        } else {
            result.push(ch);
        }
    }

    result
}

fn comment_path_newline_kind(
    document: &HjsonDocument,
    path: &CommentPath,
) -> Result<NewlineKind, HjsonEditError> {
    match path {
        CommentPath::Header => Ok(header_comment_newline_kind(document)),
        CommentPath::Footer => Ok(footer_comment_newline_kind(document)),
        CommentPath::Key { dot_path } => {
            let resolver = CommentTargetResolver::new(document, dot_path)?;
            let obj = resolver.object()?;
            let pair = resolver.pair_node()?;
            Ok(pair_comment_newline_kind(document, &obj, &pair))
        }
        CommentPath::Inline { dot_path } => {
            let resolver = CommentTargetResolver::new(document, dot_path)?;
            let obj = resolver.object()?;
            Ok(contextual_newline_kind(document, &obj))
        }
    }
}

fn pair_comment_newline_kind(
    document: &HjsonDocument,
    obj: &RedNode,
    pair: &RedNode,
) -> NewlineKind {
    leading_comment_newline_kind(pair)
        .or_else(|| Some(contextual_newline_kind(document, obj)))
        .unwrap_or_else(|| document.try_detect_newline().unwrap_or_default())
}

fn leading_comment_newline_kind(pair: &RedNode) -> Option<NewlineKind> {
    let mut elements = Vec::new();

    for elem in pair.children_with_tokens() {
        let Some(token) = elem.as_token() else {
            break;
        };

        let kind = token.kind();
        if matches!(
            kind,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::LineComment
                | SyntaxKind::HashComment
                | SyntaxKind::BlockComment
        ) {
            elements.push(clone_red_or_token(elem));
            continue;
        }
        break;
    }

    elements.try_detect_newline()
}

fn header_comment_newline_kind(document: &HjsonDocument) -> NewlineKind {
    let red = RedNode::new_root(document.green_node().clone());
    let mut elements = Vec::new();

    for elem in red.children_with_tokens() {
        let Some(token) = elem.as_token() else {
            break;
        };

        let kind = token.kind();
        if matches!(
            kind,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::LineComment
                | SyntaxKind::HashComment
                | SyntaxKind::BlockComment
        ) {
            elements.push(clone_red_or_token(elem));
            continue;
        }
        break;
    }

    elements
        .try_detect_newline()
        .unwrap_or_else(|| document.try_detect_newline().unwrap_or_default())
}

fn footer_comment_newline_kind(document: &HjsonDocument) -> NewlineKind {
    let red = RedNode::new_root(document.green_node().clone());
    let children: Vec<_> = red.children_with_tokens().collect();
    let mut elements = Vec::new();

    for elem in children.iter().rev() {
        let Some(token) = elem.as_token() else {
            break;
        };

        let kind = token.kind();
        if matches!(
            kind,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::LineComment
                | SyntaxKind::HashComment
                | SyntaxKind::BlockComment
        ) {
            elements.push(clone_red_or_token(elem.clone()));
            continue;
        }
        break;
    }
    elements.reverse();

    elements
        .try_detect_newline()
        .unwrap_or_else(|| document.try_detect_newline().unwrap_or_default())
}

fn remove_inline_comment(
    obj_green: &GreenNode,
    pair_idx: usize,
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();
    let mut pair_count = 0;
    let mut remove_range = None;

    for (i, child) in children.iter().enumerate() {
        if let NodeOrToken::Node(n) = child {
            if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair {
                if pair_count == pair_idx {
                    let start = i + 1;
                    let mut end = start;
                    for (j, child) in children.iter().enumerate().skip(start) {
                        match child {
                            NodeOrToken::Token(t) => {
                                let kind =
                                    crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                                if matches!(
                                    kind,
                                    SyntaxKind::LineComment
                                        | SyntaxKind::HashComment
                                        | SyntaxKind::BlockComment
                                ) {
                                    end = j + 1;
                                } else if kind != SyntaxKind::Whitespace
                                    && kind != SyntaxKind::Newline
                                {
                                    break;
                                }
                            }
                            NodeOrToken::Node(_) => break,
                        }
                    }
                    if end > start {
                        remove_range = Some((start, end));
                    }
                    break;
                }
                pair_count += 1;
            }
        }
    }

    if let Some((start, end)) = remove_range {
        children.drain(start..end);
    }

    Ok(SyntaxKind::Object.create_node(children))
}

fn extract_inline_comments(obj_green: &GreenNode, pair_idx: usize) -> Vec<String> {
    let children: Vec<GreenElement> = obj_green.children().map(clone_element).collect();
    let mut pair_count = 0;
    let mut comments = Vec::new();

    for (i, child) in children.iter().enumerate() {
        if let NodeOrToken::Node(n) = child {
            if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair {
                if pair_count == pair_idx {
                    for next in children.iter().skip(i + 1) {
                        match next {
                            NodeOrToken::Token(t) => {
                                let kind =
                                    crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                                if matches!(
                                    kind,
                                    SyntaxKind::LineComment
                                        | SyntaxKind::HashComment
                                        | SyntaxKind::BlockComment
                                ) {
                                    comments.push(t.text().to_string());
                                } else if kind != SyntaxKind::Whitespace
                                    && kind != SyntaxKind::Newline
                                {
                                    break;
                                }
                            }
                            NodeOrToken::Node(_) => break,
                        }
                    }
                    break;
                }
                pair_count += 1;
            }
        }
    }

    comments
}
