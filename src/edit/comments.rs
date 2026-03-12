use crate::edit::api::{create_node, create_token};
use crate::edit::high_level::navigation::RedNodeNavigateExt;
use crate::edit::high_level::{
    clone_element, CommentPath, CommentStyle, HjsonDocument, HjsonEditError, PathSegment,
};
use crate::syntax::green::GreenElement;
use crate::syntax::kind::SyntaxKind;
use crate::syntax::red::{RedNode, RedNodeCoreExt};
use rowan::Language;
use rowan::{GreenNode, NodeOrToken};

pub(crate) fn get_header_comment(document: &HjsonDocument) -> Vec<String> {
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

pub(crate) fn get_footer_comment(document: &HjsonDocument) -> Vec<String> {
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

pub(crate) fn comment_set(
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

pub(crate) fn comment_append(
    document: &HjsonDocument,
    path: &CommentPath,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let current_comments = match path {
        CommentPath::Key { dot_path } => get_comments_before_key(document, dot_path)?,
        CommentPath::Inline { dot_path } => get_inline_comment(document, dot_path)?,
        CommentPath::Header => get_header_comment(document),
        CommentPath::Footer => get_footer_comment(document),
    };

    if current_comments.is_empty() {
        return comment_set(document, path, text, style);
    }

    let append_style =
        style.unwrap_or_else(|| infer_comment_style_from_text(current_comments.last().unwrap()));
    if matches!(append_style, CommentStyle::Line | CommentStyle::Hash) && text.contains('\n') {
        return comment_set(document, path, text, Some(CommentStyle::Block));
    }

    let existing = current_comments
        .iter()
        .map(|comment| strip_comment_prefix(comment))
        .collect::<Vec<_>>()
        .join("\n");
    let new_text = format!("{}\n{}", existing, text);
    comment_set(document, path, &new_text, Some(append_style))
}

pub(crate) fn comment_remove(
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

fn set_key_comment(
    document: &HjsonDocument,
    dot_path: &str,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
    if path_segments.is_empty() {
        return Err(HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        });
    }

    let key = expect_terminal_key(&path_segments, dot_path)?;
    let parent_path = &path_segments[..path_segments.len() - 1];
    let obj = resolve_object_for_comment_path(document, parent_path, dot_path)?;

    let pair_node = obj
        .find_pair_node(key)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;

    // Try to find indentation from the Object children if the Pair node doesn't have it
    let mut indentation = Vec::new();
    let obj_green = obj.to_green();
    let mut pair_idx_in_obj = 0;
    for (i, child) in obj_green.children().enumerate() {
        if let NodeOrToken::Node(n) = child {
            if crate::syntax::kind::HjsonLanguage::kind_from_raw(n.kind()) == SyntaxKind::Pair {
                if *n == *pair_node.green() {
                    pair_idx_in_obj = i;
                    break;
                }
            }
        }
    }

    // Look backwards from pair_idx_in_obj for whitespace
    if pair_idx_in_obj > 0 {
        let mut idx = pair_idx_in_obj - 1;
        let mut temp_indent = Vec::new();
        loop {
            if let Some(NodeOrToken::Token(t)) = obj_green.children().nth(idx) {
                let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
                if kind == SyntaxKind::Whitespace {
                    temp_indent.push(NodeOrToken::Token(create_token(kind, t.text())));
                } else if kind == SyntaxKind::Newline {
                    indentation = temp_indent.into_iter().rev().collect();
                    break;
                } else {
                    break;
                }
            } else {
                break;
            }
            if idx == 0 {
                break;
            }
            idx -= 1;
        }
    }

    let mut pair_green = pair_node.to_green();

    // If we found indentation in the object and the pair doesn't start with whitespace,
    // prepend it to pair_green temporarily so insert_comment_before_pair can use it.
    if !indentation.is_empty() {
        let first_child = pair_green.children().next();
        let already_has_whitespace = matches!(first_child, Some(NodeOrToken::Token(t)) if crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind()) == SyntaxKind::Whitespace);
        if !already_has_whitespace {
            let mut new_pair_children = indentation.clone();
            new_pair_children.extend(pair_green.children().map(clone_element));
            pair_green = create_node(SyntaxKind::Pair, new_pair_children);
        }
    }

    let comment_style = determine_comment_style(&pair_green, style)?;
    let new_comment = create_comment_token(text, comment_style);
    let mut updated_pair = insert_comment_before_pair(&pair_green, new_comment)?;

    // If we prepended indentation to pair_green, we should remove it from the start of updated_pair
    // because it's still present in the Object's trivia.
    if !indentation.is_empty() {
        let mut final_children = Vec::new();
        let mut skip_count = indentation.len();
        for child in updated_pair.children() {
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }
            final_children.push(clone_element(child));
        }
        updated_pair = create_node(SyntaxKind::Pair, final_children);
    }

    document.replace_green_node_internal(&pair_node.path_from_root(), updated_pair)
}

fn set_inline_comment_for_path(
    document: &HjsonDocument,
    dot_path: &str,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
    if path_segments.is_empty() {
        return Err(HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        });
    }

    let key = expect_terminal_key(&path_segments, dot_path)?;
    let obj = resolve_object_for_comment_path(
        document,
        &path_segments[..path_segments.len() - 1],
        dot_path,
    )?;
    let pair_idx = obj
        .find_pair_index(key)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;
    let new_obj = set_inline_comment(&obj.to_green(), pair_idx, text, style)?;
    document.replace_green_node_internal(&obj.path_from_root(), new_obj)
}

