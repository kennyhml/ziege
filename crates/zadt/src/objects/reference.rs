use std::{collections::HashMap, fmt, hash::Hash};

use serde::{Deserialize, Serialize};
use stduritemplate::Value;

use super::{GlobalWorkbenchType, Identity, ObjectKey, ObjectType, descriptors};
use crate::{Discovery, ResolveError, error::ObjectError, resource::AdtUriTemplate, uri::AdtUri};

/// An ADT object at a concrete, validated resource URI.
///
/// The logical key retains any known parent identity; `parent_uri` independently
/// retains an advertised or resolved immediate parent URI. Parent metadata does
/// not participate in reference equality, which compares name, type, and URI.
#[derive(Debug, Deserialize, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "", deserialize = "ObjectKey<T>: Deserialize<'de>")
)]
pub struct ObjectRef<T = ()> {
    key: ObjectKey<T>,
    uri: AdtUri,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_uri: Option<AdtUri>,
}

impl<T> ObjectRef<T> {
    /// Locates a logical key at a validated URI without consulting discovery.
    pub fn new(key: ObjectKey<T>, uri: AdtUri) -> Self {
        Self {
            key,
            uri,
            parent_uri: None,
        }
    }

    /// Attaches immediate parent location metadata without changing identity.
    #[must_use]
    pub fn with_parent_uri(mut self, uri: AdtUri) -> Self {
        self.parent_uri = Some(uri);
        self
    }

    /// Returns the logical object key, including any known parent identity.
    pub fn key(&self) -> &ObjectKey<T> {
        &self.key
    }

    /// Returns the retained object URI.
    pub fn uri(&self) -> &AdtUri {
        &self.uri
    }

    /// Returns the advertised or resolved immediate parent URI, when known.
    pub fn parent_uri(&self) -> Option<&AdtUri> {
        self.parent_uri.as_ref()
    }

    /// Returns the object name.
    pub fn name(&self) -> &str {
        self.key.name()
    }

    /// Returns the exact Workbench type retained by this reference.
    pub fn workbench_type(&self) -> &GlobalWorkbenchType {
        self.key.workbench_type()
    }

    /// Returns a runtime-typed copy, preserving all location metadata.
    pub fn erase(&self) -> ObjectRef<()> {
        ObjectRef {
            key: self.key.erase(),
            uri: self.uri.clone(),
            parent_uri: self.parent_uri.clone(),
        }
    }

    pub(crate) fn require_descriptor(
        &self,
    ) -> Result<&'static descriptors::ObjectTypeDescriptor, ObjectError> {
        self.key.require_descriptor()
    }

    pub(crate) fn unsupported_capability(&self, capability: &'static str) -> ObjectError {
        self.key.unsupported_capability(capability)
    }

    /// Resolves parent context only for operations whose protocol needs it.
    pub(crate) fn resolve_parent_uri(
        &self,
        discovery: &Discovery,
    ) -> Result<Option<AdtUri>, ResolveError> {
        if let Some(uri) = self.parent_uri() {
            return Ok(Some(uri.clone()));
        }
        self.key
            .parent()
            .map(|parent| discovery.resolve_object_uri(parent))
            .transpose()
    }

    pub(crate) fn parent_reference(&self) -> Option<AdvertisedObjectReference> {
        let mut parent = self
            .key
            .parent()
            .map(AdvertisedObjectReference::from)
            .or_else(|| {
                self.parent_uri
                    .as_ref()
                    .map(|_| AdvertisedObjectReference::default())
            })?;
        parent.uri = self.parent_uri.as_ref().map(ToString::to_string);
        Some(parent)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        reference: ObjectKey<T>,
        uri: AdtUri,
        parent: Option<ObjectRef<()>>,
    ) -> Self {
        let reference = Self::new(reference, uri);
        match parent {
            Some(parent) => reference.with_parent_uri(parent.uri),
            None => reference,
        }
    }
}

impl<T> Clone for ObjectRef<T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            uri: self.uri.clone(),
            parent_uri: self.parent_uri.clone(),
        }
    }
}

