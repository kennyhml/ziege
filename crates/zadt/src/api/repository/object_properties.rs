use derive_builder::Builder;
use http::Method;
use serde::Deserialize;

use super::{
    common::{RepositoryFacet, ensure_ok},
    content::RepositoryObjectEntry,
};
use crate::{
    AdtRequest, AdtUri, CategoryId, Client, GlobalWorkbenchType, ObjectError, ObjectRef, Operation,
    OperationError, OperationResponse, Package, Ready, RepositoryError, ResponseError, Stateless,
    resource::{AdvertisedLink, Relations},
    target::CollectionTarget,
    vocabulary::media_type,
};

pub(super) const PACKAGE_RELATION: &str = "http://www.sap.com/adt/relations/packages";

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
#[derive(Builder, Clone, Debug)]
#[builder(pattern = "owned", setter(into))]
pub struct RepositoryObjectPropertiesQuery {
    object_uri: AdtUri,

    #[builder(default, setter(each(name = "include_facet")))]
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

    /// Creates a builder initialized for a validated object reference.
    pub fn builder<T>(object: &ObjectRef<T>) -> RepositoryObjectPropertiesQueryBuilder {
        RepositoryObjectPropertiesQueryBuilder::default().object_uri(object.uri().clone())
    }

    /// Creates a builder initialized with a validated ADT object URI.
    pub fn builder_for_uri(object_uri: AdtUri) -> RepositoryObjectPropertiesQueryBuilder {
        RepositoryObjectPropertiesQueryBuilder::default().object_uri(object_uri)
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
        ensure_ok(response.response())?;
        RepositoryObjectProperties::parse(response.body(), &self.object_uri).map_err(Into::into)
    }
}

impl RepositoryObjectEntry {
    /// Creates a uniform RIS property query for this listed object.
    pub fn properties(&self) -> RepositoryObjectPropertiesQuery {
        RepositoryObjectPropertiesQuery::new(&self.reference)
    }

    /// Creates a configurable uniform RIS property query for this listed object.
    pub fn properties_builder(&self) -> RepositoryObjectPropertiesQueryBuilder {
        RepositoryObjectPropertiesQuery::builder(&self.reference)
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
