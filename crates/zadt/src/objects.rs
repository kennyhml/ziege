use serde::{Serialize, de::DeserializeOwned};

use crate::{CategoryId, ObjectError, resource::AdvertisedLink};

mod capabilities;
pub(crate) mod descriptors;
mod reference;
mod snapshot;
mod types;
mod workbench;

pub use capabilities::{Create, Source, SourceComponents, Structure};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::SubObjectDescriptor;
pub use reference::{AdvertisedObjectReference, ObjectRef, ObjectReferences};
pub(crate) use snapshot::ErasedProperties;
pub use snapshot::ObjectSnapshot;
pub use types::*;
pub use workbench::{
    AbapLanguageVersion, GlobalWorkbenchType, InvalidWorkbenchType, WorkbenchVersion,
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
    type Properties: Clone + XmlConversion + MediaTyped + Links + AssignObjectIdentity + 'static;

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

/// An XML payload and the namespaces required to encode it through Serde.
pub trait ToXml: Serialize {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[];

    fn to_xml(&self) -> Result<Vec<u8>, ObjectError> {
        Self::XML_NAMESPACES
            .iter()
            .fold(
                serde_xml_rs::SerdeXml::new(),
                |serializer, &(prefix, namespace)| serializer.namespace(prefix, namespace),
            )
            .to_string(&self)
            .map(String::into_bytes)
            .map_err(ObjectError::InvalidRequest)
    }
}

/// An XML payload that supports both owned deserialization and serialization.
pub trait XmlConversion: ToXml + DeserializeOwned + Send + Sync {
    fn from_xml(body: &[u8]) -> Result<Self, ObjectError> {
        serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)
    }
}

impl<T> XmlConversion for T where T: ToXml + DeserializeOwned + Send + Sync {}

/// The ordered media types supported for one complete properties payload.
pub trait MediaTyped {
    /// Supported media types in client preference order.
    const MEDIA_TYPES: &'static [&'static str];
}

/// A properties representation containing advertised links.
pub trait Links {
    /// Returns the links in wire order.
    fn links(&self) -> &[AdvertisedLink];
}

/// Identity embedded in an object payload.
#[doc(hidden)]
pub trait ObjectIdentity {
    fn object_name(&self) -> &str;

    fn object_type(&self) -> &GlobalWorkbenchType;

    fn validate_for(&self, expected: &impl ObjectIdentity) -> Result<(), ObjectError> {
        if self.object_type() != expected.object_type() {
            return Err(ObjectError::UnexpectedObjectType {
                expected: expected.object_type().clone(),
                actual: self.object_type().clone(),
            });
        }
        if self.object_name() != expected.object_name() {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: format!("{} ({})", expected.object_name(), expected.object_type()),
                actual: format!("{} ({})", self.object_name(), self.object_type()),
            });
        }
        Ok(())
    }
}

/// An object payload whose identity is assigned from its target reference.
#[doc(hidden)]
pub trait AssignObjectIdentity: ObjectIdentity {
    fn assign_identity(&mut self, identity: &impl ObjectIdentity);
}

/// Selects the property storage used by an [`ObjectSnapshot`].
///
/// This is an implementation detail that allows statically typed snapshots to
/// retain `T::Properties` while [`ObjectSnapshot<()>`] stores properties behind
/// the runtime object descriptor.
#[doc(hidden)]
pub trait SnapshotKind: private::SnapshotKindSealed + Send + Sync + Sized + 'static {
    type StoredProperties: Clone + Send + Sync + 'static;
}

impl<T: ObjectType> SnapshotKind for T {
    type StoredProperties = T::Properties;
}

impl SnapshotKind for () {
    type StoredProperties = ErasedProperties;
}

pub(crate) mod private {
    use super::{ObjectType, SubObjectDescriptor};

    pub trait Sealed {}

    pub trait SnapshotKindSealed {}

    impl<T: ObjectType> SnapshotKindSealed for T {}

    impl SnapshotKindSealed for () {}

    /// Private split that adds the sub-objects such that it
    /// is not exposed through the public API.
    pub trait PrimaryMetadata {
        const SUBOBJECTS: &'static [SubObjectDescriptor];
    }
}