impl ObjectRef<()> {
    /// Recovers a typed reference, preserving its URI and parent metadata.
    pub fn typed<T: ObjectType>(&self) -> Option<ObjectRef<T>> {
        Some(ObjectRef {
            key: self.key.typed()?,
            uri: self.uri.clone(),
            parent_uri: self.parent_uri.clone(),
        })
    }
}

impl<T, U> PartialEq<ObjectRef<U>> for ObjectRef<T> {
    fn eq(&self, other: &ObjectRef<U>) -> bool {
        self.name() == other.name()
            && self.workbench_type() == other.workbench_type()
            && self.uri == other.uri
    }
}

impl<T> Eq for ObjectRef<T> {}

impl<T> Hash for ObjectRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name().hash(state);
        self.workbench_type().hash(state);
        self.uri.hash(state);
    }
}

impl<T> fmt::Display for ObjectRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.key, self.uri)
    }
}

impl<T> Identity for ObjectRef<T> {
    fn object_name(&self) -> &str {
        self.name()
    }

    fn workbench_type(&self) -> &GlobalWorkbenchType {
        self.workbench_type()
    }
}

/// A partial object reference exactly as advertised in an ADT payload.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdvertisedObjectReference {
    /// The referenced object's URI, when advertised.
    #[serde(rename = "@adtcore:uri", skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// The referenced object's global Workbench type, when advertised.
    #[serde(rename = "@adtcore:type", skip_serializing_if = "Option::is_none")]
    pub workbench_type: Option<GlobalWorkbenchType>,

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

impl<T> From<&ObjectKey<T>> for AdvertisedObjectReference {
    fn from(value: &ObjectKey<T>) -> Self {
        Self {
            workbench_type: Some(value.workbench_type().clone()),
            name: Some(value.name().to_owned()),
            ..Default::default()
        }
    }
}

