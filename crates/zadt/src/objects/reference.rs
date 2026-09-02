use std::{collections::HashMap, fmt, hash::Hash, marker::PhantomData, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize};
use stduritemplate::Value;

use super::{
    GlobalWorkbenchType, ObjectIdentity, ObjectType, PrimaryObjectType, SubObject,
    SubObjectDescriptor, descriptors,
};
use crate::{
    CategoryId,
    client::{Client, Ready},
    error::ObjectError,
    resource::AdtUriTemplate,
    uri::AdtUri,
};

/// A reference to an ADT object with its name, URI, and Workbench type.
///
/// Unlike [`crate::ObjectSnapshot<T>`], this value does not include loaded properties.
/// The type parameter `T` selects the operations available for that object
/// family.
///
/// [`ObjectRef<()>`] stores the object family at runtime. It is useful when the
/// family comes from user input or a repository response.
///
/// Primary references created by [`Client::object`] also retain any subobject
/// targets resolved from discovery. Those targets are transient and are not
/// serialized. Child references retain and serialize their parent identity so
/// response validation and operations such as activation remain stable.
#[derive(Debug, Serialize)]
#[serde(bound = "")]
pub struct ObjectRef<T = ()> {
    name: String,
    uri: AdtUri,
    object_type: GlobalWorkbenchType,

    /// Describes which sub-objects this object type may carry
    #[serde(skip)]
    subobjects: Option<Arc<HashMap<GlobalWorkbenchType, AdtUri>>>,

    /// An optional parent of this object, if it has one
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<ParentObjectIdentity>,

    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ParentObjectIdentity {
    name: String,
    uri: AdtUri,
    object_type: GlobalWorkbenchType,
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
            subobjects: self.subobjects.clone(),
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
        self.uri == other.uri && self.name == other.name && self.object_type == other.object_type
    }

    pub(crate) fn unsupported_capability(&self, capability: &'static str) -> ObjectError {
        ObjectError::UnsupportedCapability {
            object_type: self.object_type.clone(),
            capability,
        }
    }

    fn with_subobjects(mut self, subobjects: HashMap<GlobalWorkbenchType, AdtUri>) -> Self {
        self.subobjects = Some(Arc::new(subobjects));
        self
    }

    pub(crate) fn with_parent<U>(mut self, parent: &ObjectRef<U>) -> Self {
        self.parent = Some(ParentObjectIdentity {
            name: parent.name.clone(),
            uri: parent.uri.clone(),
            object_type: parent.object_type.clone(),
        });
        self
    }

    pub(crate) fn parent_identity(&self) -> Option<(&str, &AdtUri, &GlobalWorkbenchType)> {
        self.parent
            .as_ref()
            .map(|parent| (parent.name.as_str(), &parent.uri, &parent.object_type))
    }
}

