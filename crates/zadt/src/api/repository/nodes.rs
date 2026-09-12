use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    AdtLink, AdtUri, CategoryId, Discovery, EncodeError, EncodedOperation, GlobalWorkbenchType,
    ObjectError, ObjectKey, ObjectRef, Operation, OperationResponse, RepositoryError,
    RequiresDiscovery, ResponseError, Stateless, resource::resolve_href,
};

const ROOT_NODE: &str = "000000";

/// Queries one layer of the backend repository browser tree.
///
/// Unlike RIS virtual folders, this resource enumerates children of repository
/// objects, including function modules and function-group includes. The initial
/// request uses node ID `000000`. Expand the returned groups to obtain their
/// objects, or use [`RepositoryNodes::groups_query`] to request all groups together.
///
/// Queries carry their object context and do not require a stateful ADT session.
/// Backend node IDs are navigation tokens, not persistent object identities.
///
/// Rediscover groups after rebuilding a tree instead of persisting their IDs.
///
/// Handler: `CL_SEU_ADT_RES_REPO_STRUCTURE`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryNodesQuery {
    root: ObjectKey<()>,
    technical_name: Option<String>,
    node_ids: Vec<String>,
    options: NodeOptions,
    rebuild: bool,
}

impl RepositoryNodesQuery {
    const CATEGORY: CategoryId = CategoryId {
        // The misspelling is part of the advertised SAP discovery category.
        scheme: "http://www.sap.com/adt/categories/respository",
        term: "nodestructure",
    };

    const REQUEST_MEDIA_TYPE: &str = "application/vnd.sap.as+xml";
    const RESPONSE_MEDIA_TYPE: &str =
        "application/vnd.sap.as+xml;charset=utf-8;dataname=com.sap.adt.RepositoryObjectTreeContent";

    /// Starts navigation at the supplied logical object, without a type allowlist.
    pub fn new<T>(root: &ObjectKey<T>) -> Self {
        Self {
            root: root.erase(),
            technical_name: None,
            node_ids: vec![ROOT_NODE.to_owned()],
            options: NodeOptions::default(),
            rebuild: false,
        }
    }

    /// The logical object identifying this backend tree.
    pub fn root(&self) -> &ObjectKey<()> {
        &self.root
    }

    /// Overrides the technical parent name. The backend otherwise uses the logical name.
    pub fn technical_name(mut self, name: impl Into<String>) -> Self {
        self.technical_name = Some(name.into());
        self
    }

    /// Selects the repository-browser user. Omission uses the authenticated user.
    pub fn user_name(mut self, user: impl Into<String>) -> Self {
        self.options.user_name = Some(user.into());
        self
    }

    pub fn short_descriptions(mut self, enabled: bool) -> Self {
        self.options.short_descriptions = enabled;
        self
    }

    pub fn with_versions(mut self, enabled: bool) -> Self {
        self.options.with_versions = enabled;
        self
    }

    pub fn exclude_local_packages(mut self, enabled: bool) -> Self {
        self.options.exclude_local_packages = enabled;
        self
    }

    /// Requests a backend tree rebuild for this execution only.
    /// Returned child queries do not inherit this flag.
    pub fn rebuild(mut self, enabled: bool) -> Self {
        self.rebuild = enabled;
        self
    }

    /// Whether this query discovers the initial folders of an object tree.
    pub fn is_root(&self) -> bool {
        self.node_ids.len() == 1 && self.node_ids[0] == ROOT_NODE
    }

    fn for_nodes(&self, node_ids: Vec<String>) -> Self {
        Self {
            node_ids,
            rebuild: false,
            ..self.clone()
        }
    }

    /// Continues this tree at a returned browser selector.
    fn for_browser_node(&self, node: &RawNode) -> Self {
        let mut next = self.for_nodes(vec![node.node_id.clone()]);
        // PARENT_NAME can be SAPL<group>, so it must not replace the logical root.
        if !node.parent_name.is_empty() {
            next.technical_name = Some(node.parent_name.clone());
        }
        if !node.technical_name.is_empty() {
            next.technical_name = Some(node.technical_name.clone());
        }
        next
    }
}

