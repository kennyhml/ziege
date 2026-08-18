use serde::{Serialize, de::DeserializeOwned};

use super::{GlobalWorkbenchType, ObjectRef, ObjectType};
use crate::{
    ObjectError, ResponseError, compatibility::media_types_match, target::TemplateTarget,
    vocabulary::CategoryId,
};

/// Marks an object capable of being executed immediately (not a job).
///
/// This uses the ADT program / class run operations and is not to be
/// confused with the traditional execution of objects through the SAP
/// GUI, which supports user interface rendering and so on.
///
/// The ADT run only works for classes implementing `IF_ADT_CLASSRUN`
/// or reports using selection lists that are then exported into plain
/// text.
pub(crate) trait ImmediateRun: ObjectType {
    const RUN: RunCapability;
}

/// Discovery metadata for one immediate plain-text object-run operation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RunCapability {
    pub(crate) target: TemplateTarget,
    pub(crate) name_variable: &'static str,
}

impl RunCapability {
    pub(crate) const fn new(
        category: CategoryId,
        relation: &'static str,
        name_variable: &'static str,
    ) -> Self {
        Self {
            target: TemplateTarget::new(category, relation),
            name_variable,
        }
    }
}

/// An object with a readable primary source resource.
///
/// Usually, that source lives at `source/main`. Classes have multiple
/// source components, but that is the exception.
pub trait Source: ObjectType {
    #[doc(hidden)]
    fn source_uri(properties: &Self::Properties) -> Option<&str>;
}

/// An object with source components advertised by its loaded properties.
pub trait SourceComponents: Source {
    #[doc(hidden)]
    fn source_component_uri<'a>(properties: &'a Self::Properties, name: &str) -> Option<&'a str>;
}

/// A serializable ADT properties payload and its media-version vocabulary.
#[doc(hidden)]
pub trait PropertyModel: std::fmt::Debug + DeserializeOwned + Serialize + Send + Sync {
    type Version: Copy + Eq + Send + Sync + 'static;

    const SUPPORTED_VERSIONS: &'static [Self::Version];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[];

    /// Gets the media type of the provided object version
    fn media_type(version: Self::Version) -> &'static str;

    /// The name of the object these properties belong to
    fn object_name(&self) -> &str;

    /// The workbench object type these properties belong to
    fn object_type(&self) -> &GlobalWorkbenchType;

    /// Verifies whether the properties belong to the given object.
    ///
    /// This is to prevent accidents where one objects properties are written to another.
    fn belongs_to<T>(&self, reference: &ObjectRef<T>) -> bool {
        self.object_name() == reference.name() && self.object_type() == reference.object_type()
    }

    /// Deserializes properties and verifies that they belong to the given object.
    fn from_xml_for<T>(body: &[u8], reference: &ObjectRef<T>) -> Result<Self, ObjectError> {
        let properties: Self =
            serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)?;
        if !properties.belongs_to(reference) {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: reference.to_string(),
                actual: format!(
                    "{} ({})",
                    properties.object_name(),
                    properties.object_type()
                ),
            });
        }
        Ok(properties)
    }

    /// A helper function that serializes the payload while verifying that the properties
    /// truly belong to the given object.
    ///
    /// The serialization also takes into account the xml namespaces used for the properties
    /// of this object type.
    fn to_xml_for<T>(&self, reference: &ObjectRef<T>) -> Result<Vec<u8>, ObjectError> {
        if !self.belongs_to(reference) {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: reference.to_string(),
                actual: format!("{} ({})", self.object_name(), self.object_type()),
            });
        }
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

    fn version_from_media_type(media_type: &str) -> Option<Self::Version> {
        Self::SUPPORTED_VERSIONS
            .iter()
            .copied()
            .find(|version| media_types_match(Self::media_type(*version), media_type))
    }

    /// Resolves a supported media version or reports the complete expected contract.
    fn require_version_from_media_type(
        media_type: &str,
        category: CategoryId,
    ) -> Result<Self::Version, ResponseError> {
        Self::version_from_media_type(media_type).ok_or_else(|| {
            ResponseError::UnsupportedContentType {
                category,
                content_type: media_type.to_owned(),
                supported: Self::SUPPORTED_VERSIONS
                    .iter()
                    .map(|version| Self::media_type(*version).to_owned())
                    .collect(),
            }
        })
    }
}

/// A creation payload whose object identity is supplied by its reference.
#[doc(hidden)]
pub trait CreationPropertyModel: PropertyModel {
    fn set_identity<T>(&mut self, reference: &ObjectRef<T>);
}

/// An object family that can be created through its collection resource.
pub trait Create: ObjectType {
    /// The sparse properties representation accepted during creation.
    type CreateProperties: CreationPropertyModel;

    /// The media version used to serialize the creation payload.
    const CREATE_VERSION: <Self::CreateProperties as PropertyModel>::Version;

    #[doc(hidden)]
    fn creation_properties_to_xml(
        reference: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError> {
        let mut properties: Self::CreateProperties =
            serde_json::from_value(properties).map_err(ObjectError::InvalidPropertiesJson)?;
        properties.set_identity(reference);
        properties.to_xml_for(reference)
    }
}

/// Marks an object's properties as being updateable. The same payload
/// is used for updating as is returned by the initial object query.
///
/// Note that this makes no promises about which fields on the properties
/// can actually be changed effectively. ADT will simply disregard changes
/// to fields that do not support modification through this API.
#[doc(hidden)]
pub trait UpdateProperties: ObjectType {
    #[doc(hidden)]
    fn properties_to_xml(
        object: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError> {
        let properties: Self::Properties =
            serde_json::from_value(properties).map_err(ObjectError::InvalidPropertiesJson)?;
        properties.to_xml_for(object)
    }
}
