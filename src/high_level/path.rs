#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

pub struct HjsonPath {
    segments: Vec<PathSegment>,
}

impl HjsonPath {
    pub fn new(path: impl Into<HjsonPath>) -> Self {
        path.into()
    }
}

impl Into<HjsonPath> for &str {
    fn into(self) -> HjsonPath {
        unimplemented!("Initialize path from a single string is not supported. Use a vector of strings or PathSegments instead.");
    }
}

impl Into<HjsonPath> for &[&str] {
    fn into(self) -> HjsonPath {
        HjsonPath {
            segments: self
                .iter()
                .map(|s| PathSegment::Key(s.to_string()))
                .collect(),
        }
    }
}

impl Into<HjsonPath> for Vec<PathSegment> {
    fn into(self) -> HjsonPath {
        HjsonPath { segments: self }
    }
}

impl Into<HjsonPath> for &[PathSegment] {
    fn into(self) -> HjsonPath {
        HjsonPath {
            segments: self.to_vec(),
        }
    }
}
