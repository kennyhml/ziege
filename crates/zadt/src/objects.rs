use crate::CategoryId;

mod capabilities;
pub(crate) mod descriptors;
mod object;
mod reference;
mod types;
mod workbench;

pub use capabilities::{
    AssignObjectIdentity, Create, Links, MediaTyped, ObjectIdentity, Source, SourceComponents,
    Structure, ToXml, XmlConversion,
};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::SubObjectDescriptor;
pub(crate) use object::ErasedProperties;
pub use object::{ErasedObject, ObjectSnapshot};
pub use reference::{AdvertisedObjectReference, ObjectRef, ObjectReferences};
pub use types::*;
pub use workbench::{
    AbapLanguageVersion, GlobalWorkbenchType, InvalidWorkbenchType, ObjectVersion,
};

/// Statically identified ADT object type.
///
/// A resource is considered an object if it has its own set of properties
/// and a global workbench type to address it - for example `CLAS/OC`.
///
/// Because a class definitions include does not have its own properties,
/// it is not considered an object type. Consequently, a function module
/// (`FUGR/FF`), which has properties of its own despite being bound to
/// some primary parent object, is a valid object type.
pub trait ObjectType: private::Sealed + Send + Sync + Sized + 'static {
    /// The complete properties payload loaded for this object family.
    type Properties: Clone + XmlConversion + MediaTyped + Links + ObjectIdentity + 'static;

    /// The object's global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;
}

/// A primary ADT object that does not logically belong to another
/// object. Subsequently, it is also an object that is directly advertised
/// as a collection in the system discovery, identified by a category.
pub trait PrimaryObjectType: ObjectType + private::PrimaryMetadata {
    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;
}

/// Declares that an object has sub-objects of type `C`
pub trait SubObjects<C: ObjectType>: PrimaryObjectType {}

pub(crate) mod private {
    use super::SubObjectDescriptor;

    pub trait Sealed {}

    /// Private split that adds the sub-objects such that it
    /// is not exposed through the public API.
    pub trait PrimaryMetadata {
        const SUBOBJECTS: &'static [SubObjectDescriptor];
    }
}
