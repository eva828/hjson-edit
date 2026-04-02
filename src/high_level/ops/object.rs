use crate::high_level::{
    error::HjsonEditError,
    path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
    value::HjsonValue,
    HjsonDocument,
};

pub trait HjsonDocumentObjectEdit {
    fn get(
        &self,
        path: impl TryInto<HjsonObjectPath, Error = HjsonEditError>,
    ) -> Result<HjsonValue, HjsonEditError>;
    fn set(
        self,
        path: impl TryInto<HjsonObjectPath, Error = HjsonEditError>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
    fn remove(
        self,
        path: impl TryInto<HjsonObjectPath, Error = HjsonEditError>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized;
}

/// String rendering style used when writing values back into the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringStyle {
    /// Preserve whatever style already exists at the target node.
    #[default]
    Preserve,
    DoubleQuoted,
    Block,
    Unquoted,
}

impl HjsonDocumentObjectEdit for HjsonDocument {
    fn get(
        &self,
        path: impl TryInto<HjsonObjectPath, Error = HjsonEditError>,
    ) -> Result<HjsonValue, HjsonEditError> {
        let _path = path.try_into()?;
        unimplemented!()
    }

    fn set(
        self,
        path: impl TryInto<HjsonObjectPath, Error = HjsonEditError>,
        value: impl Into<HjsonValue>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.try_into()?;
        let _value = value.into();

        if path.segments().is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let last_segment = path.segments().last().unwrap();

        match last_segment {
            PathSegment::Array(ArrayPathSegment::Push) => {
                // Insert a new element at the end of the target array.
                unimplemented!()
            }
            PathSegment::Array(ArrayPathSegment::InsertAt(_index)) => {
                // Insert a new element at `_index` in the target array.
                unimplemented!()
            }
            _ => {
                // Update an existing object key / array element, creating missing
                // intermediate objects along the path as needed.
                unimplemented!()
            }
        }
    }

    fn remove(
        self,
        path: impl TryInto<HjsonObjectPath, Error = HjsonEditError>,
    ) -> Result<Self, HjsonEditError>
    where
        Self: Sized,
    {
        let path = path.try_into()?;

        if path.segments().is_empty() {
            return Err(HjsonEditError::PathNotFound(path));
        }

        let last_segment = path.segments().last().unwrap();

        match last_segment {
            PathSegment::Array(ArrayPathSegment::Index(_index)) => {
                // Remove the element at `_index` from the target array.
                unimplemented!()
            }
            _ => {
                // Remove the key-value pair at the target path.
                unimplemented!()
            }
        }
    }
}
