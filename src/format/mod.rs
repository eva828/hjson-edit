use std::collections::BTreeSet;

pub(crate) mod array;
pub(crate) mod core;
pub(crate) mod engine;
pub(crate) mod object;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormatTarget {
    Document,
    Path(String),
    Node(Vec<usize>),
}

impl FormatTarget {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(path.into())
    }

    pub fn node(path_from_root: Vec<usize>) -> Self {
        Self::Node(path_from_root)
    }
}

pub(crate) fn normalize_targets(targets: &[FormatTarget]) -> Vec<FormatTarget> {
    if targets
        .iter()
        .any(|target| matches!(target, FormatTarget::Document))
    {
        return vec![FormatTarget::Document];
    }

    let mut unique_nodes = BTreeSet::new();
    let mut unique_paths = BTreeSet::new();
    for target in targets {
        match target {
            FormatTarget::Node(path) => {
                unique_nodes.insert(path.clone());
            }
            FormatTarget::Path(path) => {
                unique_paths.insert(path.clone());
            }
            FormatTarget::Document => {}
        }
    }

    let mut result = Vec::new();
    for path in unique_paths {
        result.push(FormatTarget::Path(path));
    }

    'outer: for path in unique_nodes {
        for existing in &result {
            if let FormatTarget::Node(existing_path) = existing {
                if path.starts_with(existing_path) {
                    continue 'outer;
                }
            }
        }

        result.retain(|existing| match existing {
            FormatTarget::Node(existing_path) => !existing_path.starts_with(&path),
            FormatTarget::Document | FormatTarget::Path(_) => true,
        });
        result.push(FormatTarget::Node(path));
    }

    result
}
