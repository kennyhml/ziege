use derive_builder::Builder;
use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use super::common::{RepositoryFacet, RepositoryPreselection};
use crate::{
    AdtUri, CategoryId, Discovery, EncodeError, EncodedOperation, GlobalWorkbenchType, ObjectError,
    ObjectRef, ObjectType, Operation, OperationResponse, RepositoryError, RequiresDiscovery,
    ResponseError, Stateless,
    resource::{AdvertisedLink, Relations},
};

/// Fetches one virtual-folder hierarchy layer from the repository information system.
///
/// RIS does not recursively expand the complete hierarchy. Callers traverse a
/// tree by adding the returned folder values as preselections in subsequent
/// queries.
///
/// Handler: `CL_RIS_ADT_RES_VIRTUAL_FOLDERS`
#[derive(Builder, Clone, Debug)]
#[builder(pattern = "owned", setter(into), default)]
pub struct RepositoryContentQuery {
    /// A filter for the object names, expressions like `*` are supported
    search_pattern: String,

    /// A set of search values to match. For example the owner and package.
    ///
    /// Note: The special package prefix `..` can be used to request objects
    /// directly assigned to the package, excluding objects of sub-packages.
    #[builder(setter(each(name = "preselection")))]
    preselections: Vec<RepositoryPreselection>,

    /// The desired facets to return, when left empty, real repository objects are retured.
    /// Note: Despite accepting a list of facets, only the first one is currently used.
    #[builder(setter(each(name = "facet")))]
    facets: Vec<RepositoryFacet>,

    /// A [`RepositoryContentOperation`], default is [`RepositoryContentOperation::Expand`]
    #[builder(setter(strip_option))]
    operation: Option<RepositoryContentOperation>,

    /// Whether object descriptions should be included, off by default.
    #[builder(setter(strip_option))]
    ignore_short_descriptions: Option<bool>,

    /// Whether a version preselection should be taken into consideration. Must be set
    /// for the value in the preselection to be used.
    /// When unspecified in the query, the default behavior is `False`.
    ///
    /// **Negatively impacts the performance (+100ms), use only if needed!**
    #[builder(setter(strip_option))]
    with_versions: Option<bool>,
}

impl Default for RepositoryContentQuery {
    fn default() -> Self {
        Self {
            search_pattern: "*".to_owned(),
            preselections: Vec::new(),
            facets: Vec::new(),
            operation: None,
            ignore_short_descriptions: None,
            with_versions: None,
        }
    }
}

impl RepositoryContentQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository/virtualfolders",
        term: "contents",
    };

    /// Creates a query matching all repository object names.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for a configurable repository-content query.
    pub fn builder() -> RepositoryContentQueryBuilder {
        RepositoryContentQueryBuilder::default()
    }
}

impl Operation for RepositoryContentQuery {
    type Response = RepositoryContent;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let body =
            RepositoryContentRequest::new(&self.search_pattern, &self.preselections, &self.facets)
                .serialize()?;
        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::POST, target);
        if let Some(ignore) = self.ignore_short_descriptions {
            request.push_query("ignoreShortDescriptions", ignore.to_string());
        }
        if let Some(with_versions) = self.with_versions {
            request.push_query("withVersions", with_versions.to_string());
        }
        if let Some(operation) = self.operation {
            request.push_query("operation", operation.as_str());
        }
        request.set_accept(RepositoryContent::MEDIA_TYPE);
        request.set_content_type(RepositoryContentRequest::MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        RepositoryContent::parse(response.body(), response.request_target()).map_err(Into::into)
    }
}

/// Kind of repository content operation.
///
/// This is passed to the backend as query parameter. If it is omitted, the
/// default behavior is `Expand`, which also includes the counts. In other
/// words, `Count` is only useful when no expand is required.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryContentOperation {
    Expand,
    Count,
}

impl RepositoryContentOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Expand => "expand",
            Self::Count => "count",
        }
    }
}

/// The single hierarchy layer returned by a virtual-folder content query.
#[derive(Clone, Debug)]
pub struct RepositoryContent {
    pub object_count: u32,
    pub preselection_info: Option<RepositoryPreselectionInfo>,
    pub folders: Vec<RepositoryVirtualFolder>,
    pub objects: Vec<RepositoryObjectEntry>,
    relations: Relations,
}

impl RepositoryContent {
    pub(super) const MEDIA_TYPE: &str =
        "application/vnd.sap.adt.repository.virtualfolders.result.v1+xml";