impl Operation for RepositoryNodesQuery {
    type Response = RepositoryNodes;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let body = NodeRequest {
            version: "1.0",
            values: NodeRequestValues {
                data: NodeIds {
                    items: &self.node_ids,
                },
            },
        };

        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::POST, target);

        request.push_query("parent_type", self.root.workbench_type().as_str());
        request.push_query("parent_name", self.root.name());

        if let Some(name) = &self.technical_name {
            request.push_query("parent_tech_name", name);
        }
        if let Some(user) = &self.options.user_name {
            request.push_query("user_name", user);
        }

        // The handler interprets non-empty values as true, even the word "false".
        if self.options.short_descriptions {
            request.push_query("withShortDescriptions", "true");
        }
        if self.options.with_versions {
            request.push_query("withVersions", "true");
        }
        if self.options.exclude_local_packages {
            request.push_query("exclude_local_packages", "X");
        }
        if self.rebuild {
            request.push_query("rebuild_tree", "X");
        }

        request.set_accept(Self::RESPONSE_MEDIA_TYPE);
        request.set_content_type(Self::REQUEST_MEDIA_TYPE);
        request.set_body(body.serialize()?);

        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;

        // Accept the response with or without an explicit charset parameter.
        response.require_content_type(&[Self::RESPONSE_MEDIA_TYPE])?;

        let target = response.request_target();
        let data = RawNodes::deserialize(response.body())?.values.data;
        let mut nodes = Vec::with_capacity(data.objects.items.len());

        // The response also contains folders for the object types in a separate container.
        let mut groups = Vec::new();
        if self.is_root() {
            for kind in &data.object_types.items {
                if !kind.node_id.is_empty() && kind.node_id != ROOT_NODE {
                    groups.push(RepositoryNodeGroup {
                        definition: kind.clone(),
                        query: self.for_nodes(vec![kind.node_id.clone()]),
                    });
                }
            }
        }

        // Class subfolders such as "Inherited Methods" are placeholders in
        // TREE_CONTENT with their labels in OBJECT_TYPES. See [`RepositoryNodes`].
        // Keep every row as an object unless it can be identified as a folder.
        for raw in data.objects.items {
            if !raw.is_placeholder() {
                nodes.push(RepositoryNode::from_raw(raw, self, target)?);
                continue;
            }

            // See if theres a node in the object types that maps to this tree content.
            if let Some(kind) = data.object_types.items.iter().find(|kind| {
                kind.workbench_type == raw.workbench_type && kind.node_id == raw.node_id
            }) {
                groups.push(RepositoryNodeGroup {
                    definition: kind.clone(),
                    query: self.for_browser_node(&raw),
                });
            } else {
                nodes.push(RepositoryNode::from_raw(raw, self, target)?);
            };
        }

        Ok(RepositoryNodes {
            objects: nodes,
            groups,
            categories: data.categories.items,
            object_types: data.object_types.items,
        })
    }
}

impl<T> ObjectKey<T> {
    /// Starts repository-browser navigation at this object.
    pub fn repository_nodes(&self) -> RepositoryNodesQuery {
        RepositoryNodesQuery::new(self)
    }
}

impl<T> ObjectRef<T> {
    /// Starts repository-browser navigation using this object's logical identity.
    /// The node-structure endpoint addresses trees by type and name, not URI.
    pub fn repository_nodes(&self) -> RepositoryNodesQuery {
        self.key().repository_nodes()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NodeOptions {
    user_name: Option<String>,
    short_descriptions: bool,
    with_versions: bool,
    exclude_local_packages: bool,
}

/// One repository-browser response, preserving backend ordering.
///
/// # Objects and folders
///
/// SAP represents initial type folders in `OBJECT_TYPES`. Expanding a folder can
/// return both real members and further folders, for example:
///
/// ```text
/// Class
/// └── Methods
///     ├── Inherited Methods   Folder
///     ├── Redefinitions      Folder
///     └── MY_METHOD          Actual method
/// ```
///
/// In the response for Methods, an actual method has its name and source location
/// in `TREE_CONTENT`. The Inherited Methods folder instead has a placeholder:
///
/// ```text
/// TREE_CONTENT field  Actual method                       Folder placeholder
/// ------------------  ----------------------------------  ------------------
/// OBJECT_TYPE         CLAS/OM                             CLAS/OM
/// OBJECT_NAME         MY_METHOD                           Empty
/// OBJECT_URI          .../source/main#start=42,9          Empty
/// EXPANDABLE          Empty                               X
/// NODE_ID             Empty                               000012
/// ```
///
/// A matching `OBJECT_TYPES` row with type `CLAS/OM` and node ID `000012` supplies
/// the label `Inherited Methods`. The decoder combines that metadata and the
/// expansion query from the placeholder into a [`RepositoryNodeGroup`], rather than
/// exposing an unnamed object alongside the folder. Both initial and nested
/// folders are therefore available through [`Self::groups`].
///
/// The type alone does not identify a folder. A row must also be unnamed,
/// locationless, expandable within the current tree, and matched to type metadata.
/// Unmatched rows remain in [`Self::objects`], even when they have no location.
/// Ordinary `OBJECT_TYPES` summary rows do not become additional folders.
#[derive(Clone, Debug)]
pub struct RepositoryNodes {
    objects: Vec<RepositoryNode>,
    groups: Vec<RepositoryNodeGroup>,
    categories: Vec<RepositoryNodeCategory>,
    object_types: Vec<RepositoryNodeType>,
}

impl RepositoryNodes {
    pub fn objects(&self) -> &[RepositoryNode] {
        &self.objects
    }

    pub fn categories(&self) -> &[RepositoryNodeCategory] {
        &self.categories
    }

    /// Type metadata is returned even for object-list responses. Those summary
    /// rows are not another layer of folders to recursively expand.
    pub fn object_types(&self) -> &[RepositoryNodeType] {
        &self.object_types
    }

    /// Initial type folders and nested folders with their expansion context attached.
    /// Plain type-summary rows remain available only through `object_types`.
    pub fn groups(&self) -> &[RepositoryNodeGroup] {
        &self.groups
    }

    /// Expands all folders together when they share a request context.
    /// Returns None for no folders or incompatible contexts.
    pub fn groups_query(&self) -> Option<RepositoryNodesQuery> {
        let first = self.groups.first()?.query();
        let context = first.for_nodes(Vec::new());
        let mut node_ids = Vec::new();
        for group in &self.groups {
            if group.query.for_nodes(Vec::new()) != context {
                return None;
            }
            for id in &group.query.node_ids {
                if !node_ids.contains(id) {
                    node_ids.push(id.clone());
                }
            }
        }
        Some(first.for_nodes(node_ids))
    }
}

/// A backend category label associated with repository object types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryNodeCategory {
    #[serde(rename = "CATEGORY")]
    pub name: String,

    #[serde(rename = "CATEGORY_LABEL")]
    pub label: String,
}

/// Type metadata returned with folders and object-list results.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryNodeType {
    #[serde(rename = "OBJECT_TYPE")]
    pub workbench_type: GlobalWorkbenchType,

