use serde::{Serialize, de::DeserializeOwned};

use super::{GlobalWorkbenchType, ObjectRef, ObjectType};
use crate::{
    ObjectError, compatibility::media_types_match, target::TemplateTarget, vocabulary::CategoryId,
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

    fn media_type(version: Self::Version) -> &'static str;

    fn object_name(&self) -> &str;

    fn object_type(&self) -> &GlobalWorkbenchType;

    fn version_from_media_type(media_type: &str) -> Option<Self::Version> {
        Self::SUPPORTED_VERSIONS
            .iter()
            .copied()
            .find(|version| media_types_match(Self::media_type(*version), media_type))
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
    ) -> Result<String, ObjectError> {
        let properties: Self::Properties =
            serde_json::from_value(properties).map_err(ObjectError::InvalidPropertiesJson)?;
        if properties.object_name() != object.name() {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: object.to_string(),
                actual: format!(
                    "{} ({})",
                    properties.object_name(),
                    properties.object_type()
                ),
            });
        }
        if properties.object_type() != object.object_type() {
            return Err(ObjectError::UnexpectedObjectType {
                expected: object.object_type().clone(),
                actual: properties.object_type().clone(),
            });
        }
        Self::Properties::XML_NAMESPACES
            .iter()
            .fold(
                serde_xml_rs::SerdeXml::new(),
                |serializer, &(prefix, namespace)| serializer.namespace(prefix, namespace),
            )
            .to_string(&properties)
            .map_err(ObjectError::InvalidRequest)
    }
}
