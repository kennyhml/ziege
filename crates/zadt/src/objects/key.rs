use std::{fmt, hash::Hash, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize};

use super::{GlobalWorkbenchType, Identity, ObjectType, PrimaryObjectType, SubObject, descriptors};
use crate::error::ObjectError;

/// A logical key identifying an ADT object independently of its URI.
///
/// Unlike [`crate::ObjectRef<T>`], this value has no concrete resource location.
/// The type parameter `T` selects the operations available for that object
/// family.
///
/// [`ObjectKey<()>`] stores the object family at runtime. It is useful when the
/// family comes from user input or a repository response.
///
/// A [`crate::Discovery`] resolves the object when an operation is encoded. Child
/// keys retain their parent so the relationship template can be selected
/// without caching discovery state on the key.
#[derive(Debug, Serialize)]
#[serde(bound(serialize = ""))]
pub struct ObjectKey<T = ()> {
    /// The full name of the object
    name: String,

    /// The workbench type of the object
    #[serde(rename = "object_type")]
    workbench_type: GlobalWorkbenchType,

    /// An optional parent of this object, if it has one
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<Box<ObjectKey<()>>>,

    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<P: PrimaryObjectType> ObjectKey<P> {
    /// Creates a logical subobject key belonging to this primary object.
    ///
    /// [`SubObject<C>`] guarantees that `P` declares a subobject relationship
    /// with `C`, so constructing the child key is infallible.
    ///
    /// The parent key is retained for later discovery-based resolution.
    pub fn subobject<C>(&self, name: impl Into<String>) -> ObjectKey<C>
    where
        C: ObjectType,
        P: SubObject<C>,
    {
        ObjectKey::from_parts(name.into(), C::WORKBENCH_TYPE, Some(Box::new(self.erase())))
    }
}

impl ObjectKey<()> {
    /// Creates a logical subobject key belonging to this primary object.
    ///
    /// Because the object type is erased, there are no static guarantees that
    /// this object has any of the requested subobjects or even any at all. This
    /// is instead turned into a descriptor backed runtime check.
    ///
    /// The parent key is retained for later discovery-based resolution.
    pub fn subobject(
        &self,
        child_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<ObjectKey<()>, ObjectError> {
        let descriptor = self.require_descriptor()?;
        descriptor
            .subobjects()
            .iter()
            .find(|subobject| subobject.workbench_type() == child_type)
            .ok_or_else(|| ObjectError::UnsupportedSubObjectType {
                parent_type: self.workbench_type().clone(),
                child_type: child_type.clone(),
            })?;

        Ok(ObjectKey::from_parts(
            name.to_owned(),
            child_type.clone(),
            Some(Box::new(self.clone())),
        ))
    }
}

impl<T> ObjectKey<T> {
    pub(crate) fn from_parts(
        name: String,
        workbench_type: GlobalWorkbenchType,
        parent: Option<Box<ObjectKey<()>>>,
    ) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            workbench_type,
            parent,
            marker: PhantomData,
        }
    }

    /// Returns the object name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the exact Workbench type retained by this key.
    pub fn workbench_type(&self) -> &GlobalWorkbenchType {
        &self.workbench_type
    }

    /// Returns a runtime-typed copy of this object identity.
    pub fn erase(&self) -> ObjectKey<()> {
        self.retag()
    }

    fn retag<U>(&self) -> ObjectKey<U> {
        ObjectKey {
            name: self.name.clone(),
            workbench_type: self.workbench_type.clone(),
            parent: self.parent.clone(),
            marker: PhantomData,
        }
    }

    pub(crate) fn descriptor(&self) -> Option<&'static descriptors::ObjectTypeDescriptor> {
        descriptors::object_type_descriptor(&self.workbench_type)
    }

    pub(crate) fn require_descriptor(
        &self,
    ) -> Result<&'static descriptors::ObjectTypeDescriptor, ObjectError> {
        self.descriptor()
            .ok_or_else(|| ObjectError::UnsupportedObjectType {
                workbench_type: self.workbench_type().clone(),
            })
    }

    pub(crate) fn unsupported_capability(&self, capability: &'static str) -> ObjectError {
        ObjectError::UnsupportedCapability {
            workbench_type: self.workbench_type.clone(),
            capability,
        }
    }

    /// Returns the logical parent identity for a subobject.
    pub fn parent(&self) -> Option<&ObjectKey<()>> {
        self.parent.as_deref()
    }
}

impl<T: PrimaryObjectType> ObjectKey<T> {
    /// Creates a logical primary-object key.
    pub fn new(name: impl Into<String>) -> Self {
        Self::from_parts(name.into(), T::WORKBENCH_TYPE, None)
    }
}

