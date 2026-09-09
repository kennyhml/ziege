use std::{any::Any, fmt, sync::Arc};

use super::{
    AdvertisedObjectReference, Identity, ObjectKey, ObjectRef, ObjectType, Resources, SnapshotKind,
    WorkbenchVersion, descriptors,
};
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
/// Runtime properties remain type-erased internally. Consumers can borrow them
/// through [`ObjectSnapshot::typed_properties`] or export them as JSON and supply
/// edited JSON to a property-update operation.
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
    /// Returns the logical key, including any known parent identity.
    /// Use [`Self::reference`] for operations that should retain the loaded URI.
    pub fn key(&self) -> &ObjectKey<T> {
        self.reference.key()
    }

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

    fn resolve_parent_name<'a>(
        &'a self,
        container: Option<&'a AdvertisedObjectReference>,
    ) -> Result<Option<&'a str>, ObjectError> {
        let parent = self.key().parent();
        if let Some(expected) = descriptors::parent_type(self.key().workbench_type()) {
            for actual in container
                .and_then(|container| container.workbench_type.as_ref())
                .into_iter()
                .chain(parent.map(ObjectKey::workbench_type))
            {
                if actual != expected {
                    return Err(ObjectError::UnexpectedObjectType {
                        expected: expected.clone(),
                        actual: actual.clone(),
                    });
                }
            }
        }
        let advertised_name = container
            .and_then(|container| container.name.as_deref())
            .filter(|name| !name.is_empty());
        if let Some(parent) = parent {
            if advertised_name.is_some_and(|name| !name.eq_ignore_ascii_case(parent.name())) {
                return Err(ObjectError::InvalidParentObject {
                    workbench_type: self.key().workbench_type().clone(),
                    reason: "logical parent and advertised container names disagree".to_owned(),
                });
            }
            Ok(Some(parent.name()))
        } else {
            Ok(advertised_name)
        }
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

    /// Borrows the logical parent name, falling back to the advertised container.
    /// Checks declared parent types and rejects conflicting names. URI-only metadata
    /// returns None. No I/O or changes to the snapshot identity are performed.
    pub fn parent_name(&self) -> Result<Option<&str>, ObjectError> {
        self.resolve_parent_name(self.properties.container())
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

    fn workbench_type(&self) -> &super::GlobalWorkbenchType {
        self.reference().workbench_type()
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

    /// Borrows the concrete properties after checking the object's Workbench type.
    ///
    /// `T` is the object family, such as [`super::Class`], not its properties type.
    /// The returned reference borrows this snapshot's storage without cloning or
    /// serializing it. A different family returns [`ObjectError::UnexpectedObjectType`].
    pub fn typed_properties<T: ObjectType>(&self) -> Result<&T::Properties, ObjectError> {
        if self.key().workbench_type() != &T::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: self.key().workbench_type().clone(),
            });
        }
        Ok(self
            .properties
            .downcast_ref::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type"))
    }

    /// Borrows the logical parent name, falling back to the advertised container.
    /// Checks declared parent types and rejects conflicting names. URI-only metadata
    /// returns None. No I/O or changes to the snapshot identity are performed.
    pub fn parent_name(&self) -> Result<Option<&str>, ObjectError> {
        let container = self
            .reference
            .require_descriptor()?
            .container(&self.properties);
        self.resolve_parent_name(container)
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
                actual: self.reference.workbench_type().clone(),
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
    use crate::{FunctionGroup, FunctionGroupInclude, FunctionModule, Program, XmlCodec};

    #[test]
    fn parent_names_borrow_without_changing_identity_or_wire_properties() {
        for (parent, advertised, expected) in [
            (None, Some("Z_TEST_GROUP"), Some("Z_TEST_GROUP")),
            (
                Some("Z_TEST_GROUP"),
                Some("z_test_group"),
                Some("Z_TEST_GROUP"),
            ),
            (Some("Z_TEST_GROUP"), None, Some("Z_TEST_GROUP")),
            (Some("Z_TEST_GROUP"), Some(""), Some("Z_TEST_GROUP")),
            (None, Some("/ACME/GROUP"), Some("/ACME/GROUP")),
            (None, None, None),
            (None, Some(""), None),
        ] {
            let mut properties = <FunctionModule as ObjectType>::Properties::from_xml(
                include_bytes!("../../tests/fixtures/function-module-zzzzfunc.xml"),
            )
            .unwrap();
            properties.container.name = advertised.map(str::to_owned);
            properties.container.workbench_type = None;
            // Parent names do not depend on parsing or validating an advertised URI.
            properties.container.uri = Some("https://unusable.invalid/group".to_owned());
            assert!(std::ptr::eq(
                properties.container().unwrap(),
                &properties.container
            ));
            let reference = ObjectRef::new(
                ObjectKey::<FunctionModule>::from_parts(
                    "ZZZZFUNC".to_owned(),
                    FunctionModule::WORKBENCH_TYPE,
                    parent.map(|name| Box::new(ObjectKey::<FunctionGroup>::new(name).erase())),
                ),
                AdtUri::parse("advertised/module").unwrap(),
            )
            .with_parent_uri(AdtUri::parse("advertised/group").unwrap());
            let snapshot = ObjectSnapshot::new(
                reference.clone(),
                WorkbenchVersion::Inactive,
                FunctionModule::MEDIA_TYPES[0],
                None,
                properties,
            );
            let original = serde_json::to_value(snapshot.properties()).unwrap();
            let name = snapshot.parent_name().unwrap();
            assert_eq!(name, expected);
            let stored = snapshot.key().parent().map(ObjectKey::name).or(snapshot
                .properties()
                .container
                .name
                .as_deref()
                .filter(|name| !name.is_empty()));
            assert_eq!(name.map(str::as_ptr), stored.map(str::as_ptr));

            let erased = snapshot.into_erased();
            assert_eq!(erased.parent_name().unwrap(), expected);
            let shared = erased.clone();
            if parent.is_none() {
                assert_eq!(
                    erased.parent_name().unwrap().map(str::as_ptr),
                    shared.parent_name().unwrap().map(str::as_ptr)
                );
            }
            assert_eq!(Arc::strong_count(&erased.properties), 2);
            assert_eq!(erased.properties().unwrap(), original);
            assert_eq!(erased.key(), reference.key());
            assert_eq!(erased.reference().parent_uri(), reference.parent_uri());
            let restored = shared.try_into_typed::<FunctionModule>().unwrap();
            assert_eq!(restored.parent_name().unwrap(), expected);
            assert_eq!(
                serde_json::to_value(restored.properties()).unwrap(),
                original
            );
            assert_eq!(restored.key(), reference.key());
        }
    }

    #[test]
    fn parent_names_reject_conflicting_names_and_declared_types() {
        for invalid in ["name", "container_type", "logical_type"] {
            let mut properties = <FunctionModule as ObjectType>::Properties::from_xml(
                include_bytes!("../../tests/fixtures/function-module-zzzzfunc.xml"),
            )
            .unwrap();
            let mut parent = ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP").erase();
            match invalid {
                "name" => properties.container.name = Some("Z_OTHER".to_owned()),
                "container_type" => {
                    properties.container.workbench_type = Some(Program::WORKBENCH_TYPE)
                }
                _ => parent = ObjectKey::<Program>::new("Z_TEST_GROUP").erase(),
            }
            let snapshot = ObjectSnapshot::new(
                ObjectRef::new(
                    ObjectKey::<FunctionModule>::from_parts(
                        "ZZZZFUNC".to_owned(),
                        FunctionModule::WORKBENCH_TYPE,
                        Some(Box::new(parent)),
                    ),
                    AdtUri::parse("advertised/module").unwrap(),
                ),
                WorkbenchVersion::Inactive,
                FunctionModule::MEDIA_TYPES[0],
                None,
                properties,
            );
            let typed_error = snapshot.parent_name().unwrap_err();
            let erased = snapshot.into_erased();
            for error in [typed_error, erased.parent_name().unwrap_err()] {
                if invalid == "name" {
                    assert!(
                        matches!(error, ObjectError::InvalidParentObject { workbench_type, .. }
                        if workbench_type == FunctionModule::WORKBENCH_TYPE)
                    );
                } else {
                    assert!(
                        matches!(error, ObjectError::UnexpectedObjectType { expected, actual }
                        if expected == FunctionGroup::WORKBENCH_TYPE && actual == Program::WORKBENCH_TYPE)
                    );
                }
            }
        }
    }

    #[test]
    fn groups_have_no_container_and_includes_expose_their_borrowed_container() {
        let properties = <FunctionGroup as ObjectType>::Properties::from_xml(include_bytes!(
            "../../tests/fixtures/function-group-z-test-group.xml"
        ))
        .unwrap();
        assert!(properties.container().is_none());
        let snapshot = ObjectSnapshot::new(
            ObjectRef::new(
                ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP"),
                AdtUri::parse("advertised/group").unwrap(),
            ),
            WorkbenchVersion::Active,
            FunctionGroup::MEDIA_TYPES[0],
            None,
            properties,
        );
        assert_eq!(snapshot.parent_name().unwrap(), None);
        assert_eq!(snapshot.into_erased().parent_name().unwrap(), None);

        let properties = <FunctionGroupInclude as ObjectType>::Properties::from_xml(
            include_bytes!("../../tests/fixtures/function-group-include-lz-test-grouptop.xml"),
        )
        .unwrap();
        assert!(std::ptr::eq(
            properties.container().unwrap(),
            &properties.container
        ));
        let name = properties.container.name.as_deref().unwrap().as_ptr();
        let snapshot = ObjectSnapshot::new(
            ObjectRef::new(
                ObjectKey::<FunctionGroupInclude>::from_parts(
                    "LZ_TEST_GROUPTOP".to_owned(),
                    FunctionGroupInclude::WORKBENCH_TYPE,
                    None,
                ),
                AdtUri::parse("advertised/include").unwrap(),
            ),
            WorkbenchVersion::Active,
            FunctionGroupInclude::MEDIA_TYPES[0],
            None,
            properties,
        );
        assert_eq!(snapshot.parent_name().unwrap().unwrap().as_ptr(), name);
        let erased = snapshot.into_erased();
        assert_eq!(erased.parent_name().unwrap(), Some("Z_TEST_GROUP"));
        assert_eq!(erased.parent_name().unwrap().unwrap().as_ptr(), name);
        assert!(erased.key().parent().is_none());
    }

    #[test]
    fn typed_properties_borrow_shared_storage_and_reject_a_different_family() {
        let properties = <FunctionModule as ObjectType>::Properties::from_xml(include_bytes!(
            "../../tests/fixtures/function-module-zzzzfunc.xml"
        ))
        .unwrap();
        let name = properties.name.as_ptr();
        let links = properties.links.as_ptr();
        let snapshot = ObjectSnapshot::new(
            ObjectRef::new(
                ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP")
                    .subobject::<FunctionModule>("ZZZZFUNC"),
                AdtUri::parse("advertised/module").unwrap(),
            ),
            WorkbenchVersion::Active,
            FunctionModule::MEDIA_TYPES[0],
            None,
            properties,
        )
        .into_erased();
        let shared = snapshot.clone();
        let borrowed = snapshot.typed_properties::<FunctionModule>().unwrap();
        assert_eq!(borrowed.name.as_ptr(), name);
        assert_eq!(borrowed.links.as_ptr(), links);
        assert!(std::ptr::eq(
            borrowed,
            snapshot
                .properties
                .downcast_ref::<<FunctionModule as ObjectType>::Properties>()
                .unwrap(),
        ));
        assert!(std::ptr::eq(
            borrowed,
            shared.typed_properties::<FunctionModule>().unwrap(),
        ));
        assert!(matches!(
            snapshot.typed_properties::<FunctionGroup>(),
            Err(ObjectError::UnexpectedObjectType { expected, actual })
                if expected == FunctionGroup::WORKBENCH_TYPE
                    && actual == FunctionModule::WORKBENCH_TYPE
        ));
        assert!(std::ptr::eq(
            borrowed,
            snapshot.typed_properties::<FunctionModule>().unwrap(),
        ));
        assert_eq!(Arc::strong_count(&snapshot.properties), 2);
        assert_eq!(
            serde_json::to_value(borrowed).unwrap(),
            snapshot.properties().unwrap()
        );
    }

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
        assert!(std::ptr::eq(snapshot.key(), snapshot.reference().key()));
        assert_eq!(snapshot.key(), reference.key());
        let cloned = snapshot.clone();
        assert_eq!(cloned.resources(), snapshot.resources());
        let erased = cloned.into_erased();
        assert_eq!(erased.resources(), snapshot.resources());
        assert_eq!(erased.clone().resources(), snapshot.resources());
        assert!(std::ptr::eq(erased.key(), erased.reference().key()));
        assert_eq!(erased.key(), &reference.key().erase());
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
