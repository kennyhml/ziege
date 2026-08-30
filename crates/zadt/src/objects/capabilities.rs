use serde::{Serialize, de::DeserializeOwned};

use super::{GlobalWorkbenchType, ObjectType, PrimaryObjectType};
use crate::{CategoryId, ObjectError, operation::TemplateTarget, resource::AdvertisedLink};

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

/// An object whose loaded properties can advertise a structural representation.
pub trait Structure: ObjectType {}

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

/// An object family that can be created through its collection resource.
pub trait Create: PrimaryObjectType {
    /// The sparse XML payload accepted during creation.
    type Payload: AssignObjectIdentity + ToXml + Send + Sync;

    /// Creation media types in client preference order.
    ///
    /// Sparse creation payloads default to the preferred complete-properties
    /// representation. Implementations may override this with additional media
    /// types only when the same payload is valid for those representations.
    const CREATE_MEDIA_TYPES: &'static [&'static str] =
        &[<Self::Properties as MediaTyped>::MEDIA_TYPES[0]];
}
