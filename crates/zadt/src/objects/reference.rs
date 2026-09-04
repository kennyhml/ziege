use std::{collections::HashMap, fmt, hash::Hash, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize};
use stduritemplate::Value;

use super::{
    GlobalWorkbenchType, ObjectIdentity, ObjectType, PrimaryObjectType, SubObject, descriptors,
};
use crate::{Discovery, ResolveError, error::ObjectError, resource::AdtUriTemplate, uri::AdtUri};

/// A logical reference to an ADT object.
///
/// Unlike [`crate::ObjectSnapshot<T>`], this value does not include loaded properties.
/// The type parameter `T` selects the operations available for that object
/// family.
///
/// [`ObjectRef<()>`] stores the object family at runtime. It is useful when the
/// family comes from user input or a repository response.
///
/// A [`Discovery`] resolves the object when an operation is encoded. Child
/// references retain their parent so the relationship template can be selected
/// without caching discovery state on the reference.
#[derive(Debug, Serialize)]
pub struct ObjectRef<T = ()> {
    /// The full name of the object
    name: String,

    /// The workbench type of the object
    object_type: GlobalWorkbenchType,

    /// An optional parent of this object, if it has one
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<Box<ObjectRef<()>>>,

    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

/// An object reference whose URI and any parent URI have been resolved.
///
/// This type is primarily used internally while encoding operations that must
/// embed a complete object identity in their payload.
#[doc(hidden)]
#[derive(Debug)]
pub struct ResolvedObjectRef<T = ()> {
    reference: ObjectRef<T>,
    uri: AdtUri,
    parent: Option<Box<ResolvedObjectRef<()>>>,
}

impl<T> ResolvedObjectRef<T> {
    /// Returns the logical object reference.
    pub fn reference(&self) -> &ObjectRef<T> {
        &self.reference
    }

    /// Returns the resolved object URI.
    pub fn uri(&self) -> &AdtUri {
        &self.uri
    }

    /// Returns the fully resolved parent reference, when this is a subobject.
    pub fn parent(&self) -> Option<&ResolvedObjectRef<()>> {
        self.parent.as_deref()
    }

    pub(crate) fn parent_reference(&self) -> Option<AdvertisedObjectReference> {
        self.parent().map(AdvertisedObjectReference::from)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        reference: ObjectRef<T>,
        uri: AdtUri,
        parent: Option<ResolvedObjectRef<()>>,
    ) -> Self {
        Self {
            reference,
            uri,
            parent: parent.map(Box::new),
        }
    }
}

impl<T> Clone for ResolvedObjectRef<T> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            uri: self.uri.clone(),
            parent: self.parent.clone(),
        }
    }
}

impl<T> ObjectRef<T> {
    pub(crate) fn from_parts(
        name: String,
        object_type: GlobalWorkbenchType,
        parent: Option<Box<ObjectRef<()>>>,
    ) -> Self {
        Self {
            name,
            object_type,
            parent,
            marker: PhantomData,
        }
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
            object_type: self.object_type.clone(),
            parent: self.parent.clone(),
            marker: PhantomData,
        }
    }

    pub(crate) fn descriptor(&self) -> Option<&'static descriptors::ObjectTypeDescriptor> {
        descriptors::object_type_descriptor(&self.object_type)
    }

    pub(crate) fn require_descriptor(
        &self,
    ) -> Result<&'static descriptors::ObjectTypeDescriptor, ObjectError> {
        self.descriptor()
            .ok_or_else(|| ObjectError::UnsupportedObjectType {
                object_type: self.object_type().clone(),
            })
    }

    pub(crate) fn same_identity<U>(&self, other: &ObjectRef<U>) -> bool {
        self.name == other.name
            && self.object_type == other.object_type
            && self.parent == other.parent
    }

    pub(crate) fn unsupported_capability(&self, capability: &'static str) -> ObjectError {
        ObjectError::UnsupportedCapability {
            object_type: self.object_type.clone(),
            capability,
        }
    }

    /// Returns the logical parent identity for a subobject.
    pub fn parent(&self) -> Option<&ObjectRef<()>> {
        self.parent.as_deref()
    }
}