impl ObjectKey<()> {
    /// Creates a logical primary-object key from a Workbench type.
    ///
    /// Subobjects require a parent and must instead be created through
    /// [`ObjectKey::subobject`].
    pub fn from_workbench_type(
        workbench_type: &GlobalWorkbenchType,
        name: impl Into<String>,
    ) -> Result<Self, ObjectError> {
        let descriptor = descriptors::object_type_descriptor(workbench_type).ok_or_else(|| {
            ObjectError::UnsupportedObjectType {
                workbench_type: workbench_type.clone(),
            }
        })?;

        descriptor
            .category()
            .ok_or_else(|| ObjectError::ParentObjectRequired {
                workbench_type: workbench_type.clone(),
            })?;

        Ok(Self::from_parts(name.into(), workbench_type.clone(), None))
    }

    /// Recovers a typed key when this object has the requested type.
    pub fn typed<T: ObjectType>(&self) -> Option<ObjectKey<T>> {
        if self.workbench_type() != &T::WORKBENCH_TYPE {
            return None;
        }
        Some(self.retag())
    }
}

impl<T> Clone for ObjectKey<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            workbench_type: self.workbench_type.clone(),
            parent: self.parent.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Identity for ObjectKey<T> {
    fn object_name(&self) -> &str {
        self.name()
    }

    fn workbench_type(&self) -> &GlobalWorkbenchType {
        self.workbench_type()
    }
}

impl<T, U> PartialEq<ObjectKey<U>> for ObjectKey<T> {
    fn eq(&self, other: &ObjectKey<U>) -> bool {
        self.name == other.name
            && self.workbench_type == other.workbench_type
            && self.parent == other.parent
    }
}

impl<T> Eq for ObjectKey<T> {}

impl<T> Hash for ObjectKey<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.workbench_type.hash(state);
        self.parent.hash(state);
    }
}

impl<T> fmt::Display for ObjectKey<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.name, self.workbench_type)
    }
}

/// Temporary Serde value used while deserializing an [`ObjectKey`].
///
/// For typed keys, the Workbench type is checked before the key is constructed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawObjectKey {
    name: String,
    #[serde(rename = "object_type")]
    workbench_type: GlobalWorkbenchType,
    #[serde(default)]
    parent: Option<Box<ObjectKey<()>>>,
}

impl<'de> Deserialize<'de> for ObjectKey<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let reference = RawObjectKey::deserialize(deserializer)?;
        validate_parent_identity(&reference).map_err(serde::de::Error::custom)?;
        Ok(Self::from_parts(
            reference.name,
            reference.workbench_type,
            reference.parent,
        ))
    }
}

impl<'de, T: ObjectType> Deserialize<'de> for ObjectKey<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let reference = RawObjectKey::deserialize(deserializer)?;
        if reference.workbench_type != T::WORKBENCH_TYPE {
            return Err(serde::de::Error::custom(
                ObjectError::UnexpectedObjectType {
                    expected: T::WORKBENCH_TYPE,
                    actual: reference.workbench_type,
                },
            ));
        }
        validate_parent_identity(&reference).map_err(serde::de::Error::custom)?;
        Ok(Self::from_parts(
            reference.name,
            T::WORKBENCH_TYPE,
            reference.parent,
        ))
    }
}

