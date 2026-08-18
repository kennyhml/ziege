use std::{fmt, hash::Hash, marker::PhantomData};

use serde::{Deserialize, Serialize};

use super::{GlobalWorkbenchType, ObjectType, descriptors};
use crate::{
    client::{Client, Ready},
    error::ObjectError,
    uri::AdtUri,
    vocabulary::CategoryId,
};

/// A validated ADT repository-object identity with static or runtime type state.
///
/// Typed references obtain capabilities from `T`. [`ObjectRef<()>`] retains the
/// exact runtime Workbench type for runtime capability dispatch.
#[derive(Debug, Deserialize, Serialize)]
#[serde(bound = "")]
pub struct ObjectRef<T = ()> {
    name: String,
    uri: AdtUri,
    object_type: GlobalWorkbenchType,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
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

    /// Returns the exact Workbench type retained by this reference.
    pub fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }

    /// Returns a runtime-typed copy of this object identity.
    pub fn erase(&self) -> ObjectRef<()> {
        self.retag()
    }

    pub(crate) fn retag<U>(&self) -> ObjectRef<U> {
        ObjectRef {
            name: self.name.clone(),
            uri: self.uri.clone(),
            object_type: self.object_type.clone(),
            marker: PhantomData,
        }
    }

    pub(crate) fn descriptor(
        &self,
    ) -> Option<&'static dyn descriptors::RuntimeObjectTypeDescriptor> {
        descriptors::object_type_descriptor(&self.object_type)
    }

    pub(crate) fn same_identity<U>(&self, other: &ObjectRef<U>) -> bool {
        self.uri == other.uri && self.name == other.name && self.object_type == other.object_type
    }

    pub(crate) fn unsupported_capability(&self, capability: &'static str) -> ObjectError {
        ObjectError::UnsupportedCapability {
            object_type: self.object_type.clone(),
            capability,
        }
    }
}

impl<T: ObjectType> ObjectRef<T> {
    pub(crate) fn new(name: String, uri: AdtUri) -> Self {
        Self {
            name,
            uri,
            object_type: T::WORKBENCH_TYPE,
            marker: PhantomData,
        }
    }
}

impl ObjectRef<()> {
    pub(crate) fn erased(name: String, uri: AdtUri, object_type: GlobalWorkbenchType) -> Self {
        Self {
            name,
            uri,
            object_type,
            marker: PhantomData,
        }
    }

    /// Recovers a typed reference when this object has the requested type.
    pub fn typed<T: ObjectType>(&self) -> Option<ObjectRef<T>> {
        if self.object_type() != &T::WORKBENCH_TYPE {
            return None;
        }
        Some(ObjectRef::new(self.name.clone(), self.uri.clone()))
    }
}

impl<T> Clone for ObjectRef<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            uri: self.uri.clone(),
            object_type: self.object_type.clone(),
            marker: PhantomData,
        }
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

/// An unresolved object reference exactly as advertised in an ADT payload.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdvertisedObjectReference {
    /// The referenced object's URI, when advertised.
    #[serde(rename = "@adtcore:uri", skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// The referenced object's global Workbench type, when advertised.
    #[serde(rename = "@adtcore:type", skip_serializing_if = "Option::is_none")]
    pub object_type: Option<GlobalWorkbenchType>,

    /// The referenced object's name, when advertised.
    #[serde(rename = "@adtcore:name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The referenced object's package name, when advertised.
    #[serde(
        rename = "@adtcore:packageName",
        skip_serializing_if = "Option::is_none"
    )]
    pub package_name: Option<String>,

    /// The referenced object's description, when advertised.
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,

    /// The owning object's URI when this reference identifies a subobject.
    #[serde(rename = "@adtcore:parentUri", skip_serializing_if = "Option::is_none")]
    pub parent_uri: Option<String>,
}

impl<T> From<&ObjectRef<T>> for AdvertisedObjectReference {
    fn from(value: &ObjectRef<T>) -> Self {
        Self {
            uri: Some(value.uri().as_str().to_owned()),
            object_type: Some(value.object_type.clone()),
            name: Some(value.name.clone()),
            ..Default::default()
        }
    }
}

impl From<String> for AdvertisedObjectReference {
    fn from(name: String) -> Self {
        Self {
            name: Some(name),
            ..Default::default()
        }
    }
}

impl From<&str> for AdvertisedObjectReference {
    fn from(name: &str) -> Self {
        name.to_owned().into()
    }
}

/// A collection of unresolved object references exactly as advertised by ADT.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "adtcore:objectReferences")]
pub struct ObjectReferences {
    /// Object references in response order.
    #[serde(rename = "adtcore:objectReference", default)]
    pub objects: Vec<AdvertisedObjectReference>,
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
        let uri = collection.target()?.append_segments([&uri_name])?;
        Ok((name, uri))
    }

    /// Resolves a typed object reference from its statically known collection.
    pub fn object<T: ObjectType>(&self, name: &str) -> Result<ObjectRef<T>, ObjectError> {
        self.object_identity(T::CATEGORY, name)
            .map(|(name, uri)| ObjectRef::new(name, uri))
    }

    /// Resolves a runtime object reference from its Workbench type and name.
    pub fn repository_object(
        &self,
        object_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<ObjectRef<()>, ObjectError> {
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
    use crate::{AccessMode, Include, Program};

    #[test]
    fn advertised_object_references_preserve_partial_wire_values() {
        let xml = r#"<adtcore:objectRef adtcore:type="CLAS/OC" adtcore:name="ZCL_TEST" adtcore:packageName="ZPACKAGE" xmlns:adtcore="http://www.sap.com/adt/core" />"#;
        let reference: AdvertisedObjectReference = serde_xml_rs::from_str(xml).unwrap();

        assert_eq!(reference.object_type.as_ref().unwrap().as_str(), "CLAS/OC");
        assert_eq!(reference.name.as_deref(), Some("ZCL_TEST"));
        assert_eq!(reference.package_name.as_deref(), Some("ZPACKAGE"));
        assert!(reference.uri.is_none());

        let json = serde_json::to_value(&reference).unwrap();
        assert_eq!(json["@adtcore:type"], "CLAS/OC");
        assert_eq!(json["@adtcore:packageName"], "ZPACKAGE");
        assert!(json.get("@adtcore:uri").is_none());
        assert_eq!(
            serde_json::from_value::<AdvertisedObjectReference>(json).unwrap(),
            reference
        );
    }

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
