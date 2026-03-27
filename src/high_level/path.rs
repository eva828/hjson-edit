use std::fmt::Debug;

use crate::high_level::error::HjsonEditError;

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectPathSegment {
    Key(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPathSegment {
    Index(usize),    // [n]
    InsertAt(usize), // [+n]
    Push,            // [-]
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Object(ObjectPathSegment),
    Array(ArrayPathSegment),
}

#[derive(Debug, Clone)]
pub struct HjsonPath {
    pub segments: Vec<PathSegment>,
}

impl HjsonPath {
    pub fn new(path: impl Into<HjsonPath>) -> Self {
        path.into()
    }

    pub fn parse_dot_path(s: &str) -> Result<Self, HjsonEditError> {
        let mut segments = Vec::new();
        let mut chars = s.chars().peekable();

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
                segments.push(PathSegment::Object(ObjectPathSegment::Key(key)));
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
                    return Err(HjsonEditError::InvalidPath(s.to_string()));
                }
                chars.next(); // consume ']'

                // Parse the operation
                if op_str == "-" {
                    segments.push(PathSegment::Array(ArrayPathSegment::Push));
                } else if op_str.starts_with('+') {
                    let idx: usize = op_str[1..]
                        .parse()
                        .map_err(|_| HjsonEditError::InvalidPath(s.to_string()))?;
                    segments.push(PathSegment::Array(ArrayPathSegment::InsertAt(idx)));
                } else {
                    let idx: usize = op_str
                        .parse()
                        .map_err(|_| HjsonEditError::InvalidPath(s.to_string()))?;
                    segments.push(PathSegment::Array(ArrayPathSegment::Index(idx)));
                }
            }
        }

        Ok(HjsonPath { segments })
    }

    pub fn split_first(&self) -> Option<(PathSegment, Self)> {
        let first = self.segments.first()?.clone();
        let rest = self.segments[1..].to_vec();
        Some((first, Self { segments: rest }))
    }

    pub fn split_last(&self) -> Option<(Self, PathSegment)> {
        if self.segments.is_empty() {
            return None;
        }
        let mut parent = self.segments.clone();
        let last = parent.pop()?;
        Some((Self { segments: parent }, last))
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }
}

impl Into<HjsonPath> for &str {
    fn into(self) -> HjsonPath {
        HjsonPath::parse_dot_path(self).unwrap_or_else(|_| HjsonPath { segments: vec![] })
    }
}

impl Into<HjsonPath> for &[PathSegment] {
    fn into(self) -> HjsonPath {
        HjsonPath {
            segments: self.to_vec(),
        }
    }
}

impl Into<HjsonPath> for Vec<PathSegment> {
    fn into(self) -> HjsonPath {
        HjsonPath { segments: self }
    }
}

impl ObjectPathSegment {
    pub fn match_key(&self, key: &str) -> bool {
        match self {
            ObjectPathSegment::Key(k) => k == key,
        }
    }
}