impl<T: PrimaryObjectType> ObjectRef<T> {
    /// Creates a logical primary-object reference.
    pub fn new(name: impl Into<String>) -> Self {
        Self::from_parts(name.into().to_ascii_uppercase(), T::WORKBENCH_TYPE, None)
    }
}

impl ObjectRef<()> {
    /// Creates a logical primary-object reference from a Workbench type.
    ///
    /// Subobjects require a parent and must instead be created through
    /// [`ObjectRef::subobject`].
    pub fn from_workbench_type(
        object_type: &GlobalWorkbenchType,
        name: impl Into<String>,
    ) -> Result<Self, ObjectError> {
        let descriptor = descriptors::object_type_descriptor(object_type).ok_or_else(|| {
            ObjectError::UnsupportedObjectType {
                object_type: object_type.clone(),
            }
        })?;

        descriptor
            .category()
            .ok_or_else(|| ObjectError::ParentObjectRequired {
                object_type: object_type.clone(),
            })?;

        Ok(Self::from_parts(
            name.into().to_ascii_uppercase(),
            object_type.clone(),
            None,
        ))
    }

    /// Recovers a typed reference when this object has the requested type.
    pub fn typed<T: ObjectType>(&self) -> Option<ObjectRef<T>> {
        if self.object_type() != &T::WORKBENCH_TYPE {
            return None;
        }
        Some(self.retag())
    }
}

impl<T> Clone for ObjectRef<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            object_type: self.object_type.clone(),
            parent: self.parent.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> ObjectIdentity for ObjectRef<T> {
    fn object_name(&self) -> &str {
        self.name()
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        self.object_type()
    }
}

impl<T> ObjectIdentity for ResolvedObjectRef<T> {
    fn object_name(&self) -> &str {
        self.reference.name()
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        self.reference.object_type()
    }
}

impl<T> PartialEq for ObjectRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}

impl<T> Eq for ObjectRef<T> {}

impl<T> Hash for ObjectRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.object_type.hash(state);
        self.parent.hash(state);
    }
}

impl<T> fmt::Display for ObjectRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.name, self.object_type)
    }
}

/// Temporary Serde value used while deserializing an [`ObjectRef`].
///
/// For typed references, the Workbench type is checked before the reference is
/// constructed.
#[derive(Deserialize)]
struct RawObjectRef {
    name: String,
    object_type: GlobalWorkbenchType,
    #[serde(default)]
    parent: Option<Box<ObjectRef<()>>>,
}

impl<'de> Deserialize<'de> for ObjectRef<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let reference = RawObjectRef::deserialize(deserializer)?;
        validate_parent_identity(&reference).map_err(serde::de::Error::custom)?;
        Ok(Self::from_parts(
            reference.name,
            reference.object_type,
            reference.parent,
        ))
    }
}