fn validate_parent_identity(reference: &RawObjectKey) -> Result<(), ObjectError> {
    let Some(parent) = &reference.parent else {
        return Ok(());
    };
    if !descriptors::requires_parent(&reference.workbench_type) {
        return Err(ObjectError::InvalidParentObject {
            workbench_type: reference.workbench_type.clone(),
            reason: "the object type is directly addressable".to_owned(),
        });
    }
    if !descriptors::supports_subobject(&parent.workbench_type, &reference.workbench_type) {
        return Err(ObjectError::InvalidParentObject {
            workbench_type: reference.workbench_type.clone(),
            reason: format!(
                "type `{}` does not declare this subobject relationship",
                parent.workbench_type
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunctionGroup, FunctionModule, Include, Program};

    #[test]
    fn deserialized_keys_normalize_names_including_parent_names() {
        let json = serde_json::json!({
            "name": "z_module",
            "object_type": "FUGR/FF",
            "parent": { "name": "z_group", "object_type": "FUGR/F" }
        });
        let expected =
            ObjectKey::<FunctionGroup>::new("z_group").subobject::<FunctionModule>("z_module");
        let serialized = serde_json::json!({
            "name": "Z_MODULE",
            "object_type": "FUGR/FF",
            "parent": { "name": "Z_GROUP", "object_type": "FUGR/F" }
        });
        assert_eq!(serde_json::to_value(&expected).unwrap(), serialized);
        assert_eq!(serde_json::to_value(expected.erase()).unwrap(), serialized);
        assert_eq!(
            serde_json::from_value::<ObjectKey<FunctionModule>>(json.clone()).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::from_value::<ObjectKey<()>>(json).unwrap(),
            expected.erase()
        );
        let primary: ObjectKey<Program> = serde_json::from_value(serde_json::json!({
            "name": "/namespace/z_program", "object_type": "PROG/P"
        }))
        .unwrap();
        assert_eq!(primary, ObjectKey::<Program>::new("/namespace/z_program"));
    }

    #[test]
    fn erased_key_recovers_its_registered_type() {
        let program = ObjectKey::<Program>::new("Z_TEST");
        let object = program.erase();

        assert_eq!(object.workbench_type().as_str(), "PROG/P");
        assert_eq!(object.typed::<Program>(), Some(program));
        assert!(object.typed::<Include>().is_none());
    }

    #[test]
    fn equality_across_markers_preserves_name_type_and_parent() {
        let group = ObjectKey::<FunctionGroup>::new("Z_GROUP");
        let module = group.subobject::<FunctionModule>("Z_MODULE");
        assert_eq!(module, module.erase());
        assert_eq!(module.erase(), module);

        let other_parent = ObjectKey::<FunctionGroup>::new("Z_OTHER")
            .subobject::<FunctionModule>("Z_MODULE")
            .erase();
        assert_ne!(module, other_parent);
        assert_ne!(other_parent, module);
        assert_ne!(module, group.subobject::<FunctionModule>("Z_OTHER").erase());
        assert_ne!(module, ObjectKey::<Program>::new("Z_MODULE"));
    }

    #[test]
    fn typed_key_deserialization_validates_its_marker() {
        let program = ObjectKey::<Program>::new("Z_TEST");
        let serialized = serde_json::to_value(&program).unwrap();

        assert_eq!(
            serialized,
            serde_json::json!({
                "name": "Z_TEST", "object_type": "PROG/P"
            })
        );
        assert!(serde_json::from_value::<ObjectKey<Program>>(serialized.clone()).is_ok());
        assert!(serde_json::from_value::<ObjectKey<crate::Class>>(serialized).is_err());

        let renamed_wire_field = serde_json::json!({
            "name": "Z_TEST", "workbench_type": "PROG/P"
        });
        assert!(serde_json::from_value::<ObjectKey<Program>>(renamed_wire_field.clone()).is_err());
        assert!(serde_json::from_value::<ObjectKey<()>>(renamed_wire_field).is_err());
    }

    #[test]
    fn key_deserialization_rejects_unknown_fields_including_parents() {
        let module =
            ObjectKey::<FunctionGroup>::new("Z_GROUP").subobject::<FunctionModule>("Z_MODULE");
        let original = serde_json::to_value(module).unwrap();
        for pointer in ["", "/parent"] {
            let mut json = original.clone();
            json.pointer_mut(pointer).unwrap()["unexpected"] = true.into();
            let typed =
                serde_json::from_value::<ObjectKey<FunctionModule>>(json.clone()).unwrap_err();
            let erased = serde_json::from_value::<ObjectKey<()>>(json).unwrap_err();
            for error in [typed, erased] {
                assert!(
                    error.to_string().contains("unknown field `unexpected`"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn key_deserialization_validates_parent_metadata() {
        let group = ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP");
        let module = group.subobject::<FunctionModule>("ZZZZFUNC");
        let serialized = serde_json::to_value(&module).unwrap();

        assert!(serde_json::from_value::<ObjectKey<FunctionModule>>(serialized.clone()).is_ok());

        let mut wrong_parent_type = serialized.clone();
        wrong_parent_type["parent"]["object_type"] = serde_json::json!("PROG/P");
        assert!(serde_json::from_value::<ObjectKey<FunctionModule>>(wrong_parent_type).is_err());

        let mut primary_with_parent =
            serde_json::to_value(ObjectKey::<Program>::new("Z_TEST")).unwrap();
        primary_with_parent["parent"] = serialized["parent"].clone();
        assert!(serde_json::from_value::<ObjectKey<Program>>(primary_with_parent).is_err());

        let mut detached_child = serialized;
        detached_child.as_object_mut().unwrap().remove("parent");
        assert!(serde_json::from_value::<ObjectKey<FunctionModule>>(detached_child).is_ok());
    }
}
