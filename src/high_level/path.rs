use std::fmt::Debug;

use crate::high_level::error::HjsonEditError;

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPathSegment {
    Index(usize),    // [n]
    InsertAt(usize), // [+n]
    Push,            // [-]
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Key(String),
    Array(ArrayPathSegment),
}

#[derive(Debug, Clone)]
pub struct HjsonObjectPath {
    segments: Vec<PathSegment>,
}

#[derive(Debug, Clone)]
pub enum HjsonCommentPath {
    Header,
    Footer,
    Before(HjsonObjectPath),
    Inline(HjsonObjectPath),
}

impl TryFrom<&str> for HjsonCommentPath {
    type Error = HjsonEditError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        if raw == ":header" {
            Ok(Self::Header)
        } else if raw == ":footer" {
            Ok(Self::Footer)
        } else if let Some(stripped) = raw.strip_suffix(":inline") {
            Ok(Self::Inline(HjsonObjectPath::parse(stripped.to_string())?))
        } else if let Some(stripped) = raw.strip_suffix(":before") {
            Ok(Self::Before(HjsonObjectPath::parse(stripped.to_string())?))
        } else if raw.contains(':') {
            Err(HjsonEditError::InvalidPath(format!(
                "unknown comment suffix in path: {raw:?}"
            )))
        } else {
            Ok(Self::Before(HjsonObjectPath::parse(raw.to_string())?))
        }
    }
}

impl HjsonObjectPath {
    pub fn from_segments(segments: Vec<PathSegment>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    pub fn segment(&self, index: usize) -> Option<&PathSegment> {
        self.segments.get(index)
    }

    pub fn parse(path: impl Into<String>) -> Result<Self, HjsonEditError> {
        let path = path.into();

        if path.starts_with('.') {
            return Err(HjsonEditError::InvalidPath(format!(
                "path must not start with '.': {path:?}"
            )));
        }
        if path.ends_with('.') {
            return Err(HjsonEditError::InvalidPath(format!(
                "path must not end with '.': {path:?}"
            )));
        }
        if path.contains("..") {
            return Err(HjsonEditError::InvalidPath(format!(
                "path must not contain consecutive dots: {path:?}"
            )));
        }

        let mut segments = Vec::new();
        let mut chars = path.chars().peekable();

        while chars.peek().is_some() {
            // Parse key part
            let mut key = String::new();
            let mut in_brackets = false;

            while let Some(&c) = chars.peek() {
                if c == '.' {
                    chars.next();
                    break;
                } else if c == '[' {
                    in_brackets = true;
                    break;
                } else {
                    key.push(c);
                    chars.next();
                }
            }

            if !key.is_empty() {
                segments.push(PathSegment::Key(key));
            }

            // Parse array index if present
            if in_brackets {
                chars.next(); // consume '['

                // Check for special operators
                let mut op_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ']' {
                        break;
                    } else {
                        op_str.push(c);
                        chars.next();
                    }
                }

                // Expect closing bracket
                if chars.peek() != Some(&']') {
                    return Err(HjsonEditError::InvalidPath(path.clone()));
                }
                chars.next(); // consume ']'

                // Parse the operation
                if op_str == "-" {
                    segments.push(PathSegment::Array(ArrayPathSegment::Push));
                } else if op_str.starts_with('+') {
                    let idx: usize = op_str[1..]
                        .parse()
                        .map_err(|_| HjsonEditError::InvalidPath(path.clone()))?;
                    segments.push(PathSegment::Array(ArrayPathSegment::InsertAt(idx)));
                } else {
                    let idx: usize = op_str
                        .parse()
                        .map_err(|_| HjsonEditError::InvalidPath(path.clone()))?;
                    segments.push(PathSegment::Array(ArrayPathSegment::Index(idx)));
                }
            }
        }

        Ok(HjsonObjectPath { segments })
    }

    pub fn split_first(&self) -> Option<(PathSegment, Self)> {
        let (left, right) = self.split_at(1)?;
        let first = left.segments.into_iter().next()?;
        Some((first, right))
    }

    pub fn split_last(&self) -> Option<(Self, PathSegment)> {
        if self.segments.is_empty() {
            return None;
        }
        let (left, mut right) = self.split_at(self.segments.len() - 1)?;
        let last = right.segments.pop()?;
        Some((left, last))
    }

    pub fn split_at(&self, index: usize) -> Option<(Self, Self)> {
        if index > self.segments.len() {
            return None;
        }
        let (left, right) = self.segments.split_at(index);
        Some((
            Self::from_segments(left.to_vec()),
            Self::from_segments(right.to_vec()),
        ))
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }
}


