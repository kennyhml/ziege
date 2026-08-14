use super::{ObjectRef, ObjectType};
use crate::{
    api::properties::RawObjectProperties,
    compatibility::MediaVersionNegotiation,
    error::{ObjectError, ResponseError},
    target::TemplateTarget,
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

/// Marks an object as having a readable primary source resource.
///
/// Usually, that source lives at `source/main`. Classes have multiple
/// source components, but that is not covered by a marker trait capability
/// as it is the exception.
pub trait HasSource: ObjectType {
    /// The primary source path relative to the object resource.
    const SOURCE_PATH: &'static [&'static str] = &["source", "main"];
}

/// Marks an object as having readable typed properties.
///
/// This attaches the typed properties query and its media-version negotiation
/// to the object family. Property updates are modeled separately by
/// [`UpdateProperties`].
#[doc(hidden)]
pub trait ReadProperties: ObjectType {
    type MediaVersion: MediaVersionNegotiation;
    type Properties: std::fmt::Debug
        + TryFrom<RawObjectProperties<Self>, Error = ResponseError>
        + serde::Serialize
        + Send
        + Sync;
}

/// Marks an object's properties as being updateable. The same payload
/// is used for updating as is returned by the initial object query.
///
/// Note that this makes no promises about which fields on the properties
/// can actually be changed effectively. ADT will simply disregard changes
/// to fields that do not support modification through this API.
///
/// For this to work, we must serialize the internal properties structure
/// back into the correct XML format. Because of how XML serialization
/// behaves with namespaces, this requires the associated `Properties`
/// model to provide a method that serializes it, as that model has
/// the context over the namespaces involved.
#[doc(hidden)]
pub trait UpdateProperties: ReadProperties
where
    Self::Properties: WritableProperties<Self>,
{
}

/// This trait is applied on the property model, not the object type.
///
/// See [`UpdateProperties`] for context.
#[doc(hidden)]
pub trait WritableProperties<T>
where
    T: ReadProperties<Properties = Self>,
    Self: Sized,
{
    fn media_version(&self) -> T::MediaVersion;

    fn to_xml(&self, resource: &ObjectRef<T>) -> Result<String, ObjectError>;
}
