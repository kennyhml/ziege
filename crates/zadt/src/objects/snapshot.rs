use std::{any::Any, fmt, sync::Arc};

use super::{Identity, ObjectRef, ObjectType, Resources, SnapshotKind, WorkbenchVersion};
use crate::{AdtUri, EntityTag, ObjectError, SnapshotResources};

pub(crate) type ErasedProperties = Arc<dyn Any + Send + Sync>;

/// An immutable snapshot of a loaded ADT object representation.
///
/// Unlike [`ObjectRef<T>`], this value includes the Workbench version and object
/// properties returned by ADT. The type parameter
/// `T` selects the property type and the operations available for that object
/// family. [`ObjectSnapshot<()>`] stores the object family and its concrete
/// properties at runtime.
///
/// The runtime form is the loaded counterpart to [`ObjectRef<()>`]. It is useful
/// when the object family comes from user input or a repository response.
/// Supported object families are handled through an internal descriptor, and
/// operations check that descriptor and the loaded properties at runtime.
///
/// Runtime properties remain type-erased internally. Consumers can export them
/// as JSON and supply edited JSON to a property-update operation.
///
/// Some operations use links advertised by the loaded properties. Operations
/// that only need the object identity can use [`ObjectSnapshot::reference`].
pub struct ObjectSnapshot<T: SnapshotKind = ()> {
    reference: ObjectRef<T>,
    workbench_version: WorkbenchVersion,
    media_type: &'static str,
    etag: Option<EntityTag>,
    properties: T::StoredProperties,
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Returns the reference identifying this snapshot.
    pub fn reference(&self) -> &ObjectRef<T> {
        &self.reference
    }

    /// Returns the concrete URI from which this snapshot was loaded.
    pub fn uri(&self) -> &AdtUri {
        self.reference.uri()
    }

    /// Returns the Workbench version represented by this snapshot.
    pub fn workbench_version(&self) -> WorkbenchVersion {
        self.workbench_version
    }

    /// Returns the media type of this snapshot.
    pub fn media_type(&self) -> &'static str {
        self.media_type
    }

    /// Returns the entity tag associated with this snapshot.
    pub fn etag(&self) -> Option<&EntityTag> {
        self.etag.as_ref()
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Creates a new snapshot for an internally parsed query result.
    pub(crate) fn new(
        reference: ObjectRef<T>,
        workbench_version: WorkbenchVersion,
        media_type: &'static str,
        etag: Option<EntityTag>,
        properties: T::Properties,
    ) -> Self {
        Self {
            reference,
            workbench_version,
            media_type,
            etag,
            properties,
        }
    }

    /// Returns the immutable properties in this snapshot.
    pub fn properties(&self) -> &T::Properties {
        &self.properties
    }

    /// Returns a borrowed resource view bound to this snapshot's URI.
    ///
    /// Derived on demand from the properties; resource targets are validated when used.
    pub fn resources(&self) -> SnapshotResources<'_> {
        SnapshotResources::new(self.uri(), self.properties.resources())
    }

    /// Erases the concrete object type of this snapshot.
    ///
    /// All data is retained and properties move into type-erased storage.
    pub fn into_erased(self) -> ObjectSnapshot<()> {
        ObjectSnapshot::<()>::new_erased(
            self.reference.erase(),
            self.workbench_version,
            self.media_type,
            self.etag,
            Arc::new(self.properties),
        )
    }
}

impl<T> Clone for ObjectSnapshot<T>
where
    T: SnapshotKind,
{
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            workbench_version: self.workbench_version,
            media_type: self.media_type,
            etag: self.etag.clone(),
            properties: self.properties.clone(),
        }
    }
}

impl<T: SnapshotKind> Identity for ObjectSnapshot<T> {
    fn object_name(&self) -> &str {
        self.reference().object_name()
    }

    fn object_type(&self) -> &super::GlobalWorkbenchType {
        self.reference().object_type()
    }
}

impl ObjectSnapshot<()> {
    /// Constructs a snapshot with properties retained behind its runtime descriptor.
    pub(crate) fn new_erased(
        reference: ObjectRef<()>,
        workbench_version: WorkbenchVersion,
        media_type: &'static str,
        etag: Option<EntityTag>,
        properties: ErasedProperties,
    ) -> Self {
        Self {
            reference,
            workbench_version,
            media_type,
            etag,
            properties,
        }
    }

    /// Exports the concrete properties through their runtime JSON representation.
    ///
    /// Wire identity is retained in the JSON representation and must match this
    /// snapshot when constructing an update.
    pub fn properties(&self) -> Result<serde_json::Value, ObjectError> {
        self.reference
            .require_descriptor()?
            .properties_to_json(self.reference.key(), &self.properties)
    }

