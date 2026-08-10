use super::{ObjectRef, ObjectType};
use crate::{compatibility::MediaVersionNegotiation, error::ResponseError, protocol::EntityTag};

/// A statically known source component exposed by an object family.
pub trait SourceComponent: Sync {
    /// Returns the component name used by ADT.
    fn name(&self) -> &'static str;

    /// Returns the component path relative to its owning object.
    fn path(&self) -> &'static [&'static str];
}

/// A closed set of source components exposed by an object family.
#[doc(hidden)]
pub trait SourceComponentSet: SourceComponent + Sized + 'static {
    const COMPONENTS: &'static [&'static dyn SourceComponent];
}

/// Annotates an object type that supports fetching and decoding properties.
#[doc(hidden)]
pub trait ObjectProperties: ObjectType {
    type MediaVersion: MediaVersionNegotiation;
    type Properties: serde::Serialize + Send;

    fn parse(
        resource: &ObjectRef<Self>,
        version: Self::MediaVersion,
        body: &[u8],
        etag: Option<EntityTag>,
    ) -> Result<Self::Properties, ResponseError>;
}

/// An object type with a conventional primary source resource.
pub trait Source: ObjectType {
    /// The primary source path relative to the object resource.
    const SOURCE_PATH: &'static [&'static str] = &["source", "main"];
}

/// An object type with secondary source components in addition to its primary source.
pub trait SourceComponents: Source {
    /// The statically known secondary source component type.
    type Component: SourceComponentSet;
}
