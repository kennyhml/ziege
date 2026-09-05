use http::{Method, StatusCode};
use serde::Deserialize;

use super::{common::RepositoryFacet, content::RepositoryObjectEntry};
use crate::{
    AdtUri, CategoryId, Discovery, EncodeError, EncodedOperation, GlobalWorkbenchType, ObjectError,
    ObjectKey, ObjectRef, ObjectSnapshot, ObjectType, Operation, OperationResponse, Package,
    RepositoryError, RequiresDiscovery, ResponseError, Stateless, TransportNumber, TransportStatus,
    User,
    objects::ObjectTarget,
    resource::{AdvertisedLink, Relations, resolve_href},
};

/// Fetches uniform RIS properties for an arbitrary repository object.
///
/// The repository object can be supplied as an [`ObjectKey`], a located
/// [`ObjectRef`], or directly as an [`AdtUri`]. Logical keys are resolved through
/// discovery when the query is encoded. For example, `?uri=/sap/bc/adt/programs/programs/ZPROG`
/// returns the properties of the program `ZPROG`.
///
/// These properties are RIS centric and thus only provide generic repository information,
/// such as facet properties and packages. Notably, the entire package chain is returned,
/// the order they are returned in defines the hierarchy.
///
/// This is the information Eclipse shows in the `Properties` view under `General`.
///
/// Handler: `CL_RIS_ADT_RES_OBJ_PROPERTIES`
#[derive(Clone, Debug)]
pub struct RepositoryObjectPropertiesQuery {
    target: RepositoryObjectTarget,
    facets: Vec<RepositoryFacet>,
}

#[derive(Clone, Debug)]
enum RepositoryObjectTarget {
    Object(ObjectTarget<()>),
    Uri(AdtUri),
}

impl RepositoryObjectTarget {
    fn for_object<T>(object: &ObjectKey<T>) -> Self {
        Self::Object(object.erase().into())
    }

    fn for_uri(uri: AdtUri) -> Self {
        Self::Uri(uri)
    }

    fn resolve(&self, resolver: &Discovery) -> Result<AdtUri, EncodeError> {
        match self {
            Self::Object(object) => Ok(object.resolve_uri(resolver)?),
            Self::Uri(uri) => Ok(uri.clone()),
        }
    }
}

impl RepositoryObjectPropertiesQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository",
        term: "objectProperties",
    };
    const URI_QUERY: &str = "uri";

    /// Creates a property query for a validated object reference.
    pub fn new<T>(object: &ObjectKey<T>) -> Self {
        Self {
            target: RepositoryObjectTarget::for_object(object),
            facets: Vec::new(),
        }
    }

    /// Creates a property query preserving the object's advertised location.
    pub fn from_ref<T>(object: &ObjectRef<T>) -> Self {
        Self {
            target: RepositoryObjectTarget::Object(object.erase().into()),
            facets: Vec::new(),
        }
    }

    /// Creates a property query from a validated ADT object URI.
    pub fn for_uri(object_uri: AdtUri) -> Self {
        Self {
            target: RepositoryObjectTarget::for_uri(object_uri),
            facets: Vec::new(),
        }
    }

    /// Includes one RIS facet in the returned properties.
    #[must_use]
    pub fn include_facet(mut self, facet: impl Into<RepositoryFacet>) -> Self {
        self.facets.push(facet.into());
        self
    }
}

impl Operation for RepositoryObjectPropertiesQuery {
    type Response = RepositoryObjectProperties;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let object_uri = self.target.resolve(resolver)?;
        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        request.push_query(Self::URI_QUERY, object_uri.as_str());
        for facet in &self.facets {
            request.push_query("facet", facet.as_str());
        }
        request.set_accept(RepositoryObjectProperties::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        RepositoryObjectProperties::parse_query(response.body(), response.request_target())
    }
}

impl RepositoryObjectEntry {
    /// Creates a uniform RIS property query for this listed object.
    pub fn properties(&self) -> RepositoryObjectPropertiesQuery {
        RepositoryObjectPropertiesQuery::from_ref(&self.reference)
    }
}