    /// Returns a borrowed resource view bound to this snapshot's URI.
    ///
    /// Derived on demand from the properties; resource targets are validated when used.
    pub fn resources(&self) -> SnapshotResources<'_> {
        let view = self
            .reference
            .require_descriptor()
            .expect("snapshots must retain a registered object descriptor")
            .resources(&self.properties);
        SnapshotResources::new(self.uri(), view)
    }

    /// Restores a concrete loaded object after validating its runtime type.
    pub fn try_into_typed<T>(self) -> Result<ObjectSnapshot<T>, ObjectError>
    where
        T: ObjectType,
    {
        let reference = self.typed_reference::<T>()?;

        // If we could recover the reference from `T` then the property type matches too.
        let properties = self
            .properties
            .downcast::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type");

        let properties = match Arc::try_unwrap(properties) {
            Ok(properties) => properties,
            Err(properties) => properties.as_ref().clone(),
        };

        Ok(ObjectSnapshot {
            reference,
            workbench_version: self.workbench_version,
            media_type: self.media_type,
            etag: self.etag,
            properties,
        })
    }

    /// Returns a type tagged reference to the underlying object.
    pub(crate) fn typed_reference<T: ObjectType>(&self) -> Result<ObjectRef<T>, ObjectError> {
        self.reference
            .typed::<T>()
            .ok_or_else(|| ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: self.reference.object_type().clone(),
            })
    }
}

impl fmt::Debug for ObjectSnapshot<()> {
    // Custom debug implementation to ignore the erased properties
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectSnapshot")
            .field("reference", &self.reference)
            .field("workbench_version", &self.workbench_version)
            .field("media_type", &self.media_type)
            .field("etag", &self.etag)
            .field("properties", &"<type-erased>")
            .finish()
    }
}

impl<T> fmt::Debug for ObjectSnapshot<T>
where
    T: ObjectType,
    ObjectRef<T>: fmt::Debug,
    T::Properties: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectSnapshot")
            .field("reference", &self.reference)
            .field("workbench_version", &self.workbench_version)
            .field("media_type", &self.media_type)
            .field("etag", &self.etag)
            .field("properties", &self.properties)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunctionGroup, FunctionModule, ObjectKey, XmlCodec};

    #[test]
    fn resource_entries_borrow_typed_and_erased_properties() {
        let properties = <FunctionModule as ObjectType>::Properties::from_xml(include_bytes!(
            "../../tests/fixtures/function-module-zzzzfunc.xml"
        ))
        .unwrap();
        let snapshot = ObjectSnapshot::new(
            ObjectRef::new(
                ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP")
                    .subobject::<FunctionModule>("ZZZZFUNC"),
                AdtUri::parse("advertised/module").unwrap(),
            ),
            WorkbenchVersion::Active,
            "application/vnd.sap.adt.functions.fmodules.v3+xml",
            None,
            properties,
        );

        let view: crate::ResourceView<'_> = snapshot.properties().resources();
        assert_eq!(
            SnapshotResources::new(snapshot.uri(), view),
            snapshot.resources(),
        );

        // Entries outlive the temporary view and borrow the snapshot's wire links.
        let entry = snapshot
            .resources()
            .iter()
            .find(|entry| entry.component().is_none())
            .unwrap();
        assert!(std::ptr::eq(
            entry.link().unwrap(),
            &snapshot.properties().links[0],
        ));

        let erased = snapshot.into_erased();
        let entry = erased
            .resources()
            .iter()
            .find(|entry| entry.component().is_none())
            .unwrap();
        let properties = erased
            .properties
            .downcast_ref::<<FunctionModule as ObjectType>::Properties>()
            .unwrap();
        assert!(std::ptr::eq(entry.link().unwrap(), &properties.links[0]));
    }

    #[test]
    fn snapshot_conversions_preserve_the_located_reference_and_resources() {
        let reference = ObjectRef::new(
            ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP").subobject::<FunctionModule>("ZZZZFUNC"),
            AdtUri::parse("advertised/module").unwrap(),
        )
        .with_parent_uri(AdtUri::parse("advertised/group").unwrap());
        let properties = <FunctionModule as ObjectType>::Properties::from_xml(include_bytes!(
            "../../tests/fixtures/function-module-zzzzfunc.xml"
        ))
        .unwrap();
        let snapshot = ObjectSnapshot::new(
            reference.clone(),
            WorkbenchVersion::Active,
            "application/vnd.sap.adt.functions.fmodules.v3+xml",
            None,
            properties,
        );
        assert!(!snapshot.resources().is_empty());
        let cloned = snapshot.clone();
        assert_eq!(cloned.resources(), snapshot.resources());
        let erased = cloned.into_erased();
        assert_eq!(erased.resources(), snapshot.resources());
        assert_eq!(erased.clone().resources(), snapshot.resources());
        assert_eq!(erased.reference().key(), &reference.key().erase());
        assert_eq!(erased.uri(), reference.uri());
        assert_eq!(erased.reference().parent_uri(), reference.parent_uri());
        assert!(erased.properties().is_ok());
        for source in [snapshot.source().unwrap(), erased.source().unwrap()] {
            assert_eq!(source.object.key(), &reference.key().erase());
            assert_eq!(source.object.uri(), reference.uri());
            assert_eq!(source.object.parent_uri(), reference.parent_uri());
        }
        let shared = erased.clone().try_into_typed::<FunctionModule>().unwrap();
        assert_eq!(shared.resources(), snapshot.resources());
        let typed = erased.try_into_typed::<FunctionModule>().unwrap();
        assert_eq!(typed.resources(), snapshot.resources());
        assert_eq!(typed.reference().key(), reference.key());
        assert_eq!(typed.uri(), reference.uri());
        assert_eq!(typed.reference().parent_uri(), reference.parent_uri());
        assert_eq!(typed.workbench_version(), snapshot.workbench_version());
        assert_eq!(typed.media_type(), snapshot.media_type());
    }
}
