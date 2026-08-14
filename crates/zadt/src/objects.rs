use std::{fmt, hash::Hash};

use crate::{
    AccessMode, LockRequest, ObjectLock, UnlockRequest,
    api::object::ObjectRun,
    client::{Client, Ready},
    error::ObjectError,
    resource::SourceRef,
    uri::AdtUri,
    vocabulary::CategoryId,
};

mod capabilities;
mod descriptors;
mod families;
mod version;
mod workbench;

pub use capabilities::{HasSource, ReadProperties, UpdateProperties, WritableProperties};
pub(crate) use capabilities::{ImmediateRun, RunCapability};
pub(crate) use descriptors::RuntimeObjectTypeDescriptor;
pub use families::{Class, ClassSourceComponent, DataElement, Include, Package, Program};
pub use version::ObjectVersion;
pub use workbench::{GlobalWorkbenchType, InvalidWorkbenchType};

pub(crate) mod private {
    pub trait Sealed {}
}

/// Statically identified ADT object resource family.
pub trait ObjectType: private::Sealed + Clone + Send + Sync + Sized + 'static {
    /// The object's global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;

    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;

    #[doc(hidden)]
    fn marker() -> Self;
}

/// Runtime type information retained by a type-erased object reference.
#[derive(Clone, Debug)]
pub struct Erased {
    object_type: GlobalWorkbenchType,
    descriptor: Option<&'static dyn RuntimeObjectTypeDescriptor>,
}

impl Erased {
    fn new(object_type: GlobalWorkbenchType) -> Self {
        Self {
            descriptor: descriptors::object_type_descriptor(&object_type),
            object_type,
        }
    }
}

/// A validated ADT repository-object identity with static or runtime type state.
///
/// Typed references obtain capabilities from `T`. [`ObjectRef<Erased>`] retains
/// the exact runtime Workbench type and an optional descriptor for modeled
/// runtime capabilities.
#[derive(Clone, Debug)]
pub struct ObjectRef<T = Erased> {
    name: String,
    uri: AdtUri,
    state: T,
}

impl<T> ObjectRef<T> {
    /// Returns the object's resource URI.
    pub fn uri(&self) -> &AdtUri {
        &self.uri
    }

    /// Returns the object name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T: ObjectType> ObjectRef<T> {
    fn typed_ref(name: String, uri: AdtUri) -> Self {
        Self {
            name,
            uri,
            state: T::marker(),
        }
    }

    pub(crate) fn from_parts(name: String, uri: AdtUri) -> Self {
        Self::typed_ref(name, uri)
    }

    /// Returns a runtime-typed copy of this object identity.
    pub fn erase(&self) -> ObjectRef<Erased> {
        ObjectRef::erased(self.name.clone(), self.uri.clone(), T::WORKBENCH_TYPE)
    }

    /// Returns this reference's statically known Workbench type.
    pub fn object_type(&self) -> GlobalWorkbenchType {
        T::WORKBENCH_TYPE
    }

    pub(crate) fn source_from_path(&self, path: &[&str]) -> SourceRef {
        self.erase().source_from_path(path)
    }
}

impl ObjectRef<Erased> {
    pub(crate) fn erased(name: String, uri: AdtUri, object_type: GlobalWorkbenchType) -> Self {
        Self {
            name,
            uri,
            state: Erased::new(object_type),
        }
    }

    /// Returns the exact runtime Workbench type.
    pub fn object_type(&self) -> &GlobalWorkbenchType {
        &self.state.object_type
    }

    pub(crate) fn descriptor(&self) -> Option<&'static dyn RuntimeObjectTypeDescriptor> {
        self.state.descriptor
    }

    /// Returns another runtime-typed copy of this object identity.
    pub fn erase(&self) -> Self {
        self.clone()
    }

    /// Recovers a typed reference when this object has the requested type.
    pub fn typed<T: ObjectType>(&self) -> Option<ObjectRef<T>> {
        if self.object_type() != &T::WORKBENCH_TYPE {
            return None;
        }
        Some(ObjectRef::typed_ref(self.name.clone(), self.uri.clone()))
    }

    /// Resolves the statically known secondary source components for this family.
    pub fn source_components(&self) -> Vec<SourceRef> {
        self.descriptor()
            .map(|descriptor| {
                descriptor
                    .source_component_paths()
                    .iter()
                    .map(|path| self.source_from_path(path))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolves the primary source when available.
    pub fn source(&self) -> Option<SourceRef> {
        self.descriptor()
            .and_then(|descriptor| descriptor.source_path())
            .map(|path| self.source_from_path(path))
    }

    /// Resolves one named secondary source component when available.
    pub fn source_component(&self, name: &str) -> Option<SourceRef> {
        self.descriptor()?
            .source_component_paths()
            .iter()
            .find(|path| path.last() == Some(&name))
            .map(|path| self.source_from_path(path))
    }

    /// Creates an immediate run operation when this object family supports it.
    pub fn run(&self) -> Result<ObjectRun, ObjectError> {
        let run = self
            .descriptor()
            .and_then(|descriptor| descriptor.run())
            .ok_or_else(|| ObjectError::UnsupportedCapability {
                object_type: self.object_type().clone(),
                capability: "immediate run",
            })?;
        Ok(ObjectRun::new(self.clone(), run))
    }

    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        LockRequest::new(self.clone(), access_mode)
    }

    /// Creates an operation that releases this object's lock.
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        if self.uri() != object_lock.object().uri() {
            return Err(ObjectError::ObjectLockMismatch {
                expected: self.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        Ok(UnlockRequest::new(object_lock))
    }

    pub(crate) fn source_from_path(&self, path: &[&str]) -> SourceRef {
        let uri = self
            .uri()
            .append_segments(path)
            .expect("static source path forms a valid ADT URI");
        SourceRef::new(self.clone(), uri)
    }
}

impl<T> PartialEq for ObjectRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

impl<T> Eq for ObjectRef<T> {}

impl<T> Hash for ObjectRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
    }
}

impl<T> fmt::Display for ObjectRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.uri.fmt(formatter)
    }
}

