use crate::high_level::error::HjsonEditError;

#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

pub struct HjsonPath {
    segments: Vec<PathSegment>,
}

impl HjsonPath {
    pub fn new(path: impl IntoPath) -> Result<Self, HjsonEditError> {
        path.into_path()
    }
}

pub trait IntoPath {
    fn into_path(self) -> Result<HjsonPath, HjsonEditError>;
}

impl IntoPath for &str {
    fn into_path(self) -> Result<HjsonPath, HjsonEditError> {
        unimplemented!("Initialize path from a single string is not supported. Use a vector of strings or PathSegments instead.");
    }
}

impl IntoPath for &[&str] {
    fn into_path(self) -> Result<HjsonPath, HjsonEditError> {
        Ok(HjsonPath {
            segments: self
                .iter()
                .map(|s| PathSegment::Key(s.to_string()))
                .collect(),
        })
    }
}

impl IntoPath for Vec<PathSegment> {
    fn into_path(self) -> Result<HjsonPath, HjsonEditError> {
        Ok(HjsonPath { segments: self })
    }
}

impl IntoPath for &[PathSegment] {
    fn into_path(self) -> Result<HjsonPath, HjsonEditError> {
        Ok(HjsonPath {
            segments: self.to_vec(),
        })
    }
}
