use std::{any::Any, fmt, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize};

use super::{MediaTyped, ObjectIdentity, ObjectRef, ObjectType};
use crate::{EntityTag, ObjectError, compatibility::matching_media_type};

pub(crate) type ErasedProperties = Arc<dyn Any + Send + Sync>;

/// A loaded ADT object with its properties, media type, and entity tag.
///
/// Unlike [`ObjectRef<T>`], this value includes the object properties returned
/// by ADT. The type parameter `T` selects the property type and the operations
/// available for that object family.
///
/// Some operations use links advertised by the loaded properties. Operations
/// that only need the object identity can use [`Object::reference`].
#[derive(Debug, Serialize)]
#[serde(bound(serialize = "T::Properties: Serialize"))]
pub struct Object<T: ObjectType> {
    reference: ObjectRef<T>,
    media_type: String,
    etag: Option<EntityTag>,
    properties: T::Properties,
}

impl<T: ObjectType> Object<T> {
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

    /// Returns the immutable properties in this loaded representation.
    pub fn properties(&self) -> &T::Properties {
        &self.properties
    }

    /// Removes the static object family while retaining its concrete properties internally.
    pub fn try_into_erased(self) -> Result<ErasedObject, ObjectError> {
        validate_typed_object::<T>(&self.reference, &self.media_type, &self.properties)?;
        Ok(ErasedObject::new(
            self.reference.erase(),
            self.media_type,
            self.etag,
            Arc::new(self.properties),
        ))
    }
}

impl<T> Clone for Object<T>
where
    T: ObjectType,
    T::Properties: Clone,
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

impl<T: ObjectType> ObjectIdentity for Object<T> {
    fn object_name(&self) -> &str {
        self.reference().object_name()
    }

    fn object_type(&self) -> &super::GlobalWorkbenchType {
        self.reference().object_type()
    }
}

/// A loaded ADT object whose concrete family is known only at runtime.
///
/// This is the loaded counterpart to [`ObjectRef<()>`]. It is useful when the
/// object family comes from user input or a repository response.
///
/// Supported object families are handled through an internal descriptor.
/// Operations check this descriptor and the loaded properties at runtime.
///
/// The concrete properties remain type-erased internally. Runtime consumers can
/// export them as JSON and supply edited JSON to a property-update operation.
pub struct ErasedObject {
    reference: ObjectRef<()>,
    media_type: String,
    etag: Option<EntityTag>,
    properties: ErasedProperties,
}

impl ErasedObject {
    pub(crate) fn new(
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

    /// Returns the erased reference identifying this loaded object.
    pub fn reference(&self) -> &ObjectRef<()> {
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

    /// Exports the concrete properties through their runtime JSON representation.
    pub fn properties(&self) -> Result<serde_json::Value, ObjectError> {
        self.reference
            .require_descriptor()?
            .properties_to_json(&self.reference, &self.properties)
    }

    /// Restores a concrete loaded object after validating its runtime representation.
    pub fn try_into_typed<T>(self) -> Result<Object<T>, ObjectError>
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
        let media_type = validate_typed_object::<T>(&reference, &self.media_type, &properties)?;
        Ok(Object::new(reference, media_type, self.etag, properties))
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

impl Clone for ErasedObject {
    fn clone(&self) -> Self {
        Self::new(
            self.reference.clone(),
            self.media_type.clone(),
            self.etag.clone(),
            self.properties.clone(),
        )
    }
}

impl ObjectIdentity for ErasedObject {
    fn object_name(&self) -> &str {
        self.reference().object_name()
    }

    fn object_type(&self) -> &super::GlobalWorkbenchType {
        self.reference().object_type()
    }
}

impl fmt::Debug for ErasedObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErasedObject")
            .field("reference", &self.reference)
            .field("media_type", &self.media_type)
            .field("etag", &self.etag)
            .field("properties", &"<type-erased>")
            .finish()
    }
}

/// Temporary Serde value used while deserializing an [`Object`].
///
/// The reference, media type, and properties are checked before the object is
/// constructed.
#[derive(Deserialize)]
#[serde(bound(deserialize = "ObjectRef<T>: Deserialize<'de>, T::Properties: Deserialize<'de>"))]
struct RawObject<T: ObjectType> {
    reference: ObjectRef<T>,
    media_type: String,
    etag: Option<EntityTag>,
    properties: T::Properties,
}

impl<'de, T> Deserialize<'de> for Object<T>
where
    T: ObjectType,
    ObjectRef<T>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let object = RawObject::<T>::deserialize(deserializer)?;
        let media_type =
            validate_typed_object::<T>(&object.reference, &object.media_type, &object.properties)
                .map_err(serde::de::Error::custom)?;
        Ok(Self::new(
            object.reference,
            media_type,
            object.etag,
            object.properties,
        ))
    }
}

pub(crate) fn validate_typed_object<T>(
    reference: &ObjectRef<T>,
    media_type: &str,
    properties: &T::Properties,
) -> Result<&'static str, ObjectError>
where
    T: ObjectType,
{
    let media_type =
        matching_media_type(T::Properties::MEDIA_TYPES, media_type).ok_or_else(|| {
            ObjectError::UnsupportedPropertiesMediaType {
                object_type: T::WORKBENCH_TYPE,
                media_type: media_type.to_owned(),
            }
        })?;
    properties.validate_for(reference)?;
    Ok(media_type)
}