fn set_header_comment(
    document: &HjsonDocument,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let style = style.unwrap_or(CommentStyle::Line);
    let children: Vec<GreenElement> = RedNode::new_root(document.green_node().clone())
        .children_with_tokens()
        .map(clone_red_or_token)
        .collect();
    let split_idx = children
        .iter()
        .position(|child| !is_leading_comment_trivia(child))
        .unwrap_or(children.len());

    let mut new_children: Vec<GreenElement> = Vec::new();
    append_comment_tokens(&mut new_children, text, style);
    if split_idx < children.len() {
        new_children.push(NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")));
        new_children.extend(children.into_iter().skip(split_idx));
    }

    Ok(HjsonDocument::from_green(create_node(
        SyntaxKind::Root,
        new_children,
    )))
}

fn set_footer_comment(
    document: &HjsonDocument,
    text: &str,
    style: Option<CommentStyle>,
) -> Result<HjsonDocument, HjsonEditError> {
    let style = style.unwrap_or(CommentStyle::Line);
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
        children.push(NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")));
    }
    append_comment_tokens(&mut children, text, style);

    Ok(HjsonDocument::from_green(create_node(
        SyntaxKind::Root,
        children,
    )))
}

fn remove_key_comment(
    document: &HjsonDocument,
    dot_path: &str,
) -> Result<HjsonDocument, HjsonEditError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
    if path_segments.is_empty() {
        return Err(HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        });
    }

    let key = expect_terminal_key(&path_segments, dot_path)?;
    let obj = resolve_object_for_comment_path(
        document,
        &path_segments[..path_segments.len() - 1],
        dot_path,
    )?;
    let pair_idx = obj
        .find_pair_index(key)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;
    let pair_node = obj
        .children_nodes()
        .enumerate()
        .filter(|(_, n)| n.kind() == SyntaxKind::Pair)
        .nth(pair_idx)
        .map(|(_, n)| n)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;
    let new_pair = remove_comments_from_pair(&pair_node.to_green())?;
    document.replace_green_node_internal(&pair_node.path_from_root(), new_pair)
}

fn remove_inline_comment_for_path(
    document: &HjsonDocument,
    dot_path: &str,
) -> Result<HjsonDocument, HjsonEditError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
    if path_segments.is_empty() {
        return Err(HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        });
    }

    let key = expect_terminal_key(&path_segments, dot_path)?;
    let obj = resolve_object_for_comment_path(
        document,
        &path_segments[..path_segments.len() - 1],
        dot_path,
    )?;
    let pair_idx = obj
        .find_pair_index(key)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;
    let new_obj = remove_inline_comment(&obj.to_green(), pair_idx)?;
    document.replace_green_node_internal(&obj.path_from_root(), new_obj)
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

    Ok(HjsonDocument::from_green(create_node(
        SyntaxKind::Root,
        new_children,
    )))
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

    Ok(HjsonDocument::from_green(create_node(
        SyntaxKind::Root,
        new_children,
    )))
}

