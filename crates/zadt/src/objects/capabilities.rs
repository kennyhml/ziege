use super::{AssignObjectIdentity, MediaTyped, ObjectType, ToXml};
use crate::{CategoryId, MediaTypes};

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
    pub(crate) category: CategoryId,
    pub(crate) relation: &'static str,
    pub(crate) name_variable: &'static str,
}

impl RunCapability {
    pub(crate) const fn new(
        category: CategoryId,
        relation: &'static str,
        name_variable: &'static str,
    ) -> Self {
        Self {
            category,
            relation,
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

/// An object family that can be created through its collection resource.
pub trait Create: ObjectType {
    /// The sparse XML payload accepted during creation.
    type Payload: AssignObjectIdentity + Clone + ToXml + Send + Sync;

    /// Creation media types in client preference order.
    ///
    /// By default, sparse creation payloads support the same representations as
    /// the complete object properties.
    const CREATE_MEDIA_TYPES: MediaTypes = <Self::Properties as MediaTyped>::MEDIA_TYPES;
}
