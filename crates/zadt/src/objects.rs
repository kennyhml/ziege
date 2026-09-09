use serde::{Serialize, de::DeserializeOwned};

use crate::{AdtUri, CategoryId, Discovery, MediaTypes, ObjectError, ResolveError, ResourceView};

mod capabilities;
pub(crate) mod descriptors;
mod key;
mod name;
mod reference;
mod snapshot;
mod types;
mod workbench;

pub use capabilities::{Create, Source, SourceComponents, Structure};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub use descriptors::SubObjectDescriptor;
pub use key::ObjectKey;
pub use name::{InvalidObjectName, ObjectName};
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
    type Properties: XmlCodec + Identity + Resources + 'static + Clone;

    /// Supported XML properties media types, in client preference order.
    const MEDIA_TYPES: MediaTypes;

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
pub trait SubObject<C: ObjectType>: PrimaryObjectType {
    /// Runtime metadata for this parent-child relationship.
    #[doc(hidden)]
    const DESCRIPTOR: SubObjectDescriptor;
}

/// An operation target that either needs discovery or already has a location.
///
/// This is internal operation machinery, though it may be useful to expose
/// it if we settle on letting consumers implement their own operations.
///
/// This effectively allows operations to be pseudo generic over the state
/// of the object identity. If the object is alreade resolved ([`ObjectRef<T>`])
/// no lookup is needed. [`ObjectKey<T>`] can be resolved against the discovery
/// at the time the uri is required.
///
/// Both logical keys and located references convert directly into this target.
///
/// Some operations only require an [`ObjectKey<T>`], such as object creation.
#[derive(Debug)]
pub(crate) enum ObjectTarget<T = ()> {
    Logical(ObjectKey<T>),
    Located(ObjectRef<T>),
}

impl<T> ObjectTarget<T> {
    /// Normalizes and returns the underlying [`ObjectKey<T>`].
    pub(crate) fn key(&self) -> &ObjectKey<T> {
        match self {
            Self::Logical(key) => key,
            Self::Located(reference) => reference.key(),
        }
    }

    /// Resolves the [`AdtUri`] of this object against the discovery.
    ///
    /// If the object is alreade a reference with a uri, a clone is returnd.
    pub(crate) fn resolve_uri(&self, discovery: &Discovery) -> Result<AdtUri, ResolveError> {
        match self {
            Self::Logical(key) => discovery.resolve_object_uri(key),
            Self::Located(reference) => Ok(reference.uri().clone()),
        }
    }

    /// Resolves the [`ObjectRef<T>`] of this object from the discovery.
    ///
    /// If the object is alreade a reference, a clone is returnd.
    pub(crate) fn resolve(&self, discovery: &Discovery) -> Result<ObjectRef<T>, ResolveError> {
        match self {
            Self::Logical(key) => discovery.resolve_object(key),
            Self::Located(reference) => Ok(reference.clone()),
        }
    }

    /// Attaches the response location without discarding known parent metadata.
    pub(crate) fn at(&self, uri: AdtUri) -> ObjectRef<T> {
        match self {
            Self::Logical(key) => ObjectRef::new(key.clone(), uri),
            Self::Located(reference) => {
                let located = ObjectRef::new(reference.key().clone(), uri);
                match reference.parent_uri() {
                    Some(parent_uri) => located.with_parent_uri(parent_uri.clone()),
                    None => located,
                }
            }
        }
    }
}

impl<T> From<ObjectKey<T>> for ObjectTarget<T> {
    fn from(key: ObjectKey<T>) -> Self {
        Self::Logical(key)
    }
}

impl<T> From<ObjectRef<T>> for ObjectTarget<T> {
    fn from(reference: ObjectRef<T>) -> Self {
        Self::Located(reference)
    }
}

impl<T> Clone for ObjectTarget<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Logical(key) => Self::Logical(key.clone()),
            Self::Located(reference) => Self::Located(reference.clone()),
        }
    }
}

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
pub trait XmlCodec: ToXml + DeserializeOwned + Send + Sync {
    fn from_xml(body: &[u8]) -> Result<Self, ObjectError> {
        serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)
    }
}

impl<T> XmlCodec for T where T: ToXml + DeserializeOwned + Send + Sync {}

/// Resources advertised by an object's properties, without a bound object URI.
pub trait Resources {
    /// Borrows resource metadata; snapshots bind the view to their own URI.
    fn resources(&self) -> ResourceView<'_>;
}

/// Identity embedded in an object payload.
#[doc(hidden)]
pub trait Identity {
    fn object_name(&self) -> &str;

    fn workbench_type(&self) -> &GlobalWorkbenchType;

    /// The raw container advertised in a properties payload, when modeled.
    fn container(&self) -> Option<&AdvertisedObjectReference> {
        None
    }

    fn validate_for(&self, expected: &impl Identity) -> Result<(), ObjectError> {
        if self.workbench_type() != expected.workbench_type() {
            return Err(ObjectError::UnexpectedObjectType {
                expected: expected.workbench_type().clone(),
                actual: self.workbench_type().clone(),
            });
        }
        if self.object_name() != expected.object_name() {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: format!("{} ({})", expected.object_name(), expected.workbench_type()),
                actual: format!("{} ({})", self.object_name(), self.workbench_type()),
            });
        }
        Ok(())
    }
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
