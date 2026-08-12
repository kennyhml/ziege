use std::{fmt, hash::Hash, marker::PhantomData};

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
pub trait ObjectType: private::Sealed + Send + Sync + Sized + 'static {
    /// The objects global Workbench type.
    const WORKBENCH_TYPE: GlobalWorkbenchType;

    /// The stable category identifying the canonical object collection.
    const CATEGORY: CategoryId;
}

/// A runtime repository object backed by an optional modeled-type descriptor.
///
/// RIS objects with an unmodeled Workbench type retain their exact identity but
/// report family-specific capabilities as unsupported.
#[derive(Clone)]
pub struct RepositoryObject {
    reference: ObjectRef,
    object_type: GlobalWorkbenchType,
    descriptor: Option<&'static dyn RuntimeObjectTypeDescriptor>,
}

impl RepositoryObject {
    pub(crate) fn from_reference(
        reference: ObjectRef,
        object_type: GlobalWorkbenchType,
    ) -> Result<Self, ObjectError> {
        let descriptor = descriptors::object_type_descriptor(&object_type);
        Ok(Self {
            reference,
            object_type,
            descriptor,
        })
    }

    /// Returns the object's type-erased identity.
    pub fn reference(&self) -> ObjectRef {
        self.reference.clone()
    }

    /// Returns the exact runtime object type.
    pub fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }

    /// Resolves the statically known secondary source components for this family.
    pub fn source_components(&self) -> Vec<SourceRef> {
        self.descriptor
            .map(|descriptor| {
                descriptor
                    .source_component_paths()
                    .iter()
                    .map(|path| self.reference.source_from_path(path))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolves the primary source when available.
    pub fn source(&self) -> Option<SourceRef> {
        self.descriptor
            .and_then(|descriptor| descriptor.source_path())
            .map(|path| self.reference.source_from_path(path))
    }

    /// Resolves one named secondary source component when available.
    pub fn source_component(&self, name: &str) -> Option<SourceRef> {
        self.descriptor?
            .source_component_paths()
            .iter()
            .find(|path| path.last() == Some(&name))
            .map(|path| self.reference.source_from_path(path))
    }

    /// Creates an immediate run operation when this object family supports it.
    pub fn run(&self) -> Result<ObjectRun, ObjectError> {
        let run = self
            .descriptor
            .and_then(|descriptor| descriptor.run())
            .ok_or_else(|| ObjectError::UnsupportedCapability {
                object_type: self.object_type.clone(),
                capability: "immediate run",
            })?;
        Ok(ObjectRun::new(
            self.reference.clone(),
            self.object_type.clone(),
            run,
        ))
    }

    /// Recovers a typed reference when this object has the requested type.
    pub fn typed<T: ObjectType>(&self) -> Option<ObjectRef<T>> {
        if self.object_type != T::WORKBENCH_TYPE {
            return None;
        }
        Some(self.reference.retype())
    }

    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        LockRequest::new(self.reference(), access_mode)
    }

    /// Creates an operation that releases this object's lock.
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        let reference = self.reference();
        if reference.uri() != object_lock.object().uri() {
            return Err(ObjectError::ObjectLockMismatch {
                expected: reference.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        Ok(UnlockRequest::new(object_lock))
    }

    /// Placeholder while runtime properties dispatch is being redesigned.
    pub fn properties(&self) -> ! {
        panic!("type-erased object properties are not implemented")
    }

    /// Placeholder while runtime properties updates are being redesigned.
    pub fn update<P>(&self, _object_lock: &ObjectLock, _properties: P) -> ! {
        panic!("type-erased object properties updates are not implemented")
    }
}

impl<T> From<ObjectRef<T>> for RepositoryObject
where
    T: ObjectType,
{
    fn from(reference: ObjectRef<T>) -> Self {
        let object_type = T::WORKBENCH_TYPE;
        let descriptor = descriptors::object_type_descriptor(&object_type);
        Self {
            reference: reference.erase(),
            object_type,
            descriptor,
        }
    }
}

impl fmt::Debug for RepositoryObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryObject")
            .field("reference", &self.reference)
            .field("object_type", &self.object_type)
            .field("modeled", &self.descriptor.is_some())
            .finish()
    }
}

/// A validated ADT object identity, optionally tagged with its static object type.
///
/// A bare `ObjectRef` is type-erased and proves only the objects identity and
/// location. [`Client::object`] returns `ObjectRef<T>` for a known
/// [`ObjectType`].
pub struct ObjectRef<T = ()> {
    name: String,
    uri: AdtUri,
    marker: PhantomData<fn() -> T>,
}

impl ObjectRef {
    /// Creates a type-erased object reference from a validated ADT resource URI.
    pub(crate) fn new(uri: AdtUri) -> Self {
        Self::typed(String::new(), uri)
    }

    pub(crate) fn named(name: String, uri: AdtUri) -> Self {
        Self::typed(name, uri)
    }
}

impl<T> ObjectRef<T> {
    fn typed(name: String, uri: AdtUri) -> Self {
        Self {
            name,
            uri,
            marker: PhantomData,
        }
    }

    /// Returns the object's resource URI.
    pub fn uri(&self) -> &AdtUri {
        &self.uri
    }

    /// Returns the object name when this reference carries one.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a type-erased copy of this object identity.
    pub fn erase(&self) -> ObjectRef {
        ObjectRef::typed(self.name.clone(), self.uri.clone())
    }

    pub(crate) fn retype<U>(&self) -> ObjectRef<U> {
        ObjectRef::typed(self.name.clone(), self.uri.clone())
    }

    pub(crate) fn source_from_path(&self, path: &[&str]) -> SourceRef {
        let uri = self
            .uri()
            .append_segments(path)
            .expect("static source path forms a valid ADT URI");
        SourceRef::new(self.erase(), uri)
    }
}

impl<T: ObjectType> ObjectRef<T> {
    pub(crate) fn from_parts(name: String, uri: AdtUri) -> Self {
        Self::typed(name, uri)
    }
}

impl<T> Clone for ObjectRef<T> {
    fn clone(&self) -> Self {
        Self::typed(self.name.clone(), self.uri.clone())
    }
}

impl<T> fmt::Debug for ObjectRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("ObjectRef");
        if !self.name.is_empty() {
            debug.field("name", &self.name);
        }
        debug.field("uri", &self.uri).finish()
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

impl Client<Ready> {
    fn object_reference(&self, category: CategoryId, name: &str) -> Result<ObjectRef, ObjectError> {
        let name = name.to_ascii_uppercase();
        let uri_name = name.to_ascii_lowercase();
        let collection = self.require_collection(category)?;
        let uri = collection.target().append_segments([&uri_name])?;
        Ok(ObjectRef::named(name, uri))
    }

    /// Resolves a typed object reference from its statically known collection.
    ///
    /// Constructing a reference performs no request; the collection URI comes
    /// from the capabilities already retained by the ready client.
    pub fn object<T: ObjectType>(&self, name: &str) -> Result<ObjectRef<T>, ObjectError> {
        self.object_reference(T::CATEGORY, name)
            .map(|reference| reference.retype())
    }

    /// Resolves a runtime repository object from its Workbench type and name.
    pub fn repository_object(
        &self,
        object_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<RepositoryObject, ObjectError> {
        let descriptor = descriptors::object_type_descriptor(object_type).ok_or_else(|| {
            ObjectError::UnsupportedObjectType {
                object_type: object_type.clone(),
            }
        })?;
        let reference = self.object_reference(descriptor.category(), name)?;
        Ok(RepositoryObject {
            reference,
            object_type: object_type.clone(),
            descriptor: Some(descriptor),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_object_recovers_its_registered_type() {
        let program = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/Z_TEST").unwrap(),
        );
        let object = RepositoryObject::from(program.clone());

        let request = object.lock(AccessMode::Modify);

        assert_eq!(request.object, program.erase());
        assert_eq!(object.object_type().as_str(), "PROG/P");
        assert_eq!(object.typed::<Program>(), Some(program));
        assert!(object.typed::<Include>().is_none());
    }

    #[test]
    fn repository_object_exposes_source_components_at_runtime() {
        let class = ObjectRef::<Class>::for_test(
            "ZCL_TEST",
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_test").unwrap(),
        );
        let object = RepositoryObject::from(class.clone());

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
        assert!(object.source_component("main").is_none());
        assert_eq!(
            object.source_component("definitions"),
            Some(class.component_source(ClassSourceComponent::Definitions))
        );
        assert_eq!(
            object.source_component("localtypes"),
            Some(class.component_source(ClassSourceComponent::LocalTypes))
        );
        assert!(object.source_component("unknown").is_none());

        let package = ObjectRef::<Package>::for_test(
            "ZPACKAGE",
            AdtUri::parse("/sap/bc/adt/packages/zpackage").unwrap(),
        );
        let object = RepositoryObject::from(package);

        assert!(object.source_components().is_empty());
        assert!(object.source().is_none());
        assert!(object.source_component("main").is_none());
    }

    #[test]
    fn descriptor_registry_exposes_immediate_run_only_for_programs_and_classes() {
        let program = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
        );
        let class = ObjectRef::<Class>::for_test(
            "ZCL_TEST",
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_test").unwrap(),
        );
        let include = ObjectRef::<Include>::for_test(
            "ZTEST",
            AdtUri::parse("/sap/bc/adt/programs/includes/ztest").unwrap(),
        );
        let package = ObjectRef::<Package>::for_test(
            "ZPACKAGE",
            AdtUri::parse("/sap/bc/adt/packages/zpackage").unwrap(),
        );
        let data_element = ObjectRef::<DataElement>::for_test(
            "ZDATA_ELEMENT",
            AdtUri::parse("/sap/bc/adt/ddic/dataelements/zdata_element").unwrap(),
        );

        assert!(RepositoryObject::from(program).run().is_ok());
        assert!(RepositoryObject::from(class).run().is_ok());
        for object in [
            RepositoryObject::from(include),
            RepositoryObject::from(package),
            RepositoryObject::from(data_element),
        ] {
            assert!(matches!(
                object.run(),
                Err(ObjectError::UnsupportedCapability {
                    capability: "immediate run",
                    ..
                })
            ));
        }
    }

    #[test]
    fn other_repository_objects_report_unsupported_run_capability() {
        let unsupported = ObjectRef::named(
            "Z_UNSUPPORTED".to_owned(),
            AdtUri::parse("/sap/bc/adt/test/unsupported/z_unsupported").unwrap(),
        );
        let object =
            RepositoryObject::from_reference(unsupported, "TEST/X".parse().unwrap()).unwrap();

        assert!(matches!(
            object.run(),
            Err(ObjectError::UnsupportedCapability {
                object_type,
                capability: "immediate run",
            }) if object_type.as_str() == "TEST/X"
        ));
    }
}