    #[serde(rename = "CATEGORY_TAG", default)]
    pub category: String,

    #[serde(rename = "OBJECT_TYPE_LABEL", default)]
    pub label: String,

    #[serde(rename = "NODE_ID", default)]
    node_id: String,
}

/// A repository type folder with its expansion context attached.
#[derive(Clone, Debug)]
pub struct RepositoryNodeGroup {
    pub definition: RepositoryNodeType,
    query: RepositoryNodesQuery,
}

impl RepositoryNodeGroup {
    /// Expands this folder in its original tree, without reusing a rebuild flag.
    pub fn query(&self) -> RepositoryNodesQuery {
        self.query.clone()
    }
}

/// A repository object or member returned by the backend browser.
///
/// Navigation links can include a source fragment or query. [`Self::object_ref`]
/// returns an object reference only for a plain object-resource location.
#[derive(Clone, Debug)]
pub struct RepositoryNode {
    pub name: String,
    pub technical_name: String,
    pub workbench_type: GlobalWorkbenchType,
    pub location: Option<AdtLink>,
    pub virtual_workbench_location: Option<AdtLink>,
    pub expandable: bool,
    pub description: Option<String>,
    pub description_type: Option<String>,
    pub version: Option<String>,
    pub inactive_type: Option<String>,
    /// Backend visibility code, retained without interpreting its numeric vocabulary.
    pub visibility: Option<String>,
    pub is_final: bool,
    pub is_abstract: bool,
    pub is_for_testing: bool,
    pub is_event_handler: bool,
    pub is_constructor: bool,
    pub is_redefinition: bool,
    pub is_static: bool,
    pub is_read_only: bool,
    pub is_constant: bool,
    key: ObjectKey<()>,
    expansion: Option<RepositoryNodesQuery>,
}

impl RepositoryNode {
    /// Builds the next request when the backend marks this entry expandable.
    pub fn query(&self) -> Option<RepositoryNodesQuery> {
        self.expansion.clone()
    }

    /// Returns the located object when the advertised target has no query or fragment.
    /// Known parent/subobject relationships retain the logical parent key.
    pub fn object_ref(&self) -> Option<ObjectRef<()>> {
        let location = self.location.as_ref()?;
        if self.name.is_empty() || !location.query.is_empty() || location.fragment.is_some() {
            return None;
        }
        Some(ObjectRef::new(self.key.clone(), location.target.clone()))
    }