/// Queries unreleased transport requests assigned to an object.
///
/// If the object expands into multiple objects, such as a package,
/// all transport requests of objects assigned to that package are
/// returned.
///
/// Backend handler: `CL_RIS_ADT_RES_TR_PROPERTIES`
#[derive(Debug, Clone)]
pub struct AssignedTransportsQuery {
    target: RepositoryObjectTarget,
}

impl AssignedTransportsQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository",
        term: "transportProperties",
    };
    const URI_QUERY: &str = "uri";

    /// Creates a query for the transport requests assigned to an object.
    pub fn new<T>(object: &ObjectKey<T>) -> Self {
        Self {
            target: RepositoryObjectTarget::for_object(object),
        }
    }

    /// Creates a query preserving the object's advertised location.
    pub fn from_ref<T>(object: &ObjectRef<T>) -> Self {
        Self {
            target: RepositoryObjectTarget::Object(object.erase().into()),
        }
    }

    /// Creates a query for the repository objects resolved from an ADT URI.
    pub fn for_uri(object_uri: AdtUri) -> Self {
        Self {
            target: RepositoryObjectTarget::for_uri(object_uri),
        }
    }
}

impl Operation for AssignedTransportsQuery {
    type Kind = Stateless;
    type Response = AssignedTransportRequests;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let object_uri = self.target.resolve(resolver)?;
        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        request.push_query(Self::URI_QUERY, object_uri.as_str());
        request.set_accept(AssignedTransportRequests::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[AssignedTransportRequests::MEDIA_TYPE])?;
        serde_xml_rs::from_reader(response.body())
            .map_err(RepositoryError::InvalidResponse)
            .map_err(Into::into)
    }
}

impl<T> ObjectKey<T> {
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::new(self)
    }
}

impl<T> ObjectRef<T> {
    /// Queries transport requests using this object's advertised URI.
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::from_ref(self)
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::for_uri(self.uri().clone())
    }
}

impl ObjectSnapshot<()> {
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::for_uri(self.uri().clone())
    }
}

/// The RIS description of the object whose properties were requested.
#[derive(Clone, Debug)]
pub struct RepositoryObjectSummary {
    pub name: String,
    pub description: String,
    pub package: String,
    pub object_type: GlobalWorkbenchType,
    pub expandable: bool,
    pub reference: ObjectRef,
    relations: Relations,
}

impl RepositoryObjectSummary {
    /// Returns links advertised for this repository object.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }
}

/// One facet value associated with a repository object.
#[derive(Clone, Debug)]
pub struct RepositoryProperty {
    pub facet: RepositoryFacet,
    pub value: String,
    pub display_name: String,
    pub description: Option<String>,
    pub has_children_of_same_facet: Option<bool>,
    relations: Relations,
}

impl RepositoryProperty {
    /// Returns links advertised for this property value.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }
}

/// Uniform RIS properties for an arbitrary repository object.
#[derive(Clone, Debug)]
pub struct RepositoryObjectProperties {
    pub object: RepositoryObjectSummary,
    pub properties: Vec<RepositoryProperty>,
}

impl RepositoryObjectProperties {
    pub(super) const MEDIA_TYPE: &str =
        "application/vnd.sap.adt.repository.objproperties.result.v1+xml";
    const OBJECT_RELATION: &str = "http://www.sap.com/adt/relations/objects";
    const SAP_GUI_MEDIA_TYPE: &str = "application/vnd.sap.sapgui";

    /// Returns the package hierarchy in the top-down order emitted by RIS.
    ///
    /// The first entry is the root package and the final entry is the package
    /// directly containing the object. An empty hierarchy means package
    /// properties were not requested or the object has no package assignment.
    pub fn package_hierarchy(&self) -> Vec<ObjectKey<Package>> {
        self.properties
            .iter()
            .filter(|property| property.facet == RepositoryFacet::PACKAGE)
            .map(|property| ObjectKey::new(property.value.clone()))
            .collect()
    }
}

