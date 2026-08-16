use serde::{Deserialize, Serialize};

use super::{ObjectRef, ObjectType, PropertyModel};
use crate::{EntityTag, ObjectError};

/// A loaded ADT object representation and its transport metadata.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound(serialize = "P: Serialize", deserialize = "P: Deserialize<'de>"))]
pub struct AdtObject<P = serde_json::Value> {
    reference: ObjectRef<()>,
    media_type: String,
    pub etag: Option<EntityTag>,
    pub properties: P,
}

impl<P> AdtObject<P> {
    pub(crate) fn new(
        reference: ObjectRef<()>,
        media_type: impl Into<String>,
        etag: Option<EntityTag>,
        properties: P,
    ) -> Self {
        Self {
            reference,
            media_type: media_type.into(),
            etag,
            properties,
        }
    }

    /// Returns the reference identifying this loaded object.
    pub fn reference(&self) -> &ObjectRef<()> {
        &self.reference
    }

    /// Returns the media type of this representation.
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub(crate) fn into_parts(self) -> (ObjectRef<()>, String, Option<EntityTag>, P) {
        (self.reference, self.media_type, self.etag, self.properties)
    }
}

impl<P> AdtObject<P>
where
    P: PropertyModel,
{
    /// Returns the negotiated media-type version of this representation.
    pub fn media_version(&self) -> P::Version {
        P::version_from_media_type(&self.media_type)
            .expect("typed ADT objects are constructed with a supported media type")
    }
}

impl AdtObject {
    /// Restores a concrete loaded object after validating its runtime representation.
    pub fn try_into_typed<T>(self) -> Result<AdtObject<T::Properties>, ObjectError>
    where
        T: ObjectType,
    {
        if self.reference.object_type() != &T::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: self.reference.object_type().clone(),
            });
        }
        if T::Properties::version_from_media_type(&self.media_type).is_none() {
            return Err(ObjectError::UnsupportedPropertiesMediaType {
                object_type: T::WORKBENCH_TYPE,
                media_type: self.media_type,
            });
        }
        let properties: T::Properties =
            serde_json::from_value(self.properties).map_err(ObjectError::InvalidPropertiesJson)?;
        if properties.object_name() != self.reference.name() {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: self.reference.to_string(),
                actual: format!(
                    "{} ({})",
                    properties.object_name(),
                    properties.object_type()
                ),
            });
        }
        if properties.object_type() != self.reference.object_type() {
            return Err(ObjectError::UnexpectedObjectType {
                expected: self.reference.object_type().clone(),
                actual: properties.object_type().clone(),
            });
        }
        Ok(AdtObject::new(
            self.reference,
            self.media_type,
            self.etag,
            properties,
        ))
    }
}
