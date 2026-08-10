use super::super::ObjectRef;
use crate::{
    models::{ClassProperties, ClassPropertiesVersion},
    resource::SourceRef,
};
use zadt_macros::object_type;

/// An ABAP class object.
#[object_type(
    workbench_type = "CLAS/OC",
    collection(
        scheme = "http://www.sap.com/adt/categories/oo",
        term = "classes",
    ),
    capabilities(
        Source,
        SourceComponents(ClassSourceComponent),
        Properties(
            media_version = ClassPropertiesVersion,
            model = ClassProperties,
        ),
    ),
)]
#[derive(Debug)]
pub enum Class {}

/// A secondary source component owned and locked by an ABAP class.
///
/// Local class includes are ADT resources beneath the class object rather than
/// independent repository objects.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, zadt_macros::SourceComponent)]
#[source_component(prefix = "includes")]
pub enum ClassSourceComponent {
    Definitions,
    Implementations,
    Macros,
    TestClasses,
    LocalTypes,
}

impl serde::Serialize for ClassSourceComponent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl ObjectRef<Class> {
    /// Resolves one of the secondary source resources owned by this class.
    pub fn component_source(&self, component: ClassSourceComponent) -> SourceRef {
        self.source_from_component(&component)
    }

    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::typed(name.to_ascii_uppercase(), uri)
    }
}