impl<T> From<&ObjectRef<T>> for AdvertisedObjectReference {
    fn from(value: &ObjectRef<T>) -> Self {
        Self {
            uri: Some(value.uri().to_string()),
            workbench_type: Some(value.workbench_type().clone()),
            name: Some(value.name().to_owned()),
            parent_uri: value.parent_uri().map(ToString::to_string),
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

/// A collection of partial object references exactly as advertised by ADT.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "adtcore:objectReferences", deny_unknown_fields)]
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
    /// Resolves an object's URI and its immediate parent's URI through discovery.
    pub fn resolve_object<T>(&self, object: &ObjectKey<T>) -> Result<ObjectRef<T>, ResolveError> {
        let uri = self.resolve_object_uri(object)?;
        let parent_uri = object
            .parent()
            .map(|parent| self.resolve_object_uri(parent))
            .transpose()?;
        Ok(ObjectRef {
            key: object.clone(),
            uri,
            parent_uri,
        })
    }

    /// Resolves an object's concrete URI through discovery.
    pub fn resolve_object_uri<T>(&self, object: &ObjectKey<T>) -> Result<AdtUri, ResolveError> {
        let collection = self.resolve_object_collection(object)?;
        collection
            .target
            .append_segments([object.name().to_ascii_lowercase()])
            .map_err(ObjectError::InvalidTarget)
            .map_err(Into::into)
    }

    pub(crate) fn resolve_object_collection<T>(
        &self,
        object: &ObjectKey<T>,
    ) -> Result<ResolvedObjectCollection, ResolveError> {
        if let Some(parent) = object.parent() {
            let parent_descriptor = parent.require_descriptor()?;
            let category =
                parent_descriptor
                    .category()
                    .ok_or_else(|| ObjectError::InvalidParentObject {
                        workbench_type: object.workbench_type().clone(),
                        reason: format!(
                            "parent type `{}` is not directly addressable",
                            parent.workbench_type()
                        ),
                    })?;
            let relationship = parent_descriptor
                .subobjects()
                .iter()
                .find(|candidate| candidate.workbench_type() == object.workbench_type())
                .ok_or_else(|| ObjectError::UnsupportedSubObjectType {
                    parent_type: parent.workbench_type().clone(),
                    child_type: object.workbench_type().clone(),
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
                workbench_type: object.workbench_type().clone(),
            })?;
        let collection = self.require_collection(category)?;
        Ok(ResolvedObjectCollection {
            target: collection.target().map_err(ObjectError::InvalidTarget)?,
            accepted_media_types: collection.accepted_media_types().to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::ObjectTarget;
    use crate::{FunctionGroup, FunctionModule, Program};

    fn discovery(xml: &[u8]) -> Discovery {
        struct UnusedTransport;

        #[async_trait::async_trait]
        impl crate::Transport for UnusedTransport {
            async fn send(
                &self,
                _: crate::AdtRequest,
            ) -> Result<crate::AdtResponse, crate::TransportError> {
                unreachable!("core reference tests never send requests")
            }
        }

        crate::Client::new(UnusedTransport)
            .with_capabilities(
                crate::api::discovery::parse_capabilities(xml).unwrap(),
                crate::api::discovery::parse_capabilities(xml).unwrap(),
            )
            .discovery()
            .clone()
    }

    #[test]
    fn located_reference_serde_is_strict_and_validates_uris_and_marker() {
        let reference = ObjectRef::new(
            ObjectKey::<FunctionGroup>::new("z_group").subobject::<FunctionModule>("z_module"),
            AdtUri::parse("custom/module").unwrap(),
        )
        .with_parent_uri(AdtUri::parse("custom/group").unwrap());
        let original = serde_json::to_value(&reference).unwrap();
        assert_eq!(original["key"]["name"], "Z_MODULE");
        assert_eq!(original["uri"], "/sap/bc/adt/custom/module");
        assert_eq!(original["parent_uri"], "/sap/bc/adt/custom/group");
        let restored: ObjectRef<FunctionModule> = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(restored, reference);
        assert_eq!(restored.key(), reference.key());
        assert_eq!(restored.parent_uri(), reference.parent_uri());
        assert!(serde_json::from_value::<ObjectRef<Program>>(original.clone()).is_err());
        assert!(serde_json::from_value::<ObjectKey<FunctionModule>>(original.clone()).is_err());
        assert!(
            serde_json::from_value::<ObjectRef<FunctionModule>>(original["key"].clone()).is_err()
        );

        for pointer in ["", "/key", "/key/parent"] {
            let mut json = original.clone();
            json.pointer_mut(pointer).unwrap()["unexpected"] = true.into();
            assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(json.clone()).is_err());
            assert!(serde_json::from_value::<ObjectRef<()>>(json).is_err());
        }
        for field in ["key", "uri"] {
            let mut json = original.clone();
            json.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ObjectRef<()>>(json.clone()).is_err());
            json[field] = serde_json::Value::Null;
            assert!(serde_json::from_value::<ObjectRef<()>>(json).is_err());
        }
        for field in ["uri", "parent_uri"] {
            for invalid in [
                "",
                "https://example.com/sap/bc/adt/object",
                "/outside",
                "object?x=1",
            ] {
                let mut json = original.clone();
                json[field] = invalid.into();
                assert!(serde_json::from_value::<ObjectRef<FunctionModule>>(json.clone()).is_err());
                assert!(serde_json::from_value::<ObjectRef<()>>(json).is_err());
            }
        }
        let mut json = original.clone();
        json["key"]["parent"]["object_type"] = "PROG/P".into();
        assert!(serde_json::from_value::<ObjectRef<()>>(json).is_err());
        let mut detached = original;
        detached["key"].as_object_mut().unwrap().remove("parent");
        detached.as_object_mut().unwrap().remove("parent_uri");
        let detached: ObjectRef<FunctionModule> = serde_json::from_value(detached).unwrap();
        assert!(detached.key().parent().is_none());
        assert!(detached.parent_uri().is_none());
        assert!(
            serde_json::to_value(detached)
                .unwrap()
                .get("parent_uri")
                .is_none()
        );
        assert!(
            serde_json::from_str::<ObjectRef<()>>(
                r#"{"key":{"name":"Z","object_type":"PROG/P"},"uri":"one","uri":"two"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn located_identity_ignores_parent_metadata_but_includes_uri() {
        let first =
            ObjectKey::<FunctionGroup>::new("Z_FIRST").subobject::<FunctionModule>("Z_MODULE");
        let second =
            ObjectKey::<FunctionGroup>::new("Z_SECOND").subobject::<FunctionModule>("Z_MODULE");
        assert_ne!(first, second);
        let uri = AdtUri::parse("advertised/module").unwrap();
        let first = ObjectRef::new(first, uri.clone());
        let second = ObjectRef::new(second, uri)
            .with_parent_uri(AdtUri::parse("advertised/parent").unwrap());
        assert_eq!(first, second);
        assert_eq!(first, second.erase());
        assert_eq!(second.erase(), first);
        let mut identities = std::collections::HashSet::from([first.clone()]);
        assert!(!identities.insert(second));
        let other_uri = ObjectRef::new(first.key().clone(), AdtUri::parse("other/module").unwrap());
        assert_ne!(first, other_uri);
        assert_ne!(first, other_uri.erase());
        assert!(identities.insert(other_uri));
        let other_name = ObjectRef::new(
            ObjectKey::<FunctionGroup>::new("Z_FIRST").subobject::<FunctionModule>("Z_OTHER"),
            first.uri().clone(),
        );
        assert_ne!(first, other_name);
        assert_ne!(first, other_name.erase());
        let other_type = ObjectRef::new(ObjectKey::<Program>::new("Z_MODULE"), first.uri().clone());
        assert_ne!(first, other_type);
        assert_ne!(other_type, first);
    }

    #[test]
    fn located_conversions_preserve_key_and_advertised_parent_metadata() {
        let key =
            ObjectKey::<FunctionGroup>::new("Z_GROUP").subobject::<FunctionModule>("Z_MODULE");
        let reference = ObjectRef::new(key.clone(), AdtUri::parse("advertised/module").unwrap())
            .with_parent_uri(AdtUri::parse("advertised/group").unwrap());
        let erased = reference.erase();
        let typed = erased.typed::<FunctionModule>().unwrap();
        assert_eq!(typed.key(), &key);
        assert_eq!(typed.uri(), reference.uri());
        assert_eq!(typed.parent_uri(), reference.parent_uri());
        assert!(erased.typed::<Program>().is_none());
        let advertised = AdvertisedObjectReference::from(&erased);
        assert_eq!(advertised.uri.as_deref(), Some(reference.uri().as_str()));
        assert_eq!(
            advertised.parent_uri.as_deref(),
            Some("/sap/bc/adt/advertised/group")
        );
        let parent = reference.parent_reference().unwrap();
        assert_eq!(parent.name.as_deref(), Some("Z_GROUP"));
        assert_eq!(parent.workbench_type, Some(FunctionGroup::WORKBENCH_TYPE));
        assert_eq!(parent.uri, advertised.parent_uri);
        assert!(parent.parent_uri.is_none());

        let target = ObjectTarget::from(reference.clone());
        assert_eq!(target.key(), &key);
        let rebound = target.at(AdtUri::parse("response/module").unwrap());
        assert_eq!(rebound.key(), &key);
        assert_eq!(rebound.parent_uri(), reference.parent_uri());
        assert_eq!(rebound.uri().as_str(), "/sap/bc/adt/response/module");
        let logical = ObjectTarget::from(key.erase()).at(reference.uri().clone());
        assert_eq!(logical.key(), &key.erase());
        assert!(logical.parent_uri().is_none());

        let detached_key: ObjectKey<FunctionModule> = serde_json::from_value(serde_json::json!({
            "name": "Z_MODULE", "object_type": "FUGR/FF"
        }))
        .unwrap();
        let detached = ObjectRef::new(detached_key, reference.uri().clone())
            .with_parent_uri(reference.parent_uri().unwrap().clone());
        assert_eq!(detached, reference);
        assert_ne!(detached.key(), reference.key());
        let parent = detached.parent_reference().unwrap();
        assert_eq!(parent.uri, advertised.parent_uri);
        assert!(parent.name.is_none());
        assert!(parent.workbench_type.is_none());
    }

    #[test]
    fn target_resolution_only_resolves_parents_when_needed() {
        let empty = discovery(br#"<app:service xmlns:app="http://www.w3.org/2007/app" />"#);
        let discovery = discovery(include_bytes!("../../tests/fixtures/discovery.xml"));
        let group = ObjectKey::<FunctionGroup>::new("Z_GROUP");
        let key = group.subobject::<FunctionModule>("Z_MODULE");
        let logical = ObjectTarget::from(key.clone());
        assert!(logical.resolve_uri(&empty).is_err());
        assert!(logical.resolve(&empty).is_err());
        let resolved = logical.resolve(&discovery).unwrap();
        assert_eq!(resolved.key(), &key);
        assert_eq!(resolved.uri(), &logical.resolve_uri(&discovery).unwrap());
        assert_eq!(
            resolved.parent_uri(),
            Some(&discovery.resolve_object_uri(&group).unwrap())
        );

        let located = ObjectRef::new(key, AdtUri::parse("advertised/module").unwrap());
        assert!(located.resolve_parent_uri(&empty).is_err());
        assert_eq!(
            located.resolve_parent_uri(&discovery).unwrap(),
            resolved.parent_uri().cloned()
        );
        let target = ObjectTarget::from(located.clone());
        assert_eq!(target.resolve_uri(&empty).unwrap(), *located.uri());
        assert_eq!(target.resolve(&empty).unwrap(), located);
        let advertised = located.with_parent_uri(AdtUri::parse("advertised/group").unwrap());
        assert_eq!(
            advertised.resolve_parent_uri(&empty).unwrap(),
            advertised.parent_uri().cloned()
        );
        assert_eq!(
            ObjectTarget::from(advertised.clone())
                .resolve(&empty)
                .unwrap()
                .parent_uri(),
            advertised.parent_uri()
        );

        let detached: ObjectKey<FunctionModule> = serde_json::from_value(serde_json::json!({
            "name": "Z_MODULE", "object_type": "FUGR/FF"
        }))
        .unwrap();
        assert!(matches!(
            discovery.resolve_object_uri(&detached),
            Err(ResolveError::Object(ObjectError::ParentObjectRequired { workbench_type }))
                if workbench_type == FunctionModule::WORKBENCH_TYPE
        ));
        assert!(discovery.resolve_object(&detached).is_err());
        let detached = ObjectRef::new(detached, advertised.uri().clone());
        assert!(detached.resolve_parent_uri(&empty).unwrap().is_none());
        assert_eq!(
            ObjectTarget::from(detached.clone())
                .resolve(&empty)
                .unwrap(),
            detached
        );
    }

    #[test]
    fn advertised_object_references_preserve_partial_wire_values() {
        let xml = r#"<adtcore:objectRef adtcore:type="CLAS/OC" adtcore:name="ZCL_TEST" adtcore:packageName="ZPACKAGE" xmlns:adtcore="http://www.sap.com/adt/core" />"#;
        let reference: AdvertisedObjectReference = serde_xml_rs::from_str(xml).unwrap();

        assert_eq!(
            reference.workbench_type.as_ref().unwrap().as_str(),
            "CLAS/OC"
        );
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
    fn unmodeled_erased_reference_retains_runtime_type() {
        let object: ObjectKey<()> =
            ObjectKey::from_parts("Z_UNSUPPORTED".to_owned(), "TEST/X".parse().unwrap(), None);

        assert_eq!(object.workbench_type().as_str(), "TEST/X");
        assert!(matches!(
            object.run(),
            Err(ObjectError::UnsupportedCapability {
                workbench_type,
                capability: "immediate run",
            }) if workbench_type.as_str() == "TEST/X"
        ));

        let json = serde_json::json!({
            "key": { "name": "z_unsupported", "object_type": "TEST/X" },
            "uri": "/sap/bc/adt/Advertised/%2FObject",
            "parent_uri": "/sap/bc/adt/Advertised/%2FParent"
        });
        let located: ObjectRef = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(located.key(), &object);
        let serialized = serde_json::to_value(&located).unwrap();
        assert_eq!(serialized["uri"], json["uri"]);
        assert_eq!(serialized["parent_uri"], json["parent_uri"]);
        assert_eq!(serialized["key"]["name"], "Z_UNSUPPORTED");
        let empty = discovery(br#"<app:service xmlns:app="http://www.w3.org/2007/app" />"#);
        assert!(matches!(
            empty.resolve_object_uri(&object),
            Err(ResolveError::Object(
                ObjectError::UnsupportedObjectType { .. }
            ))
        ));
        let target = ObjectTarget::from(located.clone());
        assert_eq!(target.resolve_uri(&empty).unwrap(), *located.uri());
        assert_eq!(target.resolve(&empty).unwrap(), located);
    }
}
