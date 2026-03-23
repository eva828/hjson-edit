use crate::high_level::{path::HjsonPath, value::HjsonValue, HjsonDocument};

pub trait HjsonDocumentObjectEdit {
    fn get(&self, path: impl Into<HjsonPath>) -> Option<HjsonValue>;
    fn set(self, path: impl Into<HjsonPath>, value: impl Into<HjsonValue>) -> Self;
    fn remove(self, path: impl Into<HjsonPath>) -> Self;
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(&self, path: impl Into<HjsonPath>) -> Option<HjsonValue> {
        unimplemented!()
    }

    fn set(self, path: impl Into<HjsonPath>, value: impl Into<HjsonValue>) -> Self {
        unimplemented!()
    }

    fn remove(self, path: impl Into<HjsonPath>) -> Self {
        unimplemented!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringStyle {
    Preserve,
    Auto,
    DoubleQuoted,
    Block,
    Unquoted,
}
