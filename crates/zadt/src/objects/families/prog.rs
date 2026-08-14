use super::super::ObjectRef;
use crate::models::{
    IncludeProperties, IncludePropertyVersion, ProgramProperties, ProgramPropertiesVersion,
};
use zadt_macros::object_type;

/// The ABAP program object type.
#[object_type(
    workbench_type = "PROG/P",
    collection(
        scheme = "http://www.sap.com/adt/categories/programs",
        term = "programs",
    ),
    capabilities(
        HasSource,
        Run,
        ReadProperties(
            media_version = ProgramPropertiesVersion,
            model = ProgramProperties,
        ),
    ),
)]
#[derive(Clone, Copy, Debug)]
pub struct Program;

/// The standalone ABAP include object type.
#[object_type(
    workbench_type = "PROG/I",
    collection(
        scheme = "http://www.sap.com/adt/categories/programs",
        term = "includes",
    ),
    capabilities(
        HasSource,
        ReadProperties(
            media_version = IncludePropertyVersion,
            model = IncludeProperties,
        ),
    ),
)]
#[derive(Clone, Copy, Debug)]
pub struct Include;

impl ObjectRef<Program> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::from_parts(name.to_ascii_uppercase(), uri)
    }
}

impl ObjectRef<Include> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::from_parts(name.to_ascii_uppercase(), uri)
    }
}
