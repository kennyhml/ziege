use crate::vocabulary::CategoryId;

mod capabilities;
mod descriptors;
mod families;
mod reference;
mod version;
mod workbench;

pub use capabilities::{HasSource, PropertyModel, ReadProperties, UpdateProperties};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::RuntimeObjectTypeDescriptor;
pub use families::{
    Class, ClassProperties, ClassPropertiesVersion, ClassSourceComponent, ClassSourceProperties,
    DataElement, DataElementDefinition, DataElementProperties, DataElementPropertiesVersion,
    Include, IncludeProperties, IncludePropertyVersion, Package, PackageAssignment,
    PackageAttributes, PackageProperties, PackagePropertiesVersion, PackageTransport,
    PackageUseAccess, Program, ProgramProperties, ProgramPropertiesVersion, SyntaxConfiguration,
    SyntaxLanguage,
};
pub use reference::{AdvertisedObjectReference, Erased, ObjectRef, ObjectReferences};
pub use version::ObjectVersion;
pub use workbench::{GlobalWorkbenchType, InvalidWorkbenchType};

pub(crate) mod private {
    pub trait Sealed {}
}

/// Statically identified ADT object resource family.
pub trait ObjectType: private::Sealed + Clone + Default + Send + Sync + Sized + 'static {
    /// The object's global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;

    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;
}
