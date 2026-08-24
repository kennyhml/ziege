use serde::{Deserialize, Deserializer, Serialize};

use super::{ObjectRef, ObjectType, PropertyModel};
use crate::{EntityTag, ObjectError};

/// A loaded ADT object with its properties, media type, and entity tag.
///
/// Unlike [`ObjectRef<T>`], this value includes the object properties returned
/// by ADT. The type parameter `T` selects the property model and the operations
/// available for that object family.
///
/// Some operations use links advertised by the loaded properties. Operations
/// that only need the object identity can use [`Object::reference`].
#[derive(Debug, Serialize)]
#[serde(bound(serialize = "T::Properties: Serialize"))]
pub struct Object<T: ObjectType> {
    reference: ObjectRef<T>,
    media_type: String,
    pub etag: Option<EntityTag>,
    pub properties: T::Properties,
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

    pub(crate) fn into_parts(self) -> (ObjectRef<T>, String, Option<EntityTag>, T::Properties) {
        (self.reference, self.media_type, self.etag, self.properties)
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

impl<T> Object<T>
where
    T: ObjectType,
{
    /// Returns the negotiated media-type version of this representation.
    pub fn media_version(&self) -> <T::Properties as PropertyModel>::Version {
        T::Properties::version_from_media_type(&self.media_type)
            .expect("typed ADT objects are constructed with a supported media type")
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
/// Properties are stored as JSON so callers can inspect and edit them without
/// knowing their concrete Rust type.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnyObject {
    reference: ObjectRef<()>,
    media_type: String,
    pub etag: Option<EntityTag>,
    pub properties: serde_json::Value,
}

impl AnyObject {
    pub(crate) fn new(
        reference: ObjectRef<()>,
        media_type: impl Into<String>,
        etag: Option<EntityTag>,
        properties: serde_json::Value,
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
        let properties: T::Properties =
            serde_json::from_value(self.properties).map_err(ObjectError::InvalidPropertiesJson)?;
        validate_typed_object::<T>(&reference, &self.media_type, &properties)?;
        Ok(Object::new(
            reference,
            self.media_type,
            self.etag,
            properties,
        ))
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
        validate_typed_object::<T>(&object.reference, &object.media_type, &object.properties)
            .map_err(serde::de::Error::custom)?;
        Ok(Self::new(
            object.reference,
            object.media_type,
            object.etag,
            object.properties,
        ))
    }
}

pub(crate) fn validate_typed_object<T>(
    reference: &ObjectRef<T>,
    media_type: &str,
    properties: &T::Properties,
) -> Result<(), ObjectError>
where
    T: ObjectType,
{
    if reference.object_type() != &T::WORKBENCH_TYPE {
        return Err(ObjectError::UnexpectedObjectType {
            expected: T::WORKBENCH_TYPE,
            actual: reference.object_type().clone(),
        });
    }
    if T::Properties::version_from_media_type(media_type).is_none() {
        return Err(ObjectError::UnsupportedPropertiesMediaType {
            object_type: T::WORKBENCH_TYPE,
            media_type: media_type.to_owned(),
        });
    }
    if properties.object_type() != reference.object_type() {
        return Err(ObjectError::UnexpectedObjectType {
            expected: reference.object_type().clone(),
            actual: properties.object_type().clone(),
        });
    }
    if !properties.belongs_to(reference) {
        return Err(ObjectError::UnexpectedObjectReference {
            expected: reference.to_string(),
            actual: properties.object_description(),
        });
    }
    Ok(())
}
