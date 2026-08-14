use super::super::ObjectRef;
use crate::{models::ClassProperties, resource::SourceRef};
use zadt_macros::object_type;

/// An ABAP class object.
#[object_type(
    workbench_type = "CLAS/OC",
    collection(
        scheme = "http://www.sap.com/adt/categories/oo",
        term = "classes",
    ),
    capabilities(
        HasSource,
        SourceComponents(ClassSourceComponent),
        Run,
        ReadProperties(model = ClassProperties),
    ),
)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Class;

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

impl ObjectRef<Class> {
    /// Resolves one of the secondary source resources owned by this class.
    pub fn component_source(&self, component: ClassSourceComponent) -> SourceRef {
        SourceRef::from_object_path(self.erase(), component.path())
    }

    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::new(name.to_ascii_uppercase(), uri)
    }
}