    fn from_raw(
        raw: RawNode,
        query: &RepositoryNodesQuery,
        base: &AdtUri,
    ) -> Result<Self, ObjectError> {
        let key = query
            .root
            .subobject(&raw.workbench_type, &raw.name)
            .unwrap_or_else(|_| {
                ObjectKey::from_parts(raw.name.clone(), raw.workbench_type.clone(), None)
            });
        let expansion = if raw.expandable {
            let next = if !raw.node_id.is_empty() && raw.node_id != ROOT_NODE {
                query.for_browser_node(&raw)
            } else {
                let mut next = RepositoryNodesQuery::new(&key);
                next.options = query.options.clone();
                if !raw.technical_name.is_empty() {
                    next.technical_name = Some(raw.technical_name.clone());
                }
                next
            };
            Some(next)
        } else {
            None
        };
        Ok(Self {
            location: node_location(raw.uri, base)?,
            virtual_workbench_location: node_location(raw.virtual_workbench_uri, base)?,
            name: raw.name,
            technical_name: raw.technical_name,
            workbench_type: raw.workbench_type,
            expandable: raw.expandable,
            description: nonempty(raw.description),
            description_type: nonempty(raw.description_type),
            version: nonempty(raw.version),
            inactive_type: nonempty(raw.inactive_type),
            visibility: nonempty(raw.visibility),
            is_final: raw.is_final,
            is_abstract: raw.is_abstract,
            is_for_testing: raw.is_for_testing,
            is_event_handler: raw.is_event_handler,
            is_constructor: raw.is_constructor,
            is_redefinition: raw.is_redefinition,
            is_static: raw.is_static,
            is_read_only: raw.is_read_only,
            is_constant: raw.is_constant,
            key,
            expansion,
        })
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn node_location(href: String, base: &AdtUri) -> Result<Option<AdtLink>, ObjectError> {
    if href.is_empty() {
        return Ok(None);
    }
    let resolved = resolve_href(base, &href).map_err(|source| ObjectError::InvalidLink {
        href: href.clone(),
        source,
    })?;
    Ok(Some(AdtLink {
        href,
        target: resolved.target,
        query: resolved.query,
        fragment: resolved.fragment,
        relation: None,
        media_type: None,
        hreflang: None,
        title: None,
        length: None,
        etag: None,
    }))
}

fn abap_bool<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    match String::deserialize(deserializer)?.as_str() {
        "X" => Ok(true),
        "" | " " => Ok(false),
        value => Err(serde::de::Error::custom(format!(
            "invalid ABAP boolean `{value}`"
        ))),
    }
}

/// ASX container for the node request. The data is borrowed only to be serialized
/// into an owned payload body so no extra cloning takes place.
#[derive(Serialize)]
#[serde(rename = "asx:abap")]
struct NodeRequest<'a> {
    #[serde(rename = "@version")]
    version: &'static str,
    #[serde(rename = "asx:values")]
    values: NodeRequestValues<'a>,
}

impl NodeRequest<'_> {
    fn serialize(&self) -> Result<String, RepositoryError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("asx", "http://www.sap.com/abapxml")
            .to_string(&self)
            .map_err(RepositoryError::InvalidRequest)
    }
}

#[derive(Serialize)]
struct NodeRequestValues<'a> {
    #[serde(rename = "DATA")]
    data: NodeIds<'a>,
}

#[derive(Serialize)]
struct NodeIds<'a> {
    #[serde(rename = "item")]
    items: &'a [String],
}

#[derive(Deserialize)]
#[serde(rename = "asx:abap", deny_unknown_fields)]
struct RawNodes {
    #[serde(rename = "@version", default)]
    _version: Option<String>,
    #[serde(rename = "asx:values")]
    values: RawNodeValues,
}