    pub(super) fn parse(body: &[u8], request_uri: &AdtUri) -> Result<Self, RepositoryError> {
        let raw: RawRepositoryContent =
            serde_xml_rs::from_reader(body).map_err(RepositoryError::InvalidResponse)?;
        let query_base = request_uri.clone();
        let folders = raw
            .folders
            .into_iter()
            .map(|folder| {
                let uri = folder
                    .uri
                    .map(|uri| {
                        AdtUri::parse(&uri).map_err(|source| RepositoryError::InvalidFolderUri {
                            name: folder.name.clone(),
                            uri,
                            source,
                        })
                    })
                    .transpose()?;
                Ok(RepositoryVirtualFolder {
                    name: folder.name,
                    uri,
                    display_name: folder.display_name,
                    facet: folder.facet,
                    object_count: folder.object_count,
                    text: folder.text,
                    has_children_of_same_facet: folder.has_children_of_same_facet,
                    relations: Relations::for_base(query_base.clone(), folder.links),
                })
            })
            .collect::<Result<_, RepositoryError>>()?;
        let objects = raw
            .objects
            .into_iter()
            .map(RepositoryObjectEntry::try_from)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            object_count: raw.object_count,
            preselection_info: raw
                .preselection_info
                .map(|info| RepositoryPreselectionInfo {
                    facet: info.facet,
                    has_children_of_same_facet: info.has_children_of_same_facet,
                }),
            folders,
            objects,
            relations: Relations::for_base(query_base, raw.links),
        })
    }

    /// Returns links advertised for the result as a whole.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }
}

/// One virtual folder returned by RIS.
#[derive(Clone, Debug)]
pub struct RepositoryVirtualFolder {
    /// The technical folder value, such as `CLAS`.
    pub name: String,
    /// The validated resource URI when this folder represents an ADT resource.
    pub uri: Option<AdtUri>,
    /// The server-provided display label.
    pub display_name: String,
    /// The facet by which this folder groups its contents.
    pub facet: RepositoryFacet,
    /// The number of objects below this folder.
    pub object_count: u32,
    /// Additional server-provided text, often empty.
    pub text: String,
    /// Whether another hierarchy level uses the same facet.
    pub has_children_of_same_facet: bool,
    relations: Relations,
}

impl RepositoryVirtualFolder {
    /// Returns whether this folder selects objects assigned directly to a package.
    ///
    /// SAP prefixes the namespace-qualified package name with `..`, producing
    /// values such as `../DMO/FLIGHT_REUSE`.
    pub fn is_direct_assignment(&self) -> bool {
        self.direct_assignment_package().is_some()
    }

    /// Returns the package selected by a direct-assignment folder.
    pub fn direct_assignment_package(&self) -> Option<&str> {
        if self.facet != RepositoryFacet::PACKAGE {
            return None;
        }
        self.name
            .strip_prefix("..")
            .filter(|package| !package.is_empty())
    }

    /// Returns links advertised for this virtual folder.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }

    /// Creates the filter selecting this folder in a subsequent hierarchy query.
    pub fn as_preselection(&self) -> RepositoryPreselection {
        RepositoryPreselection::new(self.facet.clone(), self.name.clone())
    }

    pub fn name_or_technical_name(&self) -> &str {
        if self.display_name.is_empty() {
            &self.name
        } else {
            &self.display_name
        }
    }
}

/// A repository object listed in a virtual-folder result.
#[derive(Clone, Debug)]
pub struct RepositoryObjectEntry {
    pub name: String,
    /// The object version when the query requested version information.
    pub version: Option<String>,
    pub package: String,
    /// A validated, type-erased reference to the ADT object resource.
    pub reference: ObjectRef<()>,
    uri: AdtUri,
    /// The corresponding virtual Workbench URI, when supplied by SAP.
    pub virtual_workbench_uri: Option<String>,
    pub expandable: bool,
    /// The short description, omitted when descriptions were ignored.
    pub description: Option<String>,
    relations: Relations,
}

impl RepositoryObjectEntry {
    /// Returns the authoritative object URI advertised by RIS.
    pub fn uri(&self) -> &AdtUri {
        &self.uri
    }

    /// Returns links advertised for this repository object.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }

    /// Converts this RIS entry into a checked static object reference.
    ///
    /// The conversion verifies the exact Workbench type.
    pub fn typed_reference<T: ObjectType>(&self) -> Result<ObjectRef<T>, ObjectError> {
        if self.reference.object_type() != &T::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedRepositoryObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: self.reference.object_type().clone(),
            });
        }

        Ok(self.reference.retag())
    }

    /// Returns the runtime-typed object reference advertised by RIS.
    pub fn repository_object(&self) -> ObjectRef<()> {
        self.reference.clone()
    }
}

