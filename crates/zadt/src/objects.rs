use serde::{Serialize, de::DeserializeOwned};

use crate::vocabulary::CategoryId;

mod capabilities;
pub(crate) mod descriptors;
mod families;
mod language_version;
mod object;
mod reference;
mod version;
mod workbench;

pub use capabilities::{
    Create, CreationPropertyModel, PropertyModel, Source, SourceComponents, UpdateProperties,
};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::ObjectTypeDescriptor;
pub(crate) use descriptors::RuntimeObjectTypeDescriptor;
pub use families::{
    Class, ClassCategory, ClassCreateProperties, ClassCreatePropertiesBuilder,
    ClassCreatePropertiesBuilderError, ClassProperties, ClassPropertiesVersion,
    ClassSourceComponent, ClassSourceProperties, ClassTemplate, ClassTemplateProperty, DataElement,
    DataElementDefinition, DataElementProperties, DataElementPropertiesVersion, Include,
    IncludeProperties, IncludePropertyVersion, Package, PackageAssignment, PackageAttributes,
    PackageProperties, PackagePropertiesVersion, PackageTransport, PackageUseAccess, Program,
    ProgramProperties, ProgramPropertiesVersion, SyntaxConfiguration, SyntaxLanguage,
};
pub use language_version::AbapLanguageVersion;
pub use object::Object;
pub(crate) use object::validate_typed_object;
pub use reference::{AdvertisedObjectReference, ObjectRef, ObjectReferences};
pub use version::ObjectVersion;
pub use workbench::{GlobalWorkbenchType, InvalidWorkbenchType};

pub(crate) mod private {
    pub trait Sealed {}
}

/// Property representation selected by an object's static or runtime type state.
#[doc(hidden)]
pub trait ObjectState: private::Sealed + Send + Sync + Sized + 'static {
    type Representation: std::fmt::Debug + DeserializeOwned + Serialize + Send + Sync;

    fn validate_representation(
        reference: &ObjectRef<Self>,
        media_type: &str,
        properties: &Self::Representation,
    ) -> Result<(), crate::ObjectError>;
}

impl private::Sealed for () {}

impl ObjectState for () {
    type Representation = serde_json::Value;

    fn validate_representation(
        _reference: &ObjectRef<Self>,
        _media_type: &str,
        _properties: &Self::Representation,
    ) -> Result<(), crate::ObjectError> {
        Ok(())
    }
}

/// Statically identified ADT object resource family.
pub trait ObjectType:
    ObjectState<Representation = Self::Properties> + Send + Sync + Sized + 'static
{
    /// The complete properties payload loaded for this object family.
    type Properties: PropertyModel;

    /// The object's global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;

    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;
}
