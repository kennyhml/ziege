use crate::CategoryId;

mod capabilities;
pub(crate) mod descriptors;
mod object;
mod reference;
mod types;
mod workbench;

pub use capabilities::{
    Create, CreationPropertyModel, PropertyModel, Source, SourceComponents, Structure,
    UpdateProperties,
};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::ObjectTypeDescriptor;
pub(crate) use descriptors::RuntimeObjectTypeDescriptor;
pub use object::{AnyObject, Object};
pub use reference::{AdvertisedObjectReference, ObjectRef, ObjectReferences};
pub use types::{
    AccessControl, AccessControlCreateProperties, AccessControlCreatePropertiesBuilder,
    AccessControlCreatePropertiesBuilderError, AccessControlProperties,
    AccessControlPropertiesVersion, Class, ClassCategory, ClassCreateProperties,
    ClassCreatePropertiesBuilder, ClassCreatePropertiesBuilderError, ClassProperties,
    ClassPropertiesVersion, ClassSourceComponent, ClassSourceProperties, ClassTemplate,
    ClassTemplateProperty, DataDefinition, DataDefinitionCreateProperties,
    DataDefinitionCreatePropertiesBuilder, DataDefinitionCreatePropertiesBuilderError,
    DataDefinitionProperties, DataDefinitionPropertiesVersion, DataElement, DataElementDefinition,
    DataElementProperties, DataElementPropertiesVersion, Include, IncludeProperties,
    IncludePropertyVersion, Package, PackageAssignment, PackageAttributes, PackageProperties,
    PackagePropertiesVersion, PackageTransport, PackageUseAccess, Program, ProgramProperties,
    ProgramPropertiesVersion, SyntaxConfiguration, SyntaxLanguage,
};
pub use workbench::{
    AbapLanguageVersion, GlobalWorkbenchType, InvalidWorkbenchType, ObjectVersion,
};

pub(crate) mod private {
    pub trait Sealed {}
}

/// Statically identified ADT object resource family.
pub trait ObjectType: private::Sealed + Send + Sync + Sized + 'static {
    /// The complete properties payload loaded for this object family.
    type Properties: PropertyModel;

    /// The object's global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;

    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;
}