impl<T: ObjectType> ObjectRef<T> {
    pub(crate) fn new(name: String, uri: AdtUri) -> Self {
        Self {
            name,
            uri,
            object_type: T::WORKBENCH_TYPE,
            subobjects: None,
            parent: None,
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
            subobjects: None,
            parent: None,
            marker: PhantomData,
        }
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
            uri: self.uri.clone(),
            object_type: self.object_type.clone(),
            subobjects: self.subobjects.clone(),
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

/// Temporary Serde value used while deserializing an [`ObjectRef`].
///
/// For typed references, the Workbench type is checked before the reference is
/// constructed.
#[derive(Deserialize)]
struct RawObjectRef {
    name: String,
    uri: AdtUri,
    object_type: GlobalWorkbenchType,
    #[serde(default)]
    parent: Option<ParentObjectIdentity>,
}

impl<'de> Deserialize<'de> for ObjectRef<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let reference = RawObjectRef::deserialize(deserializer)?;
        validate_parent_identity(&reference).map_err(serde::de::Error::custom)?;
        let mut object = Self::erased(reference.name, reference.uri, reference.object_type);
        object.parent = reference.parent;
        Ok(object)
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
        let mut object = Self::new(reference.name, reference.uri);
        object.parent = reference.parent;
        Ok(object)
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
    if !reference.uri.is_descendant_of(&parent.uri)
        || reference.uri.as_str()[parent.uri.as_str().len()..]
            .split('/')
            .filter(|segment| !segment.is_empty())
            .count()
            != 2
    {
        return Err(ObjectError::InvalidParentObject {
            object_type: reference.object_type.clone(),
            reason: format!(
                "URI `{}` is not a direct subobject of `{}`",
                reference.uri, parent.uri
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
            uri: Some(value.uri().as_str().to_owned()),
            object_type: Some(value.object_type.clone()),
            name: Some(value.name.clone()),
            parent_uri: value
                .parent_identity()
                .map(|(_, uri, _)| uri.as_str().to_owned()),
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
    /// Loads all parts of an object identity by its category in the discovery.
    ///
    /// This includes checking all associated sub-object relationships and finding
    /// the advertised template for each of them, substituting the primary object
    /// in the template and then storing those uris in the returned reference.
    ///
    /// The resolved sub-objects map each sub-object workbench type to an [`AdtUri`]:
    /// `FUGR/FF` -> `/sap/bc/adt/functions/groups/zgroup/fmodules`
    /// `FUGR/I` -> `/sap/bc/adt/functions/groups/zgroup/includes`
    ///
    /// Both typed and erased objects can use this path, they only differ in where
    /// the sub object descriptors come from (static vs descriptor).
    fn resolve_object_identity(
        &self,
        category: CategoryId,
        name: &str,
        subobjects: &[SubObjectDescriptor],
    ) -> Result<(String, AdtUri, HashMap<GlobalWorkbenchType, AdtUri>), ObjectError> {
        let name = name.to_ascii_uppercase();
        let uri_name = name.to_ascii_lowercase();
        let collection = self.require_collection(category)?;

        // Base URI of the primary object
        let uri = collection.target()?.append_segments([&uri_name])?;
        let templates = collection.template_links();

        let mut resolved_subobjects = HashMap::new();
        for subobject in subobjects {
            let Some(link) = templates
                .iter()
                .find(|link| link.relation() == subobject.relation())
            else {
                // no template found for this sub-object, which is weird
                // and hints towards an incompatible API, but dont consider
                // this an error yet until its actually used
                continue;
            };

            // We expect the template to have exactly one variable name for the
            // parent object, e.g. `/sap/bc/adt/functions/groups/{fgroup}/fmodules`
            let template = AdtUriTemplate::new(link.template());
            if template.variable_names() != [subobject.parent_variable()] {
                continue;
            }

            // Substitute the variable to create a concrete adt uri, such
            // as `/sap/bc/adt/functions/groups/zgroup/fmodules` for FUGR.
            let variables = HashMap::from([(
                subobject.parent_variable().to_owned(),
                Value::String(uri_name.clone()),
            )]);
            let (target, query) = template.expand(&variables)?;

            // query parameters are not expected / supported
            if query.is_empty() {
                resolved_subobjects.insert(subobject.object_type().clone(), target);
            }
        }
        Ok((name, uri, resolved_subobjects))
    }

    /// Resolves a primary object reference from its statically known collection.
    ///
    /// This resolves from the client because some system discovery knowledge is
    /// required in order to resolve the base [`AdtUri`] of the object collection
    /// as well as locating and resolving any subobject relationship templates.
    pub fn object<T: PrimaryObjectType>(&self, name: &str) -> Result<ObjectRef<T>, ObjectError> {
        let (name, uri, subobjects) = self.resolve_object_identity(
            T::CATEGORY,
            name,
            <T as super::private::PrimaryMetadata>::SUBOBJECTS,
        )?;

        Ok(ObjectRef::new(name, uri).with_subobjects(subobjects))
    }

    /// Resolves an object reference from a dynamically specified workbench type
    /// and name. This method can make no static guarantees that the object is
    /// a primary object and can actually be resolved through this method.
    ///
    /// If the object is actually a subobject, such as `FUGR/FF`, it must be
    /// resolved from its parent object `FUGR/F` instead.
    ///
    /// The result is the same as when calling [`Self::object<T>`] except
    /// that the type tag is erased.
    pub fn object_from_wb_type(
        &self,
        object_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<ObjectRef<()>, ObjectError> {
        let descriptor = descriptors::object_type_descriptor(object_type).ok_or_else(|| {
            ObjectError::UnsupportedObjectType {
                object_type: object_type.clone(),
            }
        })?;

        let category = descriptor
            .category()
            .ok_or_else(|| ObjectError::ParentObjectRequired {
                object_type: object_type.clone(),
            })?;

        let (name, uri, subobjects) =
            self.resolve_object_identity(category, name, descriptor.subobjects())?;
        Ok(ObjectRef::erased(name, uri, object_type.clone()).with_subobjects(subobjects))
    }
}

impl<P: PrimaryObjectType> ObjectRef<P> {
    /// Resolves the location of a subobject belonging to this primary object.
    ///
    /// [`SubObject<C>`] guarantees that the returned object reference has a sub-
    /// object relationship with `T` and the relationship lookup is infallible.
    ///
    /// Constructing the subobject may still fail when the server does not
    /// advertise the template needed to resolve the relationship.
    ///
    /// The parent reference is implicitly added to the returnd child.
    pub fn subobject<C>(&self, name: &str) -> Result<ObjectRef<C>, ObjectError>
    where
        C: ObjectType,
        P: SubObject<C>,
    {
        let descriptor = <P as SubObject<C>>::DESCRIPTOR;
        let uri = self.subobject_uri(&descriptor, name)?;
        Ok(ObjectRef::new(name.to_ascii_uppercase(), uri).with_parent(self))
    }
}

impl ObjectRef<()> {
    /// Resolves the location of a subobject belonging to this primary object.
    ///
    /// Because the object type is erased, there are no static guarantees that
    /// this object has any of the requested subobjects or even any at all. This
    /// is instead turned into a descriptor backed runtime check.
    ///
    /// Constructing the subobject may also fail when the server does not
    /// advertise the template needed to resolve the relationship.
    ///
    /// The parent reference is implicitly added to the returnd child.
    pub fn subobject(
        &self,
        child_type: &GlobalWorkbenchType,
        name: &str,
    ) -> Result<ObjectRef<()>, ObjectError> {
        let descriptor = self.require_descriptor()?;
        let subobject = descriptor
            .subobjects()
            .iter()
            .find(|subobject| subobject.object_type() == child_type)
            .ok_or_else(|| ObjectError::UnsupportedSubObjectType {
                parent_type: self.object_type().clone(),
                child_type: child_type.clone(),
            })?;

        let uri = self.subobject_uri(subobject, name)?;
        Ok(ObjectRef::erased(name.to_ascii_uppercase(), uri, child_type.clone()).with_parent(self))
    }
}

impl<T> ObjectRef<T> {
    fn subobject_uri(
        &self,
        descriptor: &SubObjectDescriptor,
        name: &str,
    ) -> Result<AdtUri, ObjectError> {
        let base = self
            .subobjects
            .as_deref()
            .and_then(|subobjects| subobjects.get(descriptor.object_type()))
            .ok_or(ObjectError::MissingTemplate {
                relation: descriptor.relation(),
            })?;

        Ok(base.append_segments([name.to_ascii_lowercase()])?)
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
    fn typed_reference_deserialization_validates_its_marker() {
        let program = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/Z_TEST").unwrap(),
        );
        let serialized = serde_json::to_value(&program).unwrap();

        assert!(serde_json::from_value::<ObjectRef<Program>>(serialized.clone()).is_ok());
        assert!(serde_json::from_value::<ObjectRef<crate::Class>>(serialized).is_err());
    }

    #[test]
    fn reference_deserialization_validates_parent_metadata() {
        let group = ObjectRef::<FunctionGroup>::new(
            "Z_TEST_GROUP".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group").unwrap(),
        );
        let module = ObjectRef::<FunctionModule>::new(
            "ZZZZFUNC".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc").unwrap(),
        )
        .with_parent(&group);
        let serialized = serde_json::to_value(&module).unwrap();

        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(serialized.clone()).is_ok());

        let mut wrong_parent_type = serialized.clone();
        wrong_parent_type["parent"]["object_type"] = serde_json::json!("PROG/P");
        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(wrong_parent_type).is_err());

        let mut wrong_parent_uri = serialized.clone();
        wrong_parent_uri["parent"]["uri"] = serde_json::json!("/sap/bc/adt/functions");
        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(wrong_parent_uri).is_err());

        let mut primary_with_parent = serde_json::to_value(ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
        ))
        .unwrap();
        primary_with_parent["parent"] = serialized["parent"].clone();
        assert!(serde_json::from_value::<ObjectRef<Program>>(primary_with_parent).is_err());

        let mut detached_child = serialized;
        detached_child.as_object_mut().unwrap().remove("parent");
        assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(detached_child).is_ok());
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
