use serde::{Serialize, de::DeserializeOwned};

use super::{GlobalWorkbenchType, ObjectRef, ObjectType, PrimaryObjectType};
use crate::{
    CategoryId, ObjectError, compatibility::media_types_match, operation::TemplateTarget,
    resource::AdvertisedLink,
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
/// The source URI is resolved from the object's loaded properties.
pub trait Source: ObjectType {
    #[doc(hidden)]
    fn source_uri(properties: &Self::Properties) -> Option<&str>;
}

/// An object with source components advertised by its loaded properties.
pub trait SourceComponents: Source {
    #[doc(hidden)]
    fn source_component_uri<'a>(properties: &'a Self::Properties, name: &str) -> Option<&'a str>;
}

/// An object whose loaded properties can advertise a structural representation.
pub trait Structure: ObjectType {}

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

    /// Atom links advertised at the root of this properties representation.
    fn links(&self) -> &[AdvertisedLink] {
        &[]
    }

    /// Verifies whether the properties belong to the given object.
    ///
    /// This is to prevent accidents where one objects properties are written to another.
    fn belongs_to<T>(&self, reference: &ObjectRef<T>) -> bool {
        self.object_name() == reference.name() && self.object_type() == reference.object_type()
    }

    /// Formats the payload identity for mismatch diagnostics.
    fn object_description(&self) -> String {
        format!("{} ({})", self.object_name(), self.object_type())
    }

    /// Deserializes properties and verifies that they belong to the given object.
    fn from_xml_for<T>(body: &[u8], reference: &ObjectRef<T>) -> Result<Self, ObjectError> {
        let properties: Self =
            serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)?;
        if !properties.belongs_to(reference) {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: reference.to_string(),
                actual: properties.object_description(),
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
                actual: self.object_description(),
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
}

/// A creation payload whose object identity is supplied by its reference.
#[doc(hidden)]
pub trait CreationPropertyModel: PropertyModel {
    fn set_identity<T>(&mut self, reference: &ObjectRef<T>);
}

/// An object family that can be created through its collection resource.
pub trait Create: PrimaryObjectType {
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
