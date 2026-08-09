use super::super::{
    GlobalWorkbenchType, ObjectCollection, ObjectNamePolicy, ObjectProperties, ObjectRef,
    ObjectType, Source, SourceComponent, private,
};
use crate::{
    error::ResponseError,
    models::{ClassProperties, ClassPropertiesVersion},
    protocol::EntityTag,
    resource::SourceRef,
    vocabulary::CategoryId,
};

/// An ABAP class object.
#[derive(Debug)]
pub enum Class {}

impl private::Sealed for Class {}

impl ObjectType for Class {
    const WORKBENCH_TYPE: GlobalWorkbenchType = GlobalWorkbenchType::new("CLAS/OC");
    const NAMING_POLICY: ObjectNamePolicy = ObjectNamePolicy::new(30);
    const SOURCE_COMPONENTS: &'static [&'static dyn SourceComponent] = &[
        &ClassSourceComponent::Main,
        &ClassSourceComponent::Definitions,
        &ClassSourceComponent::Implementations,
        &ClassSourceComponent::Macros,
        &ClassSourceComponent::TestClasses,
        &ClassSourceComponent::LocalTypes,
    ];
}

impl ObjectCollection for Class {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/oo",
        term: "classes",
    };
}

impl Source for Class {}

impl ObjectProperties for Class {
    type MediaVersion = ClassPropertiesVersion;
    type Properties = ClassProperties;

    fn parse(
        resource: &ObjectRef<Self>,
        version: Self::MediaVersion,
        body: &[u8],
        etag: Option<EntityTag>,
    ) -> Result<Self::Properties, ResponseError> {
        ClassProperties::parse(resource, version, body, etag)
    }
}

/// A source component owned and locked by an ABAP class.
///
/// Local class includes are ADT resources beneath the class object rather than
/// independent repository objects.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClassSourceComponent {
    Main,
    Definitions,
    Implementations,
    Macros,
    TestClasses,
    LocalTypes,
}

impl ClassSourceComponent {
    /// Returns the component name used by ADT.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Definitions => "definitions",
            Self::Implementations => "implementations",
            Self::Macros => "macros",
            Self::TestClasses => "testclasses",
            Self::LocalTypes => "localtypes",
        }
    }

    /// Parses a component name used by ADT.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "main" => Some(Self::Main),
            "definitions" => Some(Self::Definitions),
            "implementations" => Some(Self::Implementations),
            "macros" => Some(Self::Macros),
            "testclasses" => Some(Self::TestClasses),
            "localtypes" => Some(Self::LocalTypes),
            _ => None,
        }
    }
}

impl serde::Serialize for ClassSourceComponent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl SourceComponent for ClassSourceComponent {
    fn name(&self) -> &'static str {
        self.as_str()
    }

    fn path(&self) -> &'static [&'static str] {
        match self {
            Self::Main => &["source", "main"],
            Self::Definitions => &["includes", "definitions"],
            Self::Implementations => &["includes", "implementations"],
            Self::Macros => &["includes", "macros"],
            Self::TestClasses => &["includes", "testclasses"],
            Self::LocalTypes => &["includes", "localtypes"],
        }
    }

    fn is_primary(&self) -> bool {
        matches!(self, Self::Main)
    }
}

impl ObjectRef<Class> {
    /// Resolves one of the source resources owned by this class.
    pub fn component_source(&self, component: ClassSourceComponent) -> SourceRef {
        self.source_from_component(&component)
    }

    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::typed(name.to_ascii_uppercase(), uri)
    }
}
