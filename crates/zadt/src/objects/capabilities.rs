use serde::{Serialize, de::DeserializeOwned};

use super::ObjectType;
use crate::{compatibility::media_types_match, target::TemplateTarget, vocabulary::CategoryId};

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

/// A serializable ADT properties payload and its media-version vocabulary.
#[doc(hidden)]
pub trait PropertyModel: std::fmt::Debug + DeserializeOwned + Serialize + Send + Sync {
    type Version: Copy + Eq + Send + Sync + 'static;

    const SUPPORTED_VERSIONS: &'static [Self::Version];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[];

    fn media_type(version: Self::Version) -> &'static str;

    fn version_from_media_type(media_type: &str) -> Option<Self::Version> {
        Self::SUPPORTED_VERSIONS
            .iter()
            .copied()
            .find(|version| media_types_match(Self::media_type(*version), media_type))
    }
}

/// Marks an object as having readable typed properties.
#[doc(hidden)]
pub trait ReadProperties: ObjectType {
    type Properties: PropertyModel;
}

/// Marks an object's properties as being updateable. The same payload
/// is used for updating as is returned by the initial object query.
///
/// Note that this makes no promises about which fields on the properties
/// can actually be changed effectively. ADT will simply disregard changes
/// to fields that do not support modification through this API.
#[doc(hidden)]
pub trait UpdateProperties: ReadProperties {}
