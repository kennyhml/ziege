use std::{any::Any, fmt, sync::Arc};

use super::{ObjectIdentity, ObjectRef, ObjectType, SnapshotKind, WorkbenchVersion};
use crate::{EntityTag, ObjectError};

pub(crate) type ErasedProperties = Arc<dyn Any + Send + Sync>;

/// An immutable snapshot of a loaded ADT object representation.
///
/// Unlike [`ObjectRef<T>`], this value includes the Workbench version and object
/// properties returned by ADT. The type parameter `T` selects the property type
/// and the operations available for that object family. [`ObjectSnapshot<()>`]
/// stores the object family and its concrete properties at runtime.
///
/// The runtime form is the loaded counterpart to [`ObjectRef<()>`]. It is useful
/// when the object family comes from user input or a repository response.
/// Supported object families are handled through an internal descriptor, and
/// operations check that descriptor and the loaded properties at runtime.
///
/// Runtime properties remain type-erased internally. Consumers can export them
/// as JSON and supply edited JSON to a property-update operation.
///
/// Some operations use links advertised by the loaded properties. Operations
/// that only need the object identity can use [`ObjectSnapshot::reference`].
pub struct ObjectSnapshot<T: SnapshotKind = ()> {
    reference: ObjectRef<T>,
    workbench_version: WorkbenchVersion,
    media_type: String,
    etag: Option<EntityTag>,
    properties: T::StoredProperties,
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Returns the reference identifying this snapshot.
    pub fn reference(&self) -> &ObjectRef<T> {
        &self.reference
    }

    /// Returns the Workbench version represented by this snapshot.
    pub fn workbench_version(&self) -> WorkbenchVersion {
        self.workbench_version
    }

    /// Returns the media type of this snapshot.
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// Returns the entity tag associated with this snapshot.
    pub fn etag(&self) -> Option<&EntityTag> {
        self.etag.as_ref()
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Creates a new snapshot for an internally parsed query result.
    pub(crate) fn new(
        reference: ObjectRef<T>,
        workbench_version: WorkbenchVersion,
        media_type: impl Into<String>,
        etag: Option<EntityTag>,
        properties: T::Properties,
    ) -> Self {
        Self {
            reference,
            workbench_version,
            media_type: media_type.into(),
            etag,
            properties,
        }
    }

    /// Returns the immutable properties in this snapshot.
    pub fn properties(&self) -> &T::Properties {
        &self.properties
    }

    /// Erases the concrete object type of this snapshot.
    ///
    /// All data is retained and properties move into type-erased storage.
    pub fn into_erased(self) -> ObjectSnapshot<()> {
        ObjectSnapshot::<()>::new_erased(
            self.reference.erase(),
            self.workbench_version,
            self.media_type,
            self.etag,
            Arc::new(self.properties),
        )
    }
}

impl<T> Clone for ObjectSnapshot<T>
where
    T: SnapshotKind,
{
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            workbench_version: self.workbench_version,
            media_type: self.media_type.clone(),
            etag: self.etag.clone(),
            properties: self.properties.clone(),
        }
    }
}

impl<T: SnapshotKind> ObjectIdentity for ObjectSnapshot<T> {
    fn object_name(&self) -> &str {
        self.reference().object_name()
    }

    fn object_type(&self) -> &super::GlobalWorkbenchType {
        self.reference().object_type()
    }
}

impl ObjectSnapshot<()> {
    /// Constructs a snapshot with properties retained behind its runtime descriptor.
    pub(crate) fn new_erased(
        reference: ObjectRef<()>,
        workbench_version: WorkbenchVersion,
        media_type: impl Into<String>,
        etag: Option<EntityTag>,
        properties: ErasedProperties,
    ) -> Self {
        Self {
            reference,
            workbench_version,
            media_type: media_type.into(),
            etag,
            properties,
        }
    }

    /// Exports the concrete properties through their runtime JSON representation.
    ///
    /// Private wire metadata is retained in the JSON representation, but is
    /// canonicalized from this snapshot when constructing an update.
    pub fn properties(&self) -> Result<serde_json::Value, ObjectError> {
        self.reference
            .require_descriptor()?
            .properties_to_json(&self.reference, &self.properties)
    }

    /// Restores a concrete loaded object after validating its runtime type.
    pub fn try_into_typed<T>(self) -> Result<ObjectSnapshot<T>, ObjectError>
    where
        T: ObjectType,
    {
        let reference = self.typed_reference::<T>()?;

        // If we could recover the reference from `T` then the property type matches too.
        // Cannot use `typed_properties` here because we actually need the reference counter.
        let properties = self
            .properties
            .downcast::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type");

        let properties = match Arc::try_unwrap(properties) {
            Ok(properties) => properties,
            Err(properties) => properties.as_ref().clone(),
        };

        Ok(ObjectSnapshot::new(
            reference,
            self.workbench_version,
            self.media_type,
            self.etag,
            properties,
        ))
    }

    /// Returns a type tagged reference to the underlying object.
    pub(crate) fn typed_reference<T: ObjectType>(&self) -> Result<ObjectRef<T>, ObjectError> {
        self.reference
            .typed::<T>()
            .ok_or_else(|| ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: self.reference.object_type().clone(),
            })
    }

    /// Casts the contained object properties to the property type of `T`
    ///
    /// This is an internal helper and panics when `T` does not match.
    pub(crate) fn typed_properties<T: ObjectType>(&self) -> &T::Properties {
        self.properties
            .downcast_ref::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type")
    }
}

impl fmt::Debug for ObjectSnapshot<()> {
    // Custom debug implementation to ignore the erased properties
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectSnapshot")
            .field("reference", &self.reference)
            .field("workbench_version", &self.workbench_version)
            .field("media_type", &self.media_type)
            .field("etag", &self.etag)
            .field("properties", &"<type-erased>")
            .finish()
    }
}

impl<T> fmt::Debug for ObjectSnapshot<T>
where
    T: ObjectType,
    ObjectRef<T>: fmt::Debug,
    T::Properties: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectSnapshot")
            .field("reference", &self.reference)
            .field("workbench_version", &self.workbench_version)
            .field("media_type", &self.media_type)
            .field("etag", &self.etag)
            .field("properties", &self.properties)
            .finish()
    }
}