impl<T: ObjectType> TryFrom<&RepositoryObjectEntry> for ObjectRef<T> {
    type Error = ObjectError;

    fn try_from(entry: &RepositoryObjectEntry) -> Result<Self, Self::Error> {
        entry.typed_reference()
    }
}

impl TryFrom<RawRepositoryObjectEntry> for RepositoryObjectEntry {
    type Error = RepositoryError;

    fn try_from(raw: RawRepositoryObjectEntry) -> Result<Self, Self::Error> {
        let uri = AdtUri::parse(&raw.uri).map_err(|source| RepositoryError::InvalidObjectUri {
            name: raw.name.clone(),
            uri: raw.uri,
            source,
        })?;
        let reference = ObjectRef::from_parts(raw.name.to_ascii_uppercase(), raw.object_type, None);
        Ok(Self {
            name: raw.name,
            version: raw.version,
            package: raw.package,
            virtual_workbench_uri: raw.virtual_workbench_uri,
            expandable: raw.expandable,
            description: raw.description,
            relations: Relations::for_base(uri.clone(), raw.links),
            reference,
            uri,
        })
    }
}

/// Information that helps construct the next package-hierarchy query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPreselectionInfo {
    pub facet: RepositoryFacet,
    pub has_children_of_same_facet: bool,
}

#[derive(Serialize)]
#[serde(rename = "vfs:virtualFoldersRequest")]
pub(super) struct RepositoryContentRequest<'a> {
    #[serde(rename = "@objectSearchPattern")]
    search_pattern: &'a str,
    #[serde(rename = "vfs:preselection")]
    preselections: &'a [RepositoryPreselection],
    #[serde(rename = "vfs:facetorder")]
    facet_order: RawFacetOrder<'a>,
}

impl<'a> RepositoryContentRequest<'a> {
    pub(super) const MEDIA_TYPE: &'static str =
        "application/vnd.sap.adt.repository.virtualfolders.request.v1+xml";
    const NAMESPACE: &'static str = "http://www.sap.com/adt/ris/virtualFolders";

    pub(super) fn new(
        search_pattern: &'a str,
        preselections: &'a [RepositoryPreselection],
        facets: &'a [RepositoryFacet],
    ) -> Self {
        Self {
            search_pattern,
            preselections,
            facet_order: RawFacetOrder { facets },
        }
    }

    pub(super) fn serialize(&self) -> Result<String, RepositoryError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("vfs", Self::NAMESPACE)
            .to_string(self)
            .map_err(RepositoryError::InvalidRequest)
    }
}

#[derive(Serialize)]
struct RawFacetOrder<'a> {
    #[serde(rename = "vfs:facet")]
    facets: &'a [RepositoryFacet],
}

#[derive(Deserialize)]
#[serde(rename = "vfs:virtualFoldersResult")]
struct RawRepositoryContent {
    #[serde(rename = "@objectCount")]
    object_count: u32,
    #[serde(rename = "vfs:preselectionInfo")]
    preselection_info: Option<RawPreselectionInfo>,
    #[serde(rename = "vfs:virtualFolder", default)]
    folders: Vec<RawVirtualFolder>,
    #[serde(rename = "vfs:object", default)]
    objects: Vec<RawRepositoryObjectEntry>,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}

#[derive(Deserialize)]
struct RawPreselectionInfo {
    #[serde(rename = "@facet")]
    facet: RepositoryFacet,
    #[serde(rename = "@hasChildrenOfSameFacet")]
    has_children_of_same_facet: bool,
}

#[derive(Deserialize)]
struct RawVirtualFolder {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@uri")]
    uri: Option<String>,
    #[serde(rename = "@displayName")]
    display_name: String,
    #[serde(rename = "@facet")]
    facet: RepositoryFacet,
    #[serde(rename = "@counter")]
    object_count: u32,
    #[serde(rename = "@text", default)]
    text: String,
    #[serde(rename = "@hasChildrenOfSameFacet")]
    has_children_of_same_facet: bool,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}

#[derive(Deserialize)]
struct RawRepositoryObjectEntry {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@version")]
    version: Option<String>,
    #[serde(rename = "@package")]
    package: String,
    #[serde(rename = "@type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@uri")]
    uri: String,
    #[serde(rename = "@vituri")]
    virtual_workbench_uri: Option<String>,
    #[serde(rename = "@expandable")]
    expandable: bool,
    #[serde(rename = "@text")]
    description: Option<String>,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}
