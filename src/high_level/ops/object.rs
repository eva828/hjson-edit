use rowan::{GreenNode, GreenToken};

use crate::{
    high_level::{error::HjsonEditError, path::HjsonPath, value::HjsonValue, HjsonDocument},
    SyntaxKind,
};
// use crate::syntax::ext::RedNodeExt;

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
        // let path = path.into();
        // if path.segments.is_empty() {
        //     return Err(HjsonEditError::PathNotFound(path));
        // }

        // let target_node = navigate_to_path(&red_node, &path.segments, 0, false)?;

        // match target_node {
        //     Some(node) => Ok(HjsonValue::from(node)),
        //     None => Err(HjsonEditError::PathNotFound(path)),
        // }
        unimplemented!()
    }

    fn set(
        mut self,
        path: impl Into<HjsonPath>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let token = value.into().into_greennode();
        self.set_internal(&path.into(), token)?;
        Ok(self)
    }

    fn remove(mut self, path: impl Into<HjsonPath>) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        self.remove_internal(&path.into())?;
        Ok(self)
    }
}
