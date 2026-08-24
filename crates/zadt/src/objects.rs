use crate::CategoryId;

mod capabilities;
pub(crate) mod descriptors;
mod object;
mod reference;
mod types;
mod workbench;

pub use capabilities::{
    Create, CreationPropertyModel, PropertyModel, Source, SourceComponents, Structure,
};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::ObjectTypeDescriptor;
pub(crate) use descriptors::RuntimeObjectTypeDescriptor;
pub use object::{AnyObject, Object};
pub use reference::{AdvertisedObjectReference, ObjectRef, ObjectReferences};
pub use types::*;
pub use workbench::{
    AbapLanguageVersion, GlobalWorkbenchType, InvalidWorkbenchType, ObjectVersion,
};

pub(crate) mod private {
    use super::SubObjectDescriptor;

    pub trait Sealed {}

    pub trait PrimaryMetadata {
        const SUBOBJECTS: &'static [SubObjectDescriptor];
    }
}

/// Statically identified ADT object resource family.
pub trait ObjectType: private::Sealed + Send + Sync + Sized + 'static {
    /// The complete properties payload loaded for this object family.
    type Properties: PropertyModel;

    /// The object's global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;
}

/// An object family addressed directly through an ADT discovery collection.
pub trait PrimaryObjectType: ObjectType + private::PrimaryMetadata {
    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;
}

/// Declares that a primary object supports children of type `C`.
pub trait SubObjects<C: ObjectType>: PrimaryObjectType {}

/// Runtime metadata for one statically declared parent-child relationship.
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct SubObjectDescriptor {
    object_type: GlobalWorkbenchType,
    relation: &'static str,
    parent_variable: &'static str,
}

impl SubObjectDescriptor {
    pub(crate) const fn new(
        object_type: GlobalWorkbenchType,
        relation: &'static str,
        parent_variable: &'static str,
    ) -> Self {
        Self {
            object_type,
            relation,
            parent_variable,
        }
    }

    pub(crate) fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }

    pub(crate) const fn relation(&self) -> &'static str {
        self.relation
    }

    pub(crate) const fn parent_variable(&self) -> &'static str {
        self.parent_variable
    }
}