impl RawNodes {
    fn deserialize(body: &[u8]) -> Result<Self, RepositoryError> {
        serde_xml_rs::from_reader(body).map_err(RepositoryError::InvalidResponse)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNodeValues {
    #[serde(rename = "DATA")]
    data: RawNodeData,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNodeData {
    #[serde(rename = "TREE_CONTENT", default)]
    objects: RawObjectNodes,
    #[serde(rename = "CATEGORIES", default)]
    categories: RawCategories,
    #[serde(rename = "OBJECT_TYPES", default)]
    object_types: RawNodeTypes,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawObjectNodes {
    #[serde(rename = "SEU_ADT_REPOSITORY_OBJ_NODE", default)]
    items: Vec<RawNode>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCategories {
    #[serde(rename = "SEU_ADT_OBJECT_CATEGORY_INFO", default)]
    items: Vec<RepositoryNodeCategory>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNodeTypes {
    #[serde(rename = "SEU_ADT_OBJECT_TYPE_INFO", default)]
    items: Vec<RepositoryNodeType>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    #[serde(rename = "OBJECT_NAME")]
    name: String,
    #[serde(rename = "OBJECT_TYPE")]
    workbench_type: GlobalWorkbenchType,
    #[serde(rename = "TECH_NAME", default)]
    technical_name: String,
    #[serde(rename = "OBJECT_URI", default)]
    uri: String,
    #[serde(rename = "OBJECT_VIT_URI", default)]
    virtual_workbench_uri: String,
    #[serde(rename = "EXPANDABLE", default, deserialize_with = "abap_bool")]
    expandable: bool,
    #[serde(rename = "NODE_ID", default)]
    node_id: String,
    #[serde(rename = "PARENT_NAME", default)]
    parent_name: String,
    #[serde(rename = "DESCRIPTION")]
    description: Option<String>,
    #[serde(rename = "DESCRIPTION_TYPE")]
    description_type: Option<String>,
    #[serde(rename = "VERSION")]
    version: Option<String>,
    #[serde(rename = "INACTIVE_TYPE")]
    inactive_type: Option<String>,
    #[serde(rename = "VISIBILITY")]
    visibility: Option<String>,
    #[serde(rename = "IS_FINAL", default, deserialize_with = "abap_bool")]
    is_final: bool,
    #[serde(rename = "IS_ABSTRACT", default, deserialize_with = "abap_bool")]
    is_abstract: bool,
    #[serde(rename = "IS_FOR_TESTING", default, deserialize_with = "abap_bool")]
    is_for_testing: bool,
    #[serde(rename = "IS_EVENT_HANDLER", default, deserialize_with = "abap_bool")]
    is_event_handler: bool,
    #[serde(rename = "IS_CONSTRUCTOR", default, deserialize_with = "abap_bool")]
    is_constructor: bool,
    #[serde(rename = "IS_REDEFINITION", default, deserialize_with = "abap_bool")]
    is_redefinition: bool,
    #[serde(rename = "IS_STATIC", default, deserialize_with = "abap_bool")]
    is_static: bool,
    #[serde(rename = "IS_READ_ONLY", default, deserialize_with = "abap_bool")]
    is_read_only: bool,
    #[serde(rename = "IS_CONSTANT", default, deserialize_with = "abap_bool")]
    is_constant: bool,
}

impl RawNode {
    /// Whether this row can represent a folder whose label comes from OBJECT_TYPES.
    fn is_placeholder(&self) -> bool {
        self.name.is_empty()
            && self.uri.is_empty()
            && self.expandable
            && !self.node_id.is_empty()
            && self.node_id != ROOT_NODE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtResponse, Client, FunctionGroup, FunctionModule, Program};
    use http::{HeaderMap, header};

    const GROUPS: &[u8] = include_bytes!("../../../tests/fixtures/repository-nodes-fugr.xml");
    const CHILDREN: &[u8] =
        include_bytes!("../../../tests/fixtures/repository-nodes-fugr-children.xml");
    const DISCOVERY: &[u8] = br#"<app:service xmlns:app="http://www.w3.org/2007/app" xmlns:atom="http://www.w3.org/2005/Atom">
      <app:workspace><atom:title>Repository</atom:title>
        <app:collection href="/sap/bc/adt/custom/nodes">
          <atom:category scheme="http://www.sap.com/adt/categories/respository" term="nodestructure"/>
        </app:collection>
      </app:workspace>
    </app:service>"#;

    fn client() -> Client<Discovery> {
        super::super::tests::discovered_client(DISCOVERY)
    }

    fn root() -> RepositoryNodesQuery {
        ObjectKey::<FunctionGroup>::new("ZGROUP123").repository_nodes()
    }

    fn decode(query: &RepositoryNodesQuery, xml: &[u8]) -> Result<RepositoryNodes, ResponseError> {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE,
            "application/vnd.sap.as+xml; charset=utf-8; dataname=com.sap.adt.RepositoryObjectTreeContent".parse().unwrap());
        query.decode(OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, xml.to_vec()),
            AdtUri::parse("/sap/bc/adt/repository/nodestructure").unwrap(),
        ))
    }

    fn one_node(fields: &str) -> String {
        format!(
            r#"<asx:abap xmlns:asx="http://www.sap.com/abapxml" version="1.0"><asx:values><DATA><TREE_CONTENT><SEU_ADT_REPOSITORY_OBJ_NODE>{fields}</SEU_ADT_REPOSITORY_OBJ_NODE></TREE_CONTENT></DATA></asx:values></asx:abap>"#
        )
    }

    #[test]
    fn initial_query_uses_discovery_and_the_asxml_root_selector() {
        fn stateless<O: Operation<Kind = Stateless, ResolutionRequirement = RequiresDiscovery>>() {}
        stateless::<RepositoryNodesQuery>();
        let client = client();
        let encoded = root().encode(client.discovery()).unwrap();
        assert_eq!(encoded.method(), Method::POST);
        assert_eq!(encoded.target().as_str(), "/sap/bc/adt/custom/nodes");
        assert_eq!(
            encoded.query(),
            [
                ("parent_type".to_owned(), "FUGR/F".to_owned()),
                ("parent_name".to_owned(), "ZGROUP123".to_owned()),
            ]
        );
        assert_eq!(
            encoded.headers()[header::CONTENT_TYPE],
            RepositoryNodesQuery::REQUEST_MEDIA_TYPE
        );
        assert_eq!(
            encoded.headers()[header::ACCEPT],
            RepositoryNodesQuery::RESPONSE_MEDIA_TYPE
        );
        let body = std::str::from_utf8(encoded.body()).unwrap();
        assert!(body.contains("xmlns:asx=\"http://www.sap.com/abapxml\""));
        assert!(body.contains("<asx:values><DATA><item>000000</item></DATA></asx:values>"));
    }

    #[test]
    fn returned_groups_retain_context_and_options_but_not_rebuild() {
        let query = root()
            .technical_name("SAPLZGROUP123")
            .user_name("OTHER_USER")
            .short_descriptions(true)
            .with_versions(true)
            .exclude_local_packages(true)
            .rebuild(true);
        let response = decode(&query, GROUPS).unwrap();
        assert!(response.objects().is_empty());
        assert_eq!(response.categories()[0].name, "source_library");
        let groups = response.groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].definition.label, "Function Modules");
        assert_eq!(groups[1].definition.label, "Function Group Includes");
        assert_eq!(groups[2].definition.workbench_type.as_str(), "FUGR/PX");
        let next = groups[1].clone().query();
        drop(response);
        assert_eq!(next.root(), query.root());
        assert_eq!(next.technical_name.as_deref(), Some("SAPLZGROUP123"));
        assert_eq!(next.node_ids, ["000005"]);
        assert_eq!(next.options, query.options);
        assert!(!next.rebuild);
        let encoded = next.encode(client().discovery()).unwrap();
        assert!(
            encoded
                .query()
                .contains(&("parent_type".into(), "FUGR/F".into()))
        );
        assert!(!encoded.query().iter().any(|(key, _)| key == "rebuild_tree"));
        assert!(
            std::str::from_utf8(encoded.body())
                .unwrap()
                .contains("<item>000005</item>")
        );
    }

    #[test]
    fn all_groups_expand_together_and_child_type_summaries_are_not_new_folders() {
        let response = decode(&root(), GROUPS).unwrap();
        let query = response.groups_query().unwrap();
        assert_eq!(query.node_ids, ["000002", "000005", "000008"]);
        let encoded = query.encode(client().discovery()).unwrap();
        assert!(
            std::str::from_utf8(encoded.body())
                .unwrap()
                .contains("<item>000002</item><item>000005</item><item>000008</item>")
        );
        let response = decode(&query, CHILDREN).unwrap();
        assert_eq!(response.objects().len(), 4);
        assert_eq!(response.object_types().len(), 2);
        assert!(response.groups().is_empty());
        assert!(response.groups_query().is_none());
        let module = &response.objects()[0];
        assert_eq!(module.name, "ZFTFTR");
        assert_eq!(module.description.as_deref(), Some("tfartart"));
        assert!(!module.expandable);
        assert!(module.query().is_none());
        let located = module.object_ref().unwrap();
        assert_eq!(
            located.uri().as_str(),
            "/sap/bc/adt/functions/groups/zgroup123/fmodules/zftftr"
        );
        assert_eq!(located.key().parent(), Some(query.root()));
        assert!(located.typed::<FunctionModule>().is_some());
        assert!(response.objects()[2].description.is_none());
        assert!(response.objects()[2].version.is_none());
    }

    #[test]
    fn local_member_queries_keep_tree_identity_and_technical_context() {
        let query = ObjectKey::<Program>::new("Z_DISPLAY_NAME")
            .repository_nodes()
            .technical_name("Z_OLD_TECH_NAME")
            .user_name("USER")
            .short_descriptions(true);
        let xml = one_node(
            r#"
            <OBJECT_TYPE>PROG/PI</OBJECT_TYPE><OBJECT_NAME>LCL_HELPER</OBJECT_NAME>
            <TECH_NAME>Z_SOURCE</TECH_NAME><PARENT_NAME>Z_REAL_PARENT</PARENT_NAME>
            <NODE_ID>000042</NODE_ID><EXPANDABLE>X</EXPANDABLE>
            <OBJECT_URI>/sap/bc/adt/programs/programs/z_source/source/main?version=inactive#start=4,0</OBJECT_URI>
            <IS_ABSTRACT>X</IS_ABSTRACT><IS_STATIC>X</IS_STATIC><VISIBILITY>2</VISIBILITY>
        "#,
        );
        let response = decode(&query, xml.as_bytes()).unwrap();
        let member = &response.objects()[0];
        assert!(member.is_abstract && member.is_static);
        assert_eq!(member.visibility.as_deref(), Some("2"));
        assert!(member.object_ref().is_none());
        let location = member.location.as_ref().unwrap();
        assert_eq!(location.query, [("version".into(), "inactive".into())]);
        assert_eq!(location.fragment.as_deref(), Some("start=4,0"));
        let next = member.query().unwrap();
        assert_eq!(next.root().workbench_type().as_str(), "PROG/P");
        assert_eq!(next.root().name(), "Z_DISPLAY_NAME");
        assert_eq!(next.technical_name.as_deref(), Some("Z_SOURCE"));
        assert_eq!(next.node_ids, ["000042"]);
        assert_eq!(next.options, query.options);
    }

    #[test]
    fn function_pool_parent_name_does_not_replace_the_group_key() {
        let query = root();
        let xml = one_node(
            r#"<OBJECT_TYPE>PROG/PL</OBJECT_TYPE><OBJECT_NAME>LCL_HELPER</OBJECT_NAME>
          <PARENT_NAME>SAPLZGROUP123</PARENT_NAME><NODE_ID>000042</NODE_ID><EXPANDABLE>X</EXPANDABLE>"#,
        );
        let result = decode(&query, xml.as_bytes()).unwrap();
        let next = result.objects()[0].query().unwrap();
        assert_eq!(next.root(), query.root());
        assert_eq!(next.technical_name.as_deref(), Some("SAPLZGROUP123"));
        assert_eq!(next.node_ids, ["000042"]);
    }

    #[test]
    fn nested_object_without_a_browser_id_starts_its_own_tree() {
        let query = root()
            .technical_name("SAPLZGROUP123")
            .with_versions(true)
            .rebuild(true);
        let xml = one_node(
            r#"<OBJECT_TYPE>FUTR/XX</OBJECT_TYPE><OBJECT_NAME>Z_FUTURE</OBJECT_NAME>
          <TECH_NAME>Z_FUTURE_TECH</TECH_NAME><EXPANDABLE>X</EXPANDABLE>
          <OBJECT_URI>/sap/bc/adt/future/z_future</OBJECT_URI>"#,
        );
        let response = decode(&query, xml.as_bytes()).unwrap();
        let node = &response.objects()[0];
        let next = node.query().unwrap();
        assert_eq!(next.root().workbench_type().as_str(), "FUTR/XX");
        assert_eq!(next.root().name(), "Z_FUTURE");
        assert_eq!(next.technical_name.as_deref(), Some("Z_FUTURE_TECH"));
        assert_eq!(next.node_ids, [ROOT_NODE]);
        assert_eq!(next.options, query.options);
        assert!(!next.rebuild);
        assert!(next.encode(client().discovery()).is_ok());
        assert!(node.object_ref().unwrap().key().parent().is_none());
    }

    #[test]
    fn locationless_rows_without_matching_folder_metadata_remain_objects() {
        for fields in [
            "<OBJECT_NAME>NAMED_OBJECT</OBJECT_NAME><EXPANDABLE>X</EXPANDABLE><NODE_ID>000042</NODE_ID>",
            "<OBJECT_NAME/><EXPANDABLE>X</EXPANDABLE><NODE_ID>000042</NODE_ID>",
            "<OBJECT_NAME/><EXPANDABLE/><NODE_ID>000042</NODE_ID>",
            "<OBJECT_NAME/><EXPANDABLE>X</EXPANDABLE><NODE_ID>000000</NODE_ID>",
        ] {
            let xml = one_node(&format!("<OBJECT_TYPE>CLAS/OM</OBJECT_TYPE>{fields}"));
            let result = decode(&root(), xml.as_bytes()).unwrap();
            assert_eq!(result.objects().len(), 1);
            assert!(result.groups().is_empty());
        }
    }

    #[test]
    fn nested_unnamed_rows_bind_type_folder_labels_and_context() {
        let query = ObjectKey::<crate::Class>::new("ZCL_DEMO")
            .repository_nodes()
            .for_nodes(vec!["000011".into()]);
        let xml = br#"<asx:abap xmlns:asx="http://www.sap.com/abapxml" version="1.0"><asx:values><DATA>
          <TREE_CONTENT>
            <SEU_ADT_REPOSITORY_OBJ_NODE><OBJECT_TYPE>CLAS/OM</OBJECT_TYPE><OBJECT_NAME/><TECH_NAME>ZCL_DEMO</TECH_NAME><OBJECT_URI/><EXPANDABLE>X</EXPANDABLE><NODE_ID>000012</NODE_ID></SEU_ADT_REPOSITORY_OBJ_NODE>
            <SEU_ADT_REPOSITORY_OBJ_NODE><OBJECT_TYPE>CLAS/OR</OBJECT_TYPE><OBJECT_NAME/><TECH_NAME>ZCL_DEMO</TECH_NAME><OBJECT_URI/><EXPANDABLE>X</EXPANDABLE><NODE_ID>000020</NODE_ID></SEU_ADT_REPOSITORY_OBJ_NODE>
            <SEU_ADT_REPOSITORY_OBJ_NODE><OBJECT_TYPE>CLAS/OM</OBJECT_TYPE><OBJECT_NAME>LOCAL_METHOD</OBJECT_NAME><OBJECT_URI>/sap/bc/adt/oo/classes/zcl_demo/source/main#start=41,9</OBJECT_URI><EXPANDABLE/></SEU_ADT_REPOSITORY_OBJ_NODE>
          </TREE_CONTENT>
          <OBJECT_TYPES>
            <SEU_ADT_OBJECT_TYPE_INFO><OBJECT_TYPE>CLAS/OM</OBJECT_TYPE><OBJECT_TYPE_LABEL>Inherited Methods</OBJECT_TYPE_LABEL><NODE_ID>000012</NODE_ID></SEU_ADT_OBJECT_TYPE_INFO>
            <SEU_ADT_OBJECT_TYPE_INFO><OBJECT_TYPE>CLAS/OR</OBJECT_TYPE><OBJECT_TYPE_LABEL>Redefinitions</OBJECT_TYPE_LABEL><NODE_ID>000020</NODE_ID></SEU_ADT_OBJECT_TYPE_INFO>
          </OBJECT_TYPES>
        </DATA></asx:values></asx:abap>"#;
        let result = decode(&query, xml).unwrap();
        assert_eq!(result.objects().len(), 1);
        assert_eq!(result.objects()[0].name, "LOCAL_METHOD");
        assert_eq!(result.groups().len(), 2);
        assert_eq!(result.groups()[0].definition.label, "Inherited Methods");
        let next = result.groups_query().unwrap();
        assert_eq!(next.root(), query.root());
        assert_eq!(next.node_ids, ["000012", "000020"]);
        assert_eq!(next.technical_name.as_deref(), Some("ZCL_DEMO"));
    }

    #[test]
    fn false_options_are_omitted_and_rebuild_uses_abap_true() {
        let encoded = root()
            .short_descriptions(true)
            .with_versions(true)
            .exclude_local_packages(true)
            .rebuild(true)
            .encode(client().discovery())
            .unwrap();
        for pair in [
            ("withShortDescriptions", "true"),
            ("withVersions", "true"),
            ("exclude_local_packages", "X"),
            ("rebuild_tree", "X"),
        ] {
            assert!(encoded.query().contains(&(pair.0.into(), pair.1.into())));
        }
        let encoded = root()
            .short_descriptions(false)
            .with_versions(false)
            .exclude_local_packages(false)
            .rebuild(false)
            .encode(client().discovery())
            .unwrap();
        assert_eq!(encoded.query().len(), 2);
    }

    #[test]
    fn names_are_query_values_and_no_object_resolution_is_required() {
        let root = ObjectKey::<FunctionGroup>::new("/ACME/GROUP");
        let located = ObjectRef::new(
            root.clone(),
            AdtUri::parse("/sap/bc/adt/different/location").unwrap(),
        );
        let query = located.repository_nodes().technical_name("/ACME/SAPLGROUP");
        assert_eq!(query.root(), &root.erase());
        let encoded = query.encode(client().discovery()).unwrap();
        assert_eq!(encoded.target().as_str(), "/sap/bc/adt/custom/nodes");
        assert!(
            encoded
                .query()
                .contains(&("parent_name".into(), "/ACME/GROUP".into()))
        );
        assert!(
            encoded
                .query()
                .contains(&("parent_tech_name".into(), "/ACME/SAPLGROUP".into()))
        );
        let missing = super::super::tests::discovered_client(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"/>"#,
        );
        assert!(matches!(
            query.encode(missing.discovery()),
            Err(EncodeError::Resolve(_))
        ));
    }

    #[test]
    fn status_and_content_type_are_checked_before_decoding() {
        let query = root();
        for (status, content_type) in [
            (
                StatusCode::NOT_FOUND,
                RepositoryNodesQuery::RESPONSE_MEDIA_TYPE,
            ),
            (StatusCode::OK, "text/plain"),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
            let result = query.decode(OperationResponse::new(
                AdtResponse::new(status, headers, GROUPS.to_vec()),
                AdtUri::parse("/sap/bc/adt/repository/nodestructure").unwrap(),
            ));
            assert!(result.is_err());
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            RepositoryNodesQuery::RESPONSE_MEDIA_TYPE.parse().unwrap(),
        );
        assert!(
            query
                .decode(OperationResponse::new(
                    AdtResponse::new(StatusCode::OK, headers, GROUPS.to_vec()),
                    AdtUri::parse("/sap/bc/adt/repository/nodestructure").unwrap(),
                ))
                .is_ok()
        );
    }

    #[test]
    fn empty_results_do_not_produce_an_accidental_root_query() {
        let xml = br#"<asx:abap xmlns:asx="http://www.sap.com/abapxml" version="1.0"><asx:values><DATA/></asx:values></asx:abap>"#;
        let response = decode(&root(), xml).unwrap();
        assert!(response.objects().is_empty());
        assert!(response.groups().is_empty());
        assert!(response.groups_query().is_none());
    }

    #[test]
    fn unknown_shapes_invalid_booleans_and_unsafe_locations_fail() {
        let groups = std::str::from_utf8(GROUPS).unwrap();
        for tag in [
            "asx:abap",
            "asx:values",
            "DATA",
            "OBJECT_TYPES",
            "SEU_ADT_OBJECT_TYPE_INFO",
        ] {
            let changed = groups.replace(&format!("</{tag}>"), &format!("<unexpected/></{tag}>"));
            assert!(decode(&root(), changed.as_bytes()).is_err(), "{tag}");
        }
        for fields in [
            "<EXPANDABLE>false</EXPANDABLE>",
            "<OBJECT_URI>https://elsewhere.invalid/sap/bc/adt/object</OBJECT_URI>",
            "<OBJECT_URI>//elsewhere.invalid/object</OBJECT_URI>",
            "<UNKNOWN/>",
        ] {
            let xml = one_node(&format!(
                "<OBJECT_TYPE>FUGR/FF</OBJECT_TYPE><OBJECT_NAME>Z_FM</OBJECT_NAME>{fields}"
            ));
            assert!(decode(&root(), xml.as_bytes()).is_err(), "{fields}");
        }
    }
}
