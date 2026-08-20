use http::{Method, StatusCode};
use serde::Deserialize;

use super::{common::RepositoryFacet, content::RepositoryObjectEntry};
use crate::{
    AdtRequest, AdtUri, AnyObject, CategoryId, Client, GlobalWorkbenchType, Object, ObjectError,
    ObjectRef, ObjectType, Operation, OperationError, OperationResponse, Package, Ready,
    RepositoryError, ResponseError, Stateless, TransportNumber, TransportStatus,
    resource::{AdvertisedLink, Relations},
    target::CollectionTarget,
    vocabulary::{media_type, query_parameter},
};

pub(super) const PACKAGE_RELATION: &str = "http://www.sap.com/adt/relations/packages";
const ASSIGNED_TRANSPORTS_CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/repository",
    term: "transportProperties",
};

/// Fetches uniform RIS properties for an arbitrary repository object.
///
/// The repository object to fetch is represented by its [`AdtUri`]. For example,
/// `?uri=/sap/bc/adt/programs/programs/ZPROG` returns the properties of the
/// program `ZPROG`.
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
    object_uri: AdtUri,
    facets: Vec<RepositoryFacet>,
}

impl RepositoryObjectPropertiesQuery {
    const TARGET: CollectionTarget = CollectionTarget::new(CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository",
        term: "objectProperties",
    });

    /// Creates a property query for a validated object reference.
    pub fn new<T>(object: &ObjectRef<T>) -> Self {
        Self::for_uri(object.uri().clone())
    }

    /// Creates a property query from a validated ADT object URI.
    pub fn for_uri(object_uri: AdtUri) -> Self {
        Self {
            object_uri,
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

impl Operation<Ready> for RepositoryObjectPropertiesQuery {
    type Response = RepositoryObjectProperties;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let mut request = Self::TARGET.request(client, Method::GET)?;
        request.push_query("uri", self.object_uri.as_str());
        for facet in &self.facets {
            request.push_query("facet", facet.as_str());
        }
        request.set_accept(media_type::REPOSITORY_OBJECT_PROPERTIES);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        RepositoryObjectProperties::parse(response.body(), &self.object_uri).map_err(Into::into)
    }
}

impl RepositoryObjectEntry {
    /// Creates a uniform RIS property query for this listed object.
    pub fn properties(&self) -> RepositoryObjectPropertiesQuery {
        RepositoryObjectPropertiesQuery::new(&self.reference)
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
    object_uri: AdtUri,
}

impl AssignedTransportsQuery {
    const TARGET: CollectionTarget = CollectionTarget::new(ASSIGNED_TRANSPORTS_CATEGORY);

    /// Creates a query for the transport requests assigned to an object.
    pub fn new<T>(object: &ObjectRef<T>) -> Self {
        Self::for_uri(object.uri().clone())
    }

    /// Creates a query for the repository objects resolved from an ADT URI.
    pub fn for_uri(object_uri: AdtUri) -> Self {
        Self { object_uri }
    }
}

impl Operation<Ready> for AssignedTransportsQuery {
    type Kind = Stateless;
    type Response = AssignedTransportRequests;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let mut request = Self::TARGET.request(client, Method::GET)?;
        request.push_query(query_parameter::URI, self.object_uri.as_str());
        request.set_accept(media_type::REPOSITORY_OBJECT_TR_PROPERTIES);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[media_type::REPOSITORY_OBJECT_TR_PROPERTIES])?;
        serde_xml_rs::from_reader(response.body())
            .map_err(RepositoryError::InvalidResponse)
            .map_err(Into::into)
    }
}

impl<T> ObjectRef<T> {
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::new(self)
    }
}

impl<T: ObjectType> Object<T> {
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::new(self.reference())
    }
}

impl AnyObject {
    pub fn transport_requests(&self) -> AssignedTransportsQuery {
        AssignedTransportsQuery::new(self.reference())
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
    /// Returns the package hierarchy in the top-down order emitted by RIS.
    ///
    /// The first entry is the root package and the final entry is the package
    /// directly containing the object. An empty hierarchy means package
    /// properties were not requested or the object has no package assignment.
    pub fn package_hierarchy(&self) -> Result<Vec<ObjectRef<Package>>, ObjectError> {
        self.properties
            .iter()
            .filter(|property| property.facet == RepositoryFacet::PACKAGE)
            .map(|property| {
                let link = property.relations.find(PACKAGE_RELATION)?.ok_or(
                    ObjectError::MissingRelation {
                        relation: PACKAGE_RELATION,
                    },
                )?;
                Ok(ObjectRef::new(property.value.clone(), link.target.clone()))
            })
            .collect()
    }
}

impl RepositoryObjectProperties {
    pub(super) fn parse(body: &[u8], object_uri: &AdtUri) -> Result<Self, RepositoryError> {
        let raw: RawRepositoryObjectProperties =
            serde_xml_rs::from_reader(body).map_err(RepositoryError::InvalidResponse)?;
        let reference = ObjectRef::erased(
            raw.object.name.clone(),
            object_uri.clone(),
            raw.object.object_type.clone(),
        );
        let properties = raw
            .properties
            .into_iter()
            .map(|property| RepositoryProperty {
                facet: property.facet,
                value: property.value,
                display_name: property.display_name,
                description: property.description,
                has_children_of_same_facet: property.has_children_of_same_facet,
                relations: Relations::new(reference.clone(), property.links),
            })
            .collect();
        let object = RepositoryObjectSummary {
            name: raw.object.name,
            description: raw.object.description,
            package: raw.object.package,
            object_type: raw.object.object_type,
            expandable: raw.object.expandable,
            relations: Relations::new(reference.clone(), raw.object.links),
            reference,
        };

        Ok(Self { object, properties })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "tpr:transportProperties")]
pub struct AssignedTransportRequests {
    #[serde(rename = "tpr:transport", default)]
    pub requests: Vec<AssignedTransport>,
}

impl AssignedTransportRequests {
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename = "tpr:transport")]
pub struct AssignedTransport {
    #[serde(rename = "@number")]
    pub number: TransportNumber,

    #[serde(rename = "@owner")]
    pub owner: String,

    #[serde(rename = "@status")]
    pub status: TransportStatus,

    #[serde(rename = "@description")]
    pub description: String,
}

#[derive(Deserialize)]
#[serde(rename = "opr:objectProperties")]
struct RawRepositoryObjectProperties {
    #[serde(rename = "opr:object")]
    object: RawRepositoryObjectSummary,
    #[serde(rename = "opr:property", default)]
    properties: Vec<RawRepositoryProperty>,
}

#[derive(Deserialize)]
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