impl RepositoryObjectProperties {
    #[cfg(test)]
    pub(super) fn parse(body: &[u8], request_uri: &AdtUri) -> Result<Self, ResponseError> {
        Self::parse_query(body, request_uri)
    }

    fn parse_query(body: &[u8], request_uri: &AdtUri) -> Result<Self, ResponseError> {
        let raw = Self::parse_raw(body)?;
        let link = raw
            .object
            .links
            .iter()
            .find(|link| {
                link.relation.as_deref() == Some(Self::OBJECT_RELATION)
                    && link.media_type.as_deref().is_none_or(|media_type| {
                        !media_type.eq_ignore_ascii_case(Self::SAP_GUI_MEDIA_TYPE)
                    })
            })
            .ok_or(ObjectError::MissingRelation {
                relation: Self::OBJECT_RELATION,
            })?;
        let object_uri = resolve_href(request_uri, &link.href)
            .map(|resolved| resolved.target)
            .map_err(|source| RepositoryError::InvalidObjectUri {
                name: raw.object.name.clone(),
                uri: link.href.clone(),
                source,
            })?;

        Ok(Self::from_raw(raw, &object_uri))
    }

    fn parse_raw(body: &[u8]) -> Result<RawRepositoryObjectProperties, RepositoryError> {
        serde_xml_rs::from_reader(body).map_err(RepositoryError::InvalidResponse)
    }

    fn from_raw(raw: RawRepositoryObjectProperties, object_uri: &AdtUri) -> Self {
        let key = ObjectKey::from_parts(
            raw.object.name.to_ascii_uppercase(),
            raw.object.object_type.clone(),
            None,
        );
        let reference = ObjectRef::new(key, object_uri.clone());
        let properties = raw
            .properties
            .into_iter()
            .map(|property| RepositoryProperty {
                facet: property.facet,
                value: property.value,
                display_name: property.display_name,
                description: property.description,
                has_children_of_same_facet: property.has_children_of_same_facet,
                relations: Relations::for_base(object_uri.clone(), property.links),
            })
            .collect();
        let object = RepositoryObjectSummary {
            name: raw.object.name,
            description: raw.object.description,
            package: raw.object.package,
            object_type: raw.object.object_type,
            expandable: raw.object.expandable,
            relations: Relations::for_base(object_uri.clone(), raw.object.links),
            reference,
        };

        Self { object, properties }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "tpr:transportProperties", deny_unknown_fields)]
pub struct AssignedTransportRequests {
    #[serde(rename = "tpr:transport", default)]
    pub requests: Vec<AssignedTransport>,
}

impl AssignedTransportRequests {
    pub(super) const MEDIA_TYPE: &str =
        "application/vnd.sap.adt.repository.trproperties.result.v1+xml";

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename = "tpr:transport", deny_unknown_fields)]
pub struct AssignedTransport {
    #[serde(rename = "@number")]
    pub number: TransportNumber,

    #[serde(rename = "@owner")]
    pub owner: User,

    #[serde(rename = "@status")]
    pub status: TransportStatus,

    #[serde(rename = "@description")]
    pub description: String,
}

#[derive(Deserialize)]
#[serde(rename = "opr:objectProperties", deny_unknown_fields)]
struct RawRepositoryObjectProperties {
    #[serde(rename = "opr:object")]
    object: RawRepositoryObjectSummary,
    #[serde(rename = "opr:property", default)]
    properties: Vec<RawRepositoryProperty>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepositoryObjectSummary {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@text", default)]
    description: String,
    #[serde(rename = "@package")]
    package: String,
    #[serde(rename = "@type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@expandable")]
    expandable: bool,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepositoryProperty {
    #[serde(rename = "@facet")]
    facet: RepositoryFacet,
    #[serde(rename = "@name")]
    value: String,
    #[serde(rename = "@displayName")]
    display_name: String,
    #[serde(rename = "@text")]
    description: Option<String>,
    #[serde(rename = "@hasChildrenOfSameFacet")]
    has_children_of_same_facet: Option<bool>,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}
