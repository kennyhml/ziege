use http::{Method, StatusCode};
use serde::Deserialize;

use super::common::RepositoryFacet;
use crate::{
    AdtRequest, CategoryId, Client, Operation, OperationError, OperationResponse, Ready,
    RepositoryError, ResponseError, Stateless, operation::CollectionTarget,
};

/// Fetches the facets supported by the repository information system.
///
/// Fundamental facets that have been part of the API for a long time
/// are supported statically via [`RepositoryFacet`]. This operation is
/// more of a way to check compatiblity with the backend system to see
/// which facets make sense to use.
///
/// Handler: `CL_RIS_ADT_RES_VF_FACETS`
#[derive(Clone, Copy, Debug, Default)]
pub struct RepositoryFacetsQuery;

impl RepositoryFacetsQuery {
    const TARGET: CollectionTarget = CollectionTarget::new(CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository/virtualfolders",
        term: "facets",
    });
}

impl Client<Ready> {
    /// Creates a query for the facets supported by the repository information system.
    pub fn repository_facets(&self) -> RepositoryFacetsQuery {
        RepositoryFacetsQuery
    }
}

impl Operation<Ready> for RepositoryFacetsQuery {
    type Response = RepositoryFacets;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        Self::TARGET.request(client, Method::GET)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        RepositoryFacets::parse(response.body()).map_err(Into::into)
    }
}

/// An optional URI-template link for discovering values of a facet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryFacetValuesLink {
    pub title: Option<String>,
    pub relation: String,
    pub template: String,
    pub media_type: Option<String>,
}

/// A facet advertised by the repository information system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryFacetDefinition {
    /// The key exactly as advertised, commonly in lowercase.
    pub key: String,
    pub display_name: String,
    pub description: String,
    pub is_hierarchical: bool,
    pub is_for_filtering: bool,
    pub is_for_structuring: bool,
    pub values: Option<RepositoryFacetValuesLink>,
}

impl RepositoryFacetDefinition {
    /// Returns this advertised key in the uppercase form used by RIS queries.
    pub fn facet(&self) -> RepositoryFacet {
        self.key.to_ascii_uppercase().into()
    }
}

/// Facets supported by the repository information system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryFacets {
    pub facets: Vec<RepositoryFacetDefinition>,
}

impl RepositoryFacets {
    pub(super) fn parse(body: &[u8]) -> Result<Self, RepositoryError> {
        let raw: RawRepositoryFacets =
            serde_xml_rs::from_reader(body).map_err(RepositoryError::InvalidResponse)?;
        Ok(Self {
            facets: raw
                .facets
                .into_iter()
                .map(|facet| RepositoryFacetDefinition {
                    key: facet.key,
                    display_name: facet.display_name,
                    description: facet.description,
                    is_hierarchical: facet.is_hierarchical,
                    is_for_filtering: facet.is_for_filtering,
                    is_for_structuring: facet.is_for_structuring,
                    values: facet.values.map(|link| RepositoryFacetValuesLink {
                        title: link.title,
                        relation: link.relation,
                        template: link.template,
                        media_type: link.media_type,
                    }),
                })
                .collect(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename = "vf:facets")]
struct RawRepositoryFacets {
    #[serde(rename = "vf:facet", default)]
    facets: Vec<RawRepositoryFacetDefinition>,
}

#[derive(Deserialize)]
struct RawRepositoryFacetDefinition {
    #[serde(rename = "@key")]
    key: String,
    #[serde(rename = "@displayName")]
    display_name: String,
    #[serde(rename = "@description")]
    description: String,
    #[serde(rename = "@isHierarchical")]
    is_hierarchical: bool,
    #[serde(rename = "@isForFiltering")]
    is_for_filtering: bool,
    #[serde(rename = "@isForStructuring")]
    is_for_structuring: bool,
    #[serde(rename = "adtcomp:templateLink")]
    values: Option<RawTemplateLink>,
}

#[derive(Deserialize)]
struct RawTemplateLink {
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@rel")]
    relation: String,
    #[serde(rename = "@template")]
    template: String,
    #[serde(rename = "@type")]
    media_type: Option<String>,
}
