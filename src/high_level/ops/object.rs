use crate::high_level::{
    error::HjsonEditError,
    path::{ArrayPathSegment, HjsonPath, ObjectPathSegment, PathSegment},
    value::HjsonValue,
    HjsonDocument,
};
use crate::syntax::ext::RedNodeExt;
use crate::syntax::{GreenElement, RedNode, SyntaxKind};
use rowan::{GreenNode, GreenNodeBuilder, GreenToken, Language, NodeOrToken};

pub trait HjsonDocumentObjectEdit {
    fn get(&self, path: impl Into<HjsonPath>) -> Result<HjsonValue, HjsonEditError>;
    fn set(
        self,
        path: impl Into<HjsonPath>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringStyle {
    #[default]
    Preserve,
    Auto,
    DoubleQuoted,
    Block,
    Unquoted,
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(&self, path: impl Into<HjsonPath>) -> Result<HjsonValue, HjsonEditError> {
        let path = path.into();
        if path.segments.is_empty() {
            return Err(HjsonEditError::ChildNotFound(vec![]));
        }

        let red_node = RedNode::new_root(self.green.clone());
        let target_node = navigate_to_path(&red_node, &path.segments, 0, false)?;

        match target_node {
            Some(node) => node_to_value(&node),
            None => Err(HjsonEditError::ChildNotFound(path.segments)),
        }
    }

    fn set(
        self,
        path: impl Into<HjsonPath>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.into();
        let value = value.into();

        if path.segments.is_empty() {
            return Err(HjsonEditError::ChildNotFound(vec![]));
        }

        let new_green = rebuild_tree_with_set(self.green.clone(), &path.segments, &value)?;

        Ok(Self {
            green: new_green,
            string_style: self.string_style,
            commnt_style: self.commnt_style,
        })
    }

    fn remove(self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.into();

        if path.segments.is_empty() {
            return Err(HjsonEditError::ChildNotFound(vec![]));
        }

        let new_green = rebuild_tree_with_remove(self.green.clone(), &path.segments)?;

        Ok(Self {
            green: new_green,
            string_style: self.string_style,
            commnt_style: self.commnt_style,
        })
    }
}

fn navigate_to_path(
    node: &RedNode,
    segments: &[PathSegment],
    idx: usize,
    create_missing: bool,
) -> Result<Option<RedNode>, HjsonEditError> {
    if idx >= segments.len() {
        return Ok(Some(node.clone()));
    }

    // If we're at Root or Value, traverse to the actual content first
    if node.kind() == SyntaxKind::Root || node.kind() == SyntaxKind::Value {
        // First look for Object, Array, or Pair directly
        let direct = node.children().find(|child| {
            matches!(
                child.kind(),
                SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Pair
            )
        });

        if let Some(n) = direct {
            // Found Object/Array/Pair directly, navigate into it
            return navigate_to_path(&n, segments, idx, create_missing);
        }

        // If not found, look for Value and navigate into it
        if let Some(value_node) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::Value)
        {
            return navigate_to_path(&value_node, segments, idx, create_missing);
        }

        return Err(HjsonEditError::ChildNotFound(segments[..idx + 1].to_vec()));
    }

    let segment = &segments[idx];

    match segment {
        PathSegment::Object(ObjectPathSegment::Key(key)) => {
            if node.kind() == SyntaxKind::Object {
                let pair = node.find_pair_by_key(key);
                match pair {
                    Some(p) => {
                        // Navigate to the value inside the pair
                        let value_node =
                            p.children().find(|child| child.kind() == SyntaxKind::Value);
                        match value_node {
                            Some(v) => navigate_to_path(&v, segments, idx + 1, create_missing),
                            None => {
                                if create_missing {
                                    Ok(None)
                                } else {
                                    Err(HjsonEditError::ChildNotFound(segments[..idx + 1].to_vec()))
                                }
                            }
                        }
                    }
                    None => {
                        if create_missing {
                            Ok(None)
                        } else {
                            Err(HjsonEditError::ChildNotFound(segments[..idx + 1].to_vec()))
                        }
                    }
                }
            } else {
                Err(HjsonEditError::ChildNotFound(segments[..idx + 1].to_vec()))
            }
        }
        PathSegment::Array(op) => {
            if node.kind() == SyntaxKind::Array {
                let result = node.get_array_element(op)?;
                match result {
                    Some(element) => navigate_to_path(&element, segments, idx + 1, create_missing),
                    None => {
                        if create_missing
                            || matches!(op, ArrayPathSegment::Push | ArrayPathSegment::InsertAt(_))
                        {
                            Ok(None)
                        } else {
                            Err(HjsonEditError::ChildNotFound(segments[..idx + 1].to_vec()))
                        }
                    }
                }
            } else {
                Err(HjsonEditError::ChildNotFound(segments[..idx + 1].to_vec()))
            }
        }
    }
}

fn node_to_value(node: &RedNode) -> Result<HjsonValue, HjsonEditError> {
    match node.kind() {
        SyntaxKind::Value => {
            // Get the actual value content
            if let Some(child) = node.children().next() {
                node_to_value(&child)
            } else if let Some(token) = node.children_with_tokens().find_map(|e| {
                if let NodeOrToken::Token(t) = e {
                    Some(t)
                } else {
                    None
                }
            }) {
                token_to_value(&token)
            } else {
                Ok(HjsonValue::Null)
            }
        }
        SyntaxKind::Object => {
            let mut pairs = Vec::new();
            for pair in node.children().filter(|c| c.kind() == SyntaxKind::Pair) {
                if let Some(key_node) = pair.children().find(|c| c.kind() == SyntaxKind::Key) {
                    let key = key_node
                        .children_with_tokens()
                        .find_map(|e| {
                            if let NodeOrToken::Token(t) = e {
                                if matches!(
                                    t.kind(),
                                    SyntaxKind::String
                                        | SyntaxKind::UnquotedString
                                        | SyntaxKind::BlockString
                                ) {
                                    Some(t.text().trim_matches('"').to_string())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    if let Some(value_node) =
                        pair.children().find(|c| c.kind() == SyntaxKind::Value)
                    {
                        let value = node_to_value(&value_node)?;
                        pairs.push((key, value));
                    }
                }
            }
            Ok(HjsonValue::Object(pairs))
        }
        SyntaxKind::Array => {
            let mut items = Vec::new();
            for value_node in node.children().filter(|c| c.kind() == SyntaxKind::Value) {
                items.push(node_to_value(&value_node)?);
            }
            Ok(HjsonValue::Array(items))
        }
        _ => {
            // Try to get value from token
            if let Some(token) = node.children_with_tokens().find_map(|e| {
                if let NodeOrToken::Token(t) = e {
                    Some(t)
                } else {
                    None
                }
            }) {
                token_to_value(&token)
            } else {
                Ok(HjsonValue::Null)
            }
        }
    }
}

fn token_to_value(token: &crate::syntax::RedToken) -> Result<HjsonValue, HjsonEditError> {
    let text = token.text();
    match token.kind() {
        SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString => {
            Ok(HjsonValue::String(text.trim_matches('"').to_string()))
        }
        SyntaxKind::Integer => text
            .parse::<i64>()
            .map(HjsonValue::Integer)
            .map_err(|_| HjsonEditError::ChildNotFound(vec![])),
        SyntaxKind::Float => text
            .parse::<f64>()
            .map(HjsonValue::Float)
            .map_err(|_| HjsonEditError::ChildNotFound(vec![])),
        SyntaxKind::True => Ok(HjsonValue::Bool(true)),
        SyntaxKind::False => Ok(HjsonValue::Bool(false)),
        SyntaxKind::Null => Ok(HjsonValue::Null),
        _ => Ok(HjsonValue::String(text.to_string())),
    }
}

// Convert child reference to owned GreenElement
fn child_to_element(
    child: &NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
) -> GreenElement {
    match child {
        NodeOrToken::Node(node_data) => {
            NodeOrToken::Node(GreenNode::from(std::borrow::Cow::Borrowed(*node_data)))
        }
        NodeOrToken::Token(token_data) => {
            NodeOrToken::Token(GreenToken::new(token_data.kind(), token_data.text()))
        }
    }
}

fn push_green_element(
    builder: &mut GreenNodeBuilder,
    element: &GreenElement,
) -> Result<(), HjsonEditError> {
    match element {
        NodeOrToken::Node(node) => {
            builder.start_node(node.kind());
            for child in node.children() {
                let elem = child_to_element(&child);
                push_green_element(builder, &elem)?;
            }
            builder.finish_node();
        }
        NodeOrToken::Token(token) => {
            builder.token(token.kind(), token.text());
        }
    }
    Ok(())
}

fn rebuild_tree_with_set(
    green: GreenNode,
    segments: &[PathSegment],
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    if segments.is_empty() {
        return Ok(value_to_green(value));
    }

    let mut builder = GreenNodeBuilder::new();
    builder.start_node(green.kind());

    let first_segment = &segments[0];
    let rest_segments = &segments[1..];

    let mut target_found = false;
    let mut value_index = 0usize;

    for child in green.children() {
        if target_found {
            // Already processed target, copy remaining children
            let elem = child_to_element(&child);
            push_green_element(&mut builder, &elem)?;
            continue;
        }

        match (first_segment, &child) {
            (PathSegment::Object(ObjectPathSegment::Key(key)), NodeOrToken::Node(node_data))
                if green.kind() == SyntaxKind::Object.into()
                    || green.kind() == SyntaxKind::Root.into() =>
            {
                let node = GreenNode::from(std::borrow::Cow::Borrowed(*node_data));
                if node.kind() == SyntaxKind::Pair.into() {
                    let pair_key = get_pair_key(&node);
                    if pair_key.as_deref() == Some(key) {
                        target_found = true;
                        if rest_segments.is_empty() {
                            // Replace this pair with new value
                            let new_pair = build_pair_with_value(key, value);
                            let elem = NodeOrToken::Node(new_pair);
                            push_green_element(&mut builder, &elem)?;
                        } else {
                            // Navigate into the pair's value
                            let new_pair = rebuild_pair_value(&node, rest_segments, value)?;
                            let elem = NodeOrToken::Node(new_pair);
                            push_green_element(&mut builder, &elem)?;
                        }
                    } else {
                        let elem = child_to_element(&child);
                        push_green_element(&mut builder, &elem)?;
                    }
                } else {
                    let elem = child_to_element(&child);
                    push_green_element(&mut builder, &elem)?;
                }
            }
            (PathSegment::Array(op), NodeOrToken::Node(node_data))
                if green.kind() == SyntaxKind::Array.into() =>
            {
                let node = GreenNode::from(std::borrow::Cow::Borrowed(*node_data));
                if node.kind() == SyntaxKind::Value.into() {
                    match op {
                        ArrayPathSegment::Index(target_idx) => {
                            if value_index == *target_idx {
                                target_found = true;
                                if rest_segments.is_empty() {
                                    // Replace this value
                                    let new_value = value_to_green(value);
                                    let elem = NodeOrToken::Node(new_value);
                                    push_green_element(&mut builder, &elem)?;
                                } else {
                                    // Navigate deeper
                                    let new_value =
                                        rebuild_tree_with_set(node, rest_segments, value)?;
                                    let elem = NodeOrToken::Node(new_value);
                                    push_green_element(&mut builder, &elem)?;
                                }
                            } else {
                                let elem = child_to_element(&child);
                                push_green_element(&mut builder, &elem)?;
                            }
                            value_index += 1;
                        }
                        ArrayPathSegment::InsertAt(target_idx) => {
                            if value_index == *target_idx {
                                target_found = true;
                                // Insert new value before this one
                                let new_value = value_to_green(value);
                                let elem = NodeOrToken::Node(new_value);
                                push_green_element(&mut builder, &elem)?;
                            }
                            let elem = child_to_element(&child);
                            push_green_element(&mut builder, &elem)?;
                            value_index += 1;
                        }
                        ArrayPathSegment::Push => {
                            // Push is handled at the end
                            let elem = child_to_element(&child);
                            push_green_element(&mut builder, &elem)?;
                            value_index += 1;
                        }
                    }
                } else {
                    let elem = child_to_element(&child);
                    push_green_element(&mut builder, &elem)?;
                }
            }
            _ => {
                let elem = child_to_element(&child);
                push_green_element(&mut builder, &elem)?;
            }
        }
    }

    // If target not found, we may need to create it
    if !target_found {
        match first_segment {
            PathSegment::Object(ObjectPathSegment::Key(key)) => {
                if green.kind() == SyntaxKind::Object.into()
                    || green.kind() == SyntaxKind::Root.into()
                {
                    let new_pair = if rest_segments.is_empty() {
                        build_pair_with_value(key, value)
                    } else {
                        // Create nested structure
                        let nested = create_missing_parents(rest_segments, value);
                        build_pair_with_node(key, nested)
                    };
                    let elem = NodeOrToken::Node(new_pair);
                    push_green_element(&mut builder, &elem)?;
                } else {
                    return Err(HjsonEditError::ChildNotFound(segments.to_vec()));
                }
            }
            PathSegment::Array(ArrayPathSegment::Push) => {
                if green.kind() == SyntaxKind::Array.into() {
                    let new_value = value_to_green(value);
                    let elem = NodeOrToken::Node(new_value);
                    push_green_element(&mut builder, &elem)?;
                } else {
                    return Err(HjsonEditError::ChildNotFound(segments.to_vec()));
                }
            }
            PathSegment::Array(ArrayPathSegment::InsertAt(_)) => {
                // If we didn't find the insertion point, the index was out of bounds
                return Err(HjsonEditError::ChildNotFound(segments.to_vec()));
            }
            PathSegment::Array(ArrayPathSegment::Index(_)) => {
                return Err(HjsonEditError::ChildNotFound(segments.to_vec()));
            }
        }
    }

    builder.finish_node();
    Ok(builder.finish())
}

fn rebuild_tree_with_remove(
    green: GreenNode,
    segments: &[PathSegment],
) -> Result<GreenNode, HjsonEditError> {
    if segments.is_empty() {
        return Err(HjsonEditError::ChildNotFound(vec![]));
    }

    let mut builder = GreenNodeBuilder::new();
    builder.start_node(green.kind());

    let first_segment = &segments[0];
    let rest_segments = &segments[1..];

    let mut target_found = false;
    let mut value_index = 0usize;

    for child in green.children() {
        if target_found {
            // Skip the target
            let elem = child_to_element(&child);
            push_green_element(&mut builder, &elem)?;
            continue;
        }

        match (first_segment, &child) {
            (PathSegment::Object(ObjectPathSegment::Key(key)), NodeOrToken::Node(node_data))
                if green.kind() == SyntaxKind::Object.into()
                    || green.kind() == SyntaxKind::Root.into() =>
            {
                let node = GreenNode::from(std::borrow::Cow::Borrowed(*node_data));
                if node.kind() == SyntaxKind::Pair.into() {
                    let pair_key = get_pair_key(&node);
                    if pair_key.as_deref() == Some(key) {
                        if rest_segments.is_empty() {
                            // Remove this pair
                            target_found = true;
                            continue;
                        } else {
                            // Navigate into the pair's value
                            let new_pair = rebuild_pair_value_remove(&node, rest_segments)?;
                            let elem = NodeOrToken::Node(new_pair);
                            push_green_element(&mut builder, &elem)?;
                        }
                    } else {
                        let elem = child_to_element(&child);
                        push_green_element(&mut builder, &elem)?;
                    }
                } else {
                    let elem = child_to_element(&child);
                    push_green_element(&mut builder, &elem)?;
                }
            }
            (
                PathSegment::Array(ArrayPathSegment::Index(target_idx)),
                NodeOrToken::Node(node_data),
            ) if green.kind() == SyntaxKind::Array.into() => {
                let node = GreenNode::from(std::borrow::Cow::Borrowed(*node_data));
                if node.kind() == SyntaxKind::Value.into() {
                    if value_index == *target_idx {
                        if rest_segments.is_empty() {
                            // Remove this value
                            target_found = true;
                            value_index += 1;
                            continue;
                        } else {
                            // Navigate deeper
                            let new_value = rebuild_tree_with_remove(node, rest_segments)?;
                            let elem = NodeOrToken::Node(new_value);
                            push_green_element(&mut builder, &elem)?;
                        }
                    } else {
                        let elem = child_to_element(&child);
                        push_green_element(&mut builder, &elem)?;
                    }
                    value_index += 1;
                } else {
                    let elem = child_to_element(&child);
                    push_green_element(&mut builder, &elem)?;
                }
            }
            _ => {
                let elem = child_to_element(&child);
                push_green_element(&mut builder, &elem)?;
            }
        }
    }

    if !target_found {
        return Err(HjsonEditError::ChildNotFound(segments.to_vec()));
    }

    builder.finish_node();
    Ok(builder.finish())
}

fn get_pair_key(pair: &GreenNode) -> Option<String> {
    for child in pair.children() {
        if let NodeOrToken::Node(node) = child {
            if node.kind() == SyntaxKind::Key.into() {
                for key_child in node.children() {
                    if let NodeOrToken::Token(token) = key_child {
                        let kind: SyntaxKind =
                            crate::syntax::HjsonLanguage::kind_from_raw(token.kind());
                        if matches!(kind, SyntaxKind::String | SyntaxKind::UnquotedString) {
                            return Some(token.text().trim_matches('"').to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

fn rebuild_pair_value(
    pair: &GreenNode,
    segments: &[PathSegment],
    value: &HjsonValue,
) -> Result<GreenNode, HjsonEditError> {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Pair.into());

    let mut found_value = false;

    for child in pair.children() {
        if found_value {
            let elem = child_to_element(&child);
            push_green_element(&mut builder, &elem)?;
            continue;
        }

        if let NodeOrToken::Node(node) = &child {
            let node_owned = GreenNode::from(std::borrow::Cow::Borrowed(*node));
            if node_owned.kind() == SyntaxKind::Value.into() {
                found_value = true;
                let new_value = rebuild_tree_with_set(node_owned, segments, value)?;
                let elem = NodeOrToken::Node(new_value);
                push_green_element(&mut builder, &elem)?;
                continue;
            }
        }
        let elem = child_to_element(&child);
        push_green_element(&mut builder, &elem)?;
    }

    builder.finish_node();
    Ok(builder.finish())
}

fn rebuild_pair_value_remove(
    pair: &GreenNode,
    segments: &[PathSegment],
) -> Result<GreenNode, HjsonEditError> {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Pair.into());

    let mut found_value = false;

    for child in pair.children() {
        if found_value {
            let elem = child_to_element(&child);
            push_green_element(&mut builder, &elem)?;
            continue;
        }

        if let NodeOrToken::Node(node) = &child {
            let node_owned = GreenNode::from(std::borrow::Cow::Borrowed(*node));
            if node_owned.kind() == SyntaxKind::Value.into() {
                found_value = true;
                let new_value = rebuild_tree_with_remove(node_owned, segments)?;
                let elem = NodeOrToken::Node(new_value);
                push_green_element(&mut builder, &elem)?;
                continue;
            }
        }
        let elem = child_to_element(&child);
        push_green_element(&mut builder, &elem)?;
    }

    builder.finish_node();
    Ok(builder.finish())
}

fn build_pair_with_value(key: &str, value: &HjsonValue) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Pair.into());

    // Key
    builder.start_node(SyntaxKind::Key.into());
    builder.token(SyntaxKind::UnquotedString.into(), key);
    builder.finish_node();

    // Colon
    builder.token(SyntaxKind::Colon.into(), ":");

    // Value
    let value_node = value_to_green(value);
    let elem = NodeOrToken::Node(value_node);
    push_green_element(&mut builder, &elem).unwrap();

    builder.finish_node();
    builder.finish()
}

fn build_pair_with_node(key: &str, node: GreenNode) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Pair.into());

    // Key
    builder.start_node(SyntaxKind::Key.into());
    builder.token(SyntaxKind::UnquotedString.into(), key);
    builder.finish_node();

    // Colon
    builder.token(SyntaxKind::Colon.into(), ":");

    // Value (the provided node)
    let elem = NodeOrToken::Node(node);
    push_green_element(&mut builder, &elem).unwrap();

    builder.finish_node();
    builder.finish()
}

fn value_to_green(value: &HjsonValue) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Value.into());

    match value {
        HjsonValue::Null => {
            builder.token(SyntaxKind::Null.into(), "null");
        }
        HjsonValue::Bool(b) => {
            builder.token(
                if *b {
                    SyntaxKind::True
                } else {
                    SyntaxKind::False
                }
                .into(),
                if *b { "true" } else { "false" },
            );
        }
        HjsonValue::Integer(n) => {
            builder.token(SyntaxKind::Integer.into(), &n.to_string());
        }
        HjsonValue::Float(n) => {
            builder.token(SyntaxKind::Float.into(), &n.to_string());
        }
        HjsonValue::String(s) => {
            builder.token(SyntaxKind::String.into(), &format!("\"{}\"", s));
        }
        HjsonValue::Array(items) => {
            builder.start_node(SyntaxKind::Array.into());
            builder.token(SyntaxKind::LBracket.into(), "[");

            for (i, item) in items.iter().enumerate() {
                let item_node = value_to_green(item);
                let elem = NodeOrToken::Node(item_node);
                push_green_element(&mut builder, &elem).unwrap();

                if i < items.len() - 1 {
                    builder.token(SyntaxKind::Comma.into(), ",");
                }
            }

            builder.token(SyntaxKind::RBracket.into(), "]");
            builder.finish_node();
        }
        HjsonValue::Object(pairs) => {
            builder.start_node(SyntaxKind::Object.into());
            builder.token(SyntaxKind::LBrace.into(), "{");

            for (i, (k, v)) in pairs.iter().enumerate() {
                let pair_node = build_pair_with_value(k, v);
                let elem = NodeOrToken::Node(pair_node);
                push_green_element(&mut builder, &elem).unwrap();

                if i < pairs.len() - 1 {
                    builder.token(SyntaxKind::Comma.into(), ",");
                }
            }

            builder.token(SyntaxKind::RBrace.into(), "}");
            builder.finish_node();
        }
    }

    builder.finish_node();
    builder.finish()
}

fn create_missing_parents(segments: &[PathSegment], value: &HjsonValue) -> GreenNode {
    if segments.is_empty() {
        return value_to_green(value);
    }

    let first = &segments[0];
    let rest = &segments[1..];

    match first {
        PathSegment::Object(ObjectPathSegment::Key(k)) => {
            let child = create_missing_parents(rest, value);
            build_pair_with_node(k, child)
        }
        PathSegment::Array(_) => {
            // For arrays in missing parent chain, create a single-element array
            let child = create_missing_parents(rest, value);
            let mut builder = GreenNodeBuilder::new();
            builder.start_node(SyntaxKind::Array.into());
            builder.token(SyntaxKind::LBracket.into(), "[");
            let elem = NodeOrToken::Node(child);
            push_green_element(&mut builder, &elem).unwrap();
            builder.token(SyntaxKind::RBracket.into(), "]");
            builder.finish_node();
            builder.finish()
        }
    }
}