impl<T> serde::Serialize for ObjectRef<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct as _;

        let mut reference = serializer.serialize_struct("ObjectRef", 2)?;
        reference.serialize_field("name", &self.name)?;
        reference.serialize_field("uri", &self.uri)?;
        reference.end()
    }
}

impl<'de, T> serde::Deserialize<'de> for ObjectRef<T>
where
    T: ObjectType,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct SerializedObjectRef {
            name: String,
            uri: AdtUri,
        }

        let reference = SerializedObjectRef::deserialize(deserializer)?;
        Ok(Self::typed_ref(reference.name, reference.uri))
    }
}

impl Client<Ready> {
    fn object_identity(
        &self,
        category: CategoryId,
        name: &str,
    ) -> Result<(String, AdtUri), ObjectError> {
        let name = name.to_ascii_uppercase();
        let uri_name = name.to_ascii_lowercase();
        let collection = self.require_collection(category)?;
        let uri = collection.target().append_segments([&uri_name])?;
        Ok((name, uri))
    }

    /// Resolves a typed object reference from its statically known collection.
    pub fn object<T: ObjectType>(&self, name: &str) -> Result<ObjectRef<T>, ObjectError> {
        self.object_identity(T::CATEGORY, name)
            .map(|(name, uri)| ObjectRef::typed_ref(name, uri))
    }

    /// Resolves a runtime object reference from its Workbench type and name.
    pub fn repository_object(
        &self,
        object_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<ObjectRef<Erased>, ObjectError> {
        let descriptor = descriptors::object_type_descriptor(object_type).ok_or_else(|| {
            ObjectError::UnsupportedObjectType {
                object_type: object_type.clone(),
            }
        })?;
        let (name, uri) = self.object_identity(descriptor.category(), name)?;
        Ok(ObjectRef::erased(name, uri, object_type.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erased_reference_recovers_its_registered_type() {
        let program = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/Z_TEST").unwrap(),
        );
        let object = program.erase();

        assert_eq!(object.lock(AccessMode::Modify).object, object);
        assert_eq!(object.object_type().as_str(), "PROG/P");
        assert_eq!(object.typed::<Program>(), Some(program));
        assert!(object.typed::<Include>().is_none());
    }

    #[test]
    fn erased_reference_exposes_source_components_at_runtime() {
        let class = ObjectRef::<Class>::for_test(
            "ZCL_TEST",
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_test").unwrap(),
        );
        let object = class.erase();

        assert_eq!(
            object
                .source_components()
                .iter()
                .map(|component| component.uri.as_str())
                .collect::<Vec<_>>(),
            [
                "/sap/bc/adt/oo/classes/zcl_test/includes/definitions",
                "/sap/bc/adt/oo/classes/zcl_test/includes/implementations",
                "/sap/bc/adt/oo/classes/zcl_test/includes/macros",
                "/sap/bc/adt/oo/classes/zcl_test/includes/testclasses",
                "/sap/bc/adt/oo/classes/zcl_test/includes/localtypes",
            ]
        );
        assert_eq!(object.source(), Some(class.source()));
    }

    #[test]
    fn unmodeled_erased_reference_retains_runtime_type() {
        let object = ObjectRef::erased(
            "Z_UNSUPPORTED".to_owned(),
            AdtUri::parse("/sap/bc/adt/test/unsupported/z_unsupported").unwrap(),
            "TEST/X".parse().unwrap(),
        );

        assert_eq!(object.object_type().as_str(), "TEST/X");
        assert!(matches!(
            object.run(),
            Err(ObjectError::UnsupportedCapability {
                object_type,
                capability: "immediate run",
            }) if object_type.as_str() == "TEST/X"
        ));
    }
}