fn get_comments_before_key(
    document: &HjsonDocument,
    dot_path: &str,
) -> Result<Vec<String>, HjsonEditError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
    if path_segments.is_empty() {
        return Err(HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        });
    }

    let key = expect_terminal_key(&path_segments, dot_path)?;
    let obj = resolve_object_for_comment_path(
        document,
        &path_segments[..path_segments.len() - 1],
        dot_path,
    )?;
    let pair_node = obj
        .find_pair_node(key)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;

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

fn get_inline_comment(
    document: &HjsonDocument,
    dot_path: &str,
) -> Result<Vec<String>, HjsonEditError> {
    let path_segments = crate::edit::high_level::parse_path_for_format(dot_path)?;
    if path_segments.is_empty() {
        return Err(HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        });
    }

    let key = expect_terminal_key(&path_segments, dot_path)?;
    let obj = resolve_object_for_comment_path(
        document,
        &path_segments[..path_segments.len() - 1],
        dot_path,
    )?;
    let pair_idx = obj
        .find_pair_index(key)
        .ok_or_else(|| HjsonEditError::InvalidPath {
            path: dot_path.to_string(),
        })?;
    Ok(extract_inline_comments(&obj.to_green(), pair_idx))
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

fn create_comment_token(text: &str, style: CommentStyle) -> rowan::GreenToken {
    let (kind, content) = match style {
        CommentStyle::Line => (SyntaxKind::LineComment, format!("// {}", text)),
        CommentStyle::Hash => (SyntaxKind::HashComment, format!("# {}", text)),
        CommentStyle::Block => (SyntaxKind::BlockComment, format!("/* {} */", text)),
    };
    create_token(kind, &content)
}

fn append_comment_tokens(children: &mut Vec<GreenElement>, text: &str, style: CommentStyle) {
    match style {
        CommentStyle::Block => {
            children.push(NodeOrToken::Token(create_comment_token(text, style)));
        }
        CommentStyle::Line | CommentStyle::Hash => {
            let mut lines = text.split('\n').peekable();
            while let Some(line) = lines.next() {
                children.push(NodeOrToken::Token(create_comment_token(line, style)));
                if lines.peek().is_some() {
                    children.push(NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")));
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
) -> Result<GreenNode, HjsonEditError> {
    let mut children: Vec<GreenElement> = Vec::new();
    let mut prefix_trivia: Vec<GreenElement> = Vec::new();
    let mut line_indent: Vec<GreenElement> = Vec::new();

    for child in pair_green.children() {
        if let NodeOrToken::Token(t) = child {
            let kind = crate::syntax::kind::HjsonLanguage::kind_from_raw(t.kind());
            if kind == SyntaxKind::Whitespace || kind == SyntaxKind::Newline {
                let elem = NodeOrToken::Token(create_token(kind, t.text()));
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
    children.push(NodeOrToken::Token(create_token(SyntaxKind::Newline, "\n")));
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

    Ok(create_node(SyntaxKind::Pair, children))
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

    Ok(create_node(SyntaxKind::Pair, children))
}

fn set_inline_comment(
    obj_green: &GreenNode,
    pair_idx: usize,
    text: &str,
    style: Option<CommentStyle>,
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
        let comment = create_comment_token(text, style);
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
            NodeOrToken::Token(create_token(SyntaxKind::Whitespace, "  ")),
            NodeOrToken::Token(comment),
        ];
        if let Some(comma_pos) = comma_pos {
            if comma_pos >= end {
                replacement.push(NodeOrToken::Token(create_token(
                    SyntaxKind::Whitespace,
                    " ",
                )));
            }
        }
        children.splice(start..end, replacement);
    }

    Ok(create_node(SyntaxKind::Object, children))
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

    Ok(create_node(SyntaxKind::Object, children))
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