impl<'de, T: ObjectType> Deserialize<'de> for ObjectRef<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let reference = RawObjectRef::deserialize(deserializer)?;
        if reference.object_type != T::WORKBENCH_TYPE {
            return Err(serde::de::Error::custom(
                ObjectError::UnexpectedObjectType {
                    expected: T::WORKBENCH_TYPE,
                    actual: reference.object_type,
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

fn validate_parent_identity(reference: &RawObjectRef) -> Result<(), ObjectError> {
    let Some(parent) = &reference.parent else {
        return Ok(());
    };
    if !descriptors::requires_parent(&reference.object_type) {
        return Err(ObjectError::InvalidParentObject {
            object_type: reference.object_type.clone(),
            reason: "the object type is directly addressable".to_owned(),
        });
    }
    if !descriptors::supports_subobject(&parent.object_type, &reference.object_type) {
        return Err(ObjectError::InvalidParentObject {
            object_type: reference.object_type.clone(),
            reason: format!(
                "type `{}` does not declare this subobject relationship",
                parent.object_type
            ),
        });
    }
    Ok(())
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
            object_type: Some(value.object_type.clone()),
            name: Some(value.name.clone()),
            ..Default::default()
        }
    }
}

impl<T> From<&ResolvedObjectRef<T>> for AdvertisedObjectReference {
    fn from(value: &ResolvedObjectRef<T>) -> Self {
        Self {
            uri: Some(value.uri().to_string()),
            object_type: Some(value.reference.object_type.clone()),
            name: Some(value.reference.name.clone()),
            parent_uri: value.parent().map(|parent| parent.uri().to_string()),
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

pub(crate) struct ResolvedObjectCollection {
    pub(crate) target: AdtUri,
    pub(crate) accepted_media_types: Vec<String>,
}

impl Discovery {
    /// Resolves an object and its parent to references with concrete URIs.
    pub fn resolve_object<T>(
        &self,
        object: &ObjectRef<T>,
    ) -> Result<ResolvedObjectRef<T>, ResolveError> {
        let uri = self.resolve_object_uri(object)?;
        let parent = object
            .parent()
            .map(|parent| self.resolve_object(parent))
            .transpose()?
            .map(Box::new);
        Ok(ResolvedObjectRef {
            reference: object.clone(),
            uri,
            parent,
        })
    }

    /// Resolves an object's concrete URI through discovery.
    pub fn resolve_object_uri<T>(&self, object: &ObjectRef<T>) -> Result<AdtUri, ResolveError> {
        let collection = self.resolve_object_collection(object)?;
        collection
            .target
            .append_segments([object.name().to_ascii_lowercase()])
            .map_err(ObjectError::InvalidTarget)
            .map_err(Into::into)
    }

    pub(crate) fn resolve_object_collection<T>(
        &self,
        object: &ObjectRef<T>,
    ) -> Result<ResolvedObjectCollection, ResolveError> {
        if let Some(parent) = object.parent() {
            let parent_descriptor = parent.require_descriptor()?;
            let category =
                parent_descriptor
                    .category()
                    .ok_or_else(|| ObjectError::InvalidParentObject {
                        object_type: object.object_type().clone(),
                        reason: format!(
                            "parent type `{}` is not directly addressable",
                            parent.object_type()
                        ),
                    })?;
            let relationship = parent_descriptor
                .subobjects()
                .iter()
                .find(|candidate| candidate.object_type() == object.object_type())
                .ok_or_else(|| ObjectError::UnsupportedSubObjectType {
                    parent_type: parent.object_type().clone(),
                    child_type: object.object_type().clone(),
                })?;
            let link = self.require_template(category, relationship.relation())?;
            let template = AdtUriTemplate::new(link.template());
            if template.variable_names() != [relationship.parent_variable()] {
                return Err(ObjectError::MissingTemplate {
                    relation: relationship.relation(),
                }
                .into());
            }
            let variables = HashMap::from([(
                relationship.parent_variable().to_owned(),
                Value::String(parent.name().to_ascii_lowercase()),
            )]);
            let (target, query) = template.expand(&variables)?;
            if !query.is_empty() {
                return Err(ObjectError::MissingTemplate {
                    relation: relationship.relation(),
                }
                .into());
            }
            return Ok(ResolvedObjectCollection {
                target,
                accepted_media_types: link.media_type().map(str::to_owned).into_iter().collect(),
            });
        }

        let descriptor = object.require_descriptor()?;
        let category = descriptor
            .category()
            .ok_or_else(|| ObjectError::ParentObjectRequired {
                object_type: object.object_type().clone(),
            })?;
        let collection = self.require_collection(category)?;
        Ok(ResolvedObjectCollection {
            target: collection.target().map_err(ObjectError::InvalidTarget)?,
            accepted_media_types: collection.accepted_media_types().to_vec(),
        })
    }
}

impl<P: PrimaryObjectType> ObjectRef<P> {
    /// Creates a logical subobject reference belonging to this primary object.
    ///
    /// [`SubObject<C>`] guarantees that the returned object reference has a sub-
    /// object relationship with `T` and the relationship lookup is infallible.
    ///
    /// The parent reference is retained for later discovery-based resolution.
    pub fn subobject<C>(&self, name: impl Into<String>) -> ObjectRef<C>
    where
        C: ObjectType,
        P: SubObject<C>,
    {
        ObjectRef::from_parts(
            name.into().to_ascii_uppercase(),
            C::WORKBENCH_TYPE,
            Some(Box::new(self.erase())),
        )
    }
}

impl ObjectRef<()> {
    /// Creates a logical subobject reference belonging to this primary object.
    ///
    /// Because the object type is erased, there are no static guarantees that
    /// this object has any of the requested subobjects or even any at all. This
    /// is instead turned into a descriptor backed runtime check.
    ///
    /// The parent reference is retained for later discovery-based resolution.
    pub fn subobject(
        &self,
        child_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<ObjectRef<()>, ObjectError> {
        let descriptor = self.require_descriptor()?;
        descriptor
            .subobjects()
            .iter()
            .find(|subobject| subobject.object_type() == child_type)
            .ok_or_else(|| ObjectError::UnsupportedSubObjectType {
                parent_type: self.object_type().clone(),
                child_type: child_type.clone(),
            })?;

        Ok(ObjectRef::from_parts(
            name.to_ascii_uppercase(),
            child_type.clone(),
            Some(Box::new(self.clone())),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessMode, FunctionGroup, FunctionModule, Include, Program};

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
        let program = ObjectRef::<Program>::new("Z_TEST");
        let object = program.erase();

        assert_eq!(object.lock(AccessMode::Modify).object, object);
        assert_eq!(object.object_type().as_str(), "PROG/P");
        assert_eq!(object.typed::<Program>(), Some(program));
        assert!(object.typed::<Include>().is_none());
    }

    #[test]
    fn typed_reference_deserialization_validates_its_marker() {
        let program = ObjectRef::<Program>::new("Z_TEST");
        let serialized = serde_json::to_value(&program).unwrap();

        assert!(serialized.get("uri").is_none());
        assert!(serde_json::from_value::<ObjectRef<Program>>(serialized.clone()).is_ok());
        assert!(serde_json::from_value::<ObjectRef<crate::Class>>(serialized).is_err());
    }

    #[test]
    fn reference_deserialization_validates_parent_metadata() {
        let group = ObjectRef::<FunctionGroup>::new("Z_TEST_GROUP");
        let module = group.subobject::<FunctionModule>("ZZZZFUNC");
        let serialized = serde_json::to_value(&module).unwrap();

        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(serialized.clone()).is_ok());

        let mut wrong_parent_type = serialized.clone();
        wrong_parent_type["parent"]["object_type"] = serde_json::json!("PROG/P");
        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(wrong_parent_type).is_err());

        let mut primary_with_parent =
            serde_json::to_value(ObjectRef::<Program>::new("Z_TEST")).unwrap();
        primary_with_parent["parent"] = serialized["parent"].clone();
        assert!(serde_json::from_value::<ObjectRef<Program>>(primary_with_parent).is_err());

        let mut detached_child = serialized;
        detached_child.as_object_mut().unwrap().remove("parent");
        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(detached_child).is_ok());
    }

    #[test]
    fn unmodeled_erased_reference_retains_runtime_type() {
        let object: ObjectRef<()> =
            ObjectRef::from_parts("Z_UNSUPPORTED".to_owned(), "TEST/X".parse().unwrap(), None);

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
