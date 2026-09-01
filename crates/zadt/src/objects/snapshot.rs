use std::{any::Any, fmt, sync::Arc};

use super::{MediaTyped, ObjectIdentity, ObjectRef, ObjectType, SnapshotKind};
use crate::{EntityTag, ObjectError, compatibility};

pub(crate) type ErasedProperties = Arc<dyn Any + Send + Sync>;

/// An immutable snapshot of a loaded ADT object representation.
///
/// Unlike [`ObjectRef<T>`], this value includes the object properties returned
/// by ADT. The type parameter `T` selects the property type and the operations
/// available for that object family. [`ObjectSnapshot<()>`] stores the object
/// family and its concrete properties at runtime.
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
    media_type: String,
    etag: Option<EntityTag>,
    properties: T::StoredProperties,
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Returns the reference identifying this loaded object.
    pub fn reference(&self) -> &ObjectRef<T> {
        &self.reference
    }

    /// Returns the media type of this representation.
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// Returns the entity tag associated with this representation.
    pub fn etag(&self) -> Option<&EntityTag> {
        self.etag.as_ref()
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    pub(crate) fn new(
        reference: ObjectRef<T>,
        media_type: impl Into<String>,
        etag: Option<EntityTag>,
        properties: T::Properties,
    ) -> Self {
        Self {
            reference,
            media_type: media_type.into(),
            etag,
            properties,
        }
    }

    /// Returns the immutable properties in this loaded representation.
    pub fn properties(&self) -> &T::Properties {
        &self.properties
    }

    /// Removes the static object family while retaining its concrete properties internally.
    pub fn try_into_erased(self) -> Result<ObjectSnapshot<()>, ObjectError> {
        validate_typed_snapshot::<T>(&self.reference, &self.media_type, &self.properties)?;
        Ok(ObjectSnapshot::<()>::new_erased(
            self.reference.erase(),
            self.media_type,
            self.etag,
            Arc::new(self.properties),
        ))
    }
}

impl<T> Clone for ObjectSnapshot<T>
where
    T: SnapshotKind,
{
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
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
        media_type: impl Into<String>,
        etag: Option<EntityTag>,
        properties: ErasedProperties,
    ) -> Self {
        Self {
            reference,
            media_type: media_type.into(),
            etag,
            properties,
        }
    }

    /// Exports the concrete properties through their runtime JSON representation.
    pub fn properties(&self) -> Result<serde_json::Value, ObjectError> {
        self.reference
            .require_descriptor()?
            .properties_to_json(&self.reference, &self.properties)
    }

    /// Restores a concrete loaded object after validating its runtime representation.
    pub fn try_into_typed<T>(self) -> Result<ObjectSnapshot<T>, ObjectError>
    where
        T: ObjectType,
    {
        let reference =
            self.reference
                .typed::<T>()
                .ok_or_else(|| ObjectError::UnexpectedObjectType {
                    expected: T::WORKBENCH_TYPE,
                    actual: self.reference.object_type().clone(),
                })?;
        let properties = self
            .properties
            .downcast::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type");
        let properties = match Arc::try_unwrap(properties) {
            Ok(properties) => properties,
            Err(properties) => properties.as_ref().clone(),
        };
        let media_type = validate_typed_snapshot::<T>(&reference, &self.media_type, &properties)?;
        Ok(ObjectSnapshot::new(
            reference, media_type, self.etag, properties,
        ))
    }

    pub(crate) fn typed_reference<T: ObjectType>(&self) -> Result<ObjectRef<T>, ObjectError> {
        self.reference
            .typed::<T>()
            .ok_or_else(|| ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: self.reference.object_type().clone(),
            })
    }

    pub(crate) fn typed_properties<T: ObjectType>(&self) -> &T::Properties {
        self.properties
            .downcast_ref::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type")
    }
}

impl fmt::Debug for ObjectSnapshot<()> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectSnapshot")
            .field("reference", &self.reference)
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
            .field("media_type", &self.media_type)
            .field("etag", &self.etag)
            .field("properties", &self.properties)
            .finish()
    }
}

pub(crate) fn validate_typed_snapshot<T>(
    reference: &ObjectRef<T>,
    media_type: &str,
    properties: &T::Properties,
) -> Result<&'static str, ObjectError>
where
    T: ObjectType,
{
    let media_type = compatibility::matching_media_type(T::Properties::MEDIA_TYPES, media_type)
        .ok_or_else(|| ObjectError::UnsupportedPropertiesMediaType {
            object_type: T::WORKBENCH_TYPE,
            media_type: media_type.to_owned(),
        })?;
    properties.validate_for(reference)?;
    Ok(media_type)
}
