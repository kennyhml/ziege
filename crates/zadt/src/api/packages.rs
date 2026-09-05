use std::collections::HashMap;

use http::{Method, StatusCode};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    AdtUri, AdvertisedObjectReference, CategoryId, Discovery, EncodeError, EncodedOperation,
    GlobalWorkbenchType, ObjectError, ObjectRef, ObjectType, Operation, OperationResponse, Package,
    PrimaryObjectType, RequiresDiscovery, ResolveError, ResponseError, Stateless,
    resource::{AdtUriTemplate, resolve_href},
};

/// A typed package reference and its optional short description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageReference {
    /// The typed package resource.
    pub reference: ObjectRef<Package>,
    /// The package short description, when advertised.
    pub description: Option<String>,
}

/// A package-interface reference advertised through a package representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageInterfaceReference {
    /// The package-interface name.
    pub name: String,
    /// The validated package-interface resource URI.
    pub uri: AdtUri,
    /// The wire object type, either `PINF/KI` or compact `PINFKI`.
    pub object_type: String,
    /// The package-interface description, when advertised.
    pub description: Option<String>,
}

impl PackageInterfaceReference {
    const OBJECT_TYPE: &str = "PINF/KI";
    const COMPACT_OBJECT_TYPE: &str = "PINFKI";
}

/// Which side of a package hierarchy to request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PackageTreeKind {
    /// Fetch the package and its ancestors.
    Super,
    /// Fetch the package's immediate subpackages.
    Sub,
}

impl PackageTreeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Super => "super",
            Self::Sub => "sub",
        }
    }
}

/// A package hierarchy returned by the package tree resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTree {
    /// Whether this response represents an ancestor tree.
    pub is_super_tree: bool,
    /// Package nodes in backend response order.
    pub nodes: Vec<PackageTreeNode>,
}

impl PackageTree {
    const COMPACT_OBJECT_TYPE: &str = "DEVCK";
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.packages.tree.v1+xml";

    pub(crate) fn parse(body: &[u8], base: &AdtUri) -> Result<Self, ResponseError> {
        let raw: RawPackageTree =
            serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)?;
        let nodes = raw
            .nodes
            .into_iter()
            .map(|node| PackageTreeNode::from_raw(node, base))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            is_super_tree: raw.is_super_tree,
            nodes,
        })
    }
}

/// One package and its interfaces in a package hierarchy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTreeNode {
    /// The package represented by this node.
    pub package: PackageReference,
    /// Whether the package is encapsulated.
    pub encapsulated: bool,
    /// Whether the package has direct subpackages.
    pub has_subpackages: bool,
    /// Whether the package defines package interfaces.
    pub has_interfaces: bool,
    /// The direct parent package, when advertised.
    pub super_package: Option<PackageReference>,
    /// Interfaces defined by this package.
    pub package_interfaces: Vec<PackageInterfaceReference>,
}

impl PackageTreeNode {
    fn from_raw(raw: RawPackageTreeNode, base: &AdtUri) -> Result<Self, ObjectError> {
        let package = package_reference(
            AdvertisedObjectReference {
                uri: Some(raw.uri),
                object_type: Some(raw.object_type),
                name: Some(raw.name),
                description: raw.description,
                ..Default::default()
            },
            base,
            true,
        )?
        .ok_or(ObjectError::IncompleteObjectReference {
            field: "adtcore:name",
        })?;
        let super_package = package_reference(raw.super_package, base, true)?;
        let package_interfaces = raw
            .package_interfaces
            .items
            .into_iter()
            .map(|item| package_interface_reference(item, base))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            package,
            encapsulated: raw.encapsulated,
            has_subpackages: raw.has_subpackages,
            has_interfaces: raw.has_interfaces,
            super_package,
            package_interfaces,
        })
    }
}

/// Global package editor settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageSettings {
    /// Whether package-check errors should be shown in the package editor.
    pub show_package_check_errors: bool,
}

impl PackageSettings {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.packages.settings+xml";

    pub(crate) fn parse(body: &[u8]) -> Result<Self, ResponseError> {
        let raw: RawPackageSettings =
            serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)?;
        Ok(Self {
            show_package_check_errors: raw.show_package_check_errors,
        })
    }
}

fn package_reference(
    raw: AdvertisedObjectReference,
    base: &AdtUri,
    compact_type: bool,
) -> Result<Option<PackageReference>, ObjectError> {
    if raw.name.is_none() && raw.uri.is_none() && raw.object_type.is_none() {
        return Ok(None);
    }
    let name = required(raw.name, "adtcore:name")?;
    let href = required(raw.uri, "adtcore:uri")?;
    let object_type = required(raw.object_type, "adtcore:type")?;
    if compact_type {
        if object_type.as_str() != PackageTree::COMPACT_OBJECT_TYPE {
            return Err(ObjectError::UnexpectedCompactObjectType {
                expected: PackageTree::COMPACT_OBJECT_TYPE,
                actual: object_type.to_string(),
            });
        }
    } else if object_type != Package::WORKBENCH_TYPE {
        return Err(ObjectError::UnexpectedObjectType {
            expected: Package::WORKBENCH_TYPE,
            actual: object_type,
        });
    }
    resolve_href(base, &href).map_err(|source| ObjectError::InvalidLink {
        href: href.clone(),
        source,
    })?;
    let reference = ObjectRef::<Package>::new(name);
    Ok(Some(PackageReference {
        reference,
        description: raw.description,
    }))
}

fn package_interface_reference(
    raw: AdvertisedObjectReference,
    base: &AdtUri,
) -> Result<PackageInterfaceReference, ObjectError> {
    let name = required(raw.name, "adtcore:name")?;
    let href = required(raw.uri, "adtcore:uri")?;
    let object_type = required(raw.object_type, "adtcore:type")?;
    if object_type.as_str() != PackageInterfaceReference::OBJECT_TYPE
        && object_type.as_str() != PackageInterfaceReference::COMPACT_OBJECT_TYPE
    {
        return Err(ObjectError::UnexpectedCompactObjectType {
            expected: PackageInterfaceReference::OBJECT_TYPE,
            actual: object_type.to_string(),
        });
    }
    let uri = resolve_href(base, &href)
        .map_err(|source| ObjectError::InvalidLink {
            href: href.clone(),
            source,
        })?
        .target;
    Ok(PackageInterfaceReference {
        name,
        uri,
        object_type: object_type.to_string(),
        description: raw.description,
    })
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, ObjectError> {
    value.ok_or(ObjectError::IncompleteObjectReference { field })
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackageInterfaces {
    #[serde(rename = "@pak:isVisible", default)]
    _visible: bool,
    #[serde(rename = "pak:packageInterfaceRef", default)]
    items: Vec<AdvertisedObjectReference>,
}

#[derive(Deserialize)]
#[serde(rename = "pak:packageTree", deny_unknown_fields)]
struct RawPackageTree {
    #[serde(rename = "@pak:isSuperTree")]
    is_super_tree: bool,
    #[serde(rename = "pak:treeNode", default)]
    nodes: Vec<RawPackageTreeNode>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackageTreeNode {
    #[serde(rename = "@adtcore:uri")]
    uri: String,
    #[serde(rename = "@adtcore:type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:name")]
    name: String,
    #[serde(rename = "@adtcore:description")]
    description: Option<String>,
    #[serde(rename = "@pak:isEncapsulated")]
    encapsulated: bool,
    #[serde(rename = "@pak:hasSubpackages")]
    has_subpackages: bool,
    #[serde(rename = "@pak:hasInterfaces")]
    has_interfaces: bool,
    #[serde(rename = "pak:superPackageRef", default)]
    super_package: AdvertisedObjectReference,
    #[serde(rename = "pak:packageInterfaces", default)]
    package_interfaces: RawPackageInterfaces,
}

#[derive(Deserialize)]
#[serde(rename = "pkcs:settings", deny_unknown_fields)]
struct RawPackageSettings {
    #[serde(rename = "@pkcs:showPackageCheckErrors")]
    show_package_check_errors: bool,
}

/// Fetches either the ancestors or immediate children of a package.
#[derive(Clone, Debug)]
pub struct PackageTreeQuery {
    /// The package at which tree traversal starts.
    pub package: ObjectRef<Package>,

    /// Whether ancestors or immediate subpackages are requested.
    pub kind: PackageTreeKind,
}

impl PackageTreeQuery {
    const RELATION: &str = "tree";

    /// Creates a package-tree query with the selected direction.
    pub fn new(package: ObjectRef<Package>, kind: PackageTreeKind) -> Self {
        Self { package, kind }
    }
}

impl Operation for PackageTreeQuery {
    type Response = PackageTree;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let link = resolver.require_template(Package::CATEGORY, Self::RELATION)?;
        let template = AdtUriTemplate::new(link.template());
        for name in ["packagename", "type"] {
            if !template.has_variable(name) {
                return Err(
                    ResolveError::from(ObjectError::UnsupportedTemplateParameter {
                        parameter: name,
                    })
                    .into(),
                );
            }
        }
        let variables = HashMap::from([
            (
                "packagename".to_owned(),
                Value::String(self.package.name().to_owned()),
            ),
            (
                "type".to_owned(),
                Value::String(self.kind.as_str().to_owned()),
            ),
        ]);
        let (target, query) = template.expand(&variables).map_err(ResolveError::from)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        for (name, value) in query {
            request.push_query(name, value);
        }
        request.set_accept(PackageTree::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ensure_xml_response(&response, PackageTree::MEDIA_TYPE)?;
        PackageTree::parse(response.body(), response.request_target())
    }
}

/// Fetches global package editor settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct PackageSettingsQuery;

impl PackageSettingsQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/wbobj/packages",
        term: "settings",
    };
}

impl Operation for PackageSettingsQuery {
    type Response = PackageSettings;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        request.set_accept(PackageSettings::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ensure_xml_response(&response, PackageSettings::MEDIA_TYPE)?;
        PackageSettings::parse(response.body())
    }
}

impl ObjectRef<Package> {
    /// Creates a query for this package and all of its ancestors.
    pub fn super_tree(&self) -> PackageTreeQuery {
        PackageTreeQuery::new(self.clone(), PackageTreeKind::Super)
    }

    /// Creates a query for this package's immediate subpackages.
    pub fn sub_tree(&self) -> PackageTreeQuery {
        PackageTreeQuery::new(self.clone(), PackageTreeKind::Sub)
    }
}

fn ensure_xml_response(
    response: &OperationResponse,
    expected_content_type: &'static str,
) -> Result<(), ResponseError> {
    response.require_status(StatusCode::OK)?;
    response.require_content_type(&[expected_content_type])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, header};

    use super::*;
    use crate::{
        AdtRequest, AdtResponse, Client, OperationError, PackageProperties, Transport,
        TransportError,
    };

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");
    const PACKAGE_XML: &[u8] = include_bytes!("../../tests/fixtures/package-sadt-tools-core.xml");
    const SUPER_TREE_XML: &[u8] = include_bytes!("../../tests/fixtures/package-tree-super.xml");
    const SETTINGS_XML: &[u8] = include_bytes!("../../tests/fixtures/package-settings.xml");

    struct RecordingTransport {
        requests: Arc<Mutex<Vec<AdtRequest>>>,
    }

    #[async_trait]
    impl Transport for RecordingTransport {
        async fn send(&self, request: AdtRequest) -> Result<AdtResponse, TransportError> {
            self.requests.lock().unwrap().push(request);
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(PackageTree::MEDIA_TYPE),
            );
            Ok(AdtResponse::new(
                StatusCode::OK,
                headers,
                SUPER_TREE_XML.to_vec(),
            ))
        }
    }

    fn discovered_client(xml: &[u8]) -> (Client<Discovery>, Arc<Mutex<Vec<AdtRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = Client::new(RecordingTransport {
            requests: Arc::clone(&requests),
        })
        .with_capabilities(
            crate::api::discovery::parse_capabilities(xml).unwrap(),
            crate::api::discovery::parse_capabilities(xml).unwrap(),
        );
        (client, requests)
    }

    #[test]
    fn parses_live_package_properties() {
        let properties: PackageProperties = serde_xml_rs::from_reader(PACKAGE_XML).unwrap();

        assert_eq!(properties.name, "SADT_TOOLS_CORE");
        assert_eq!(properties.object_type, Package::WORKBENCH_TYPE);
    }

    #[test]
    fn preserves_advertised_root_identity() {
        let xml = String::from_utf8(PACKAGE_XML.to_vec())
            .unwrap()
            .replacen("adtcore:type=\"DEVC/K\"", "adtcore:type=\"PROG/P\"", 1)
            .replacen(
                "adtcore:name=\"SADT_TOOLS_CORE\"",
                "adtcore:name=\"OTHER_PACKAGE\"",
                1,
            );
        let properties: PackageProperties = serde_xml_rs::from_str(&xml).unwrap();

        assert_eq!(properties.object_type.as_str(), "PROG/P");
        assert_eq!(properties.name, "OTHER_PACKAGE");
    }

    #[test]
    fn rejects_unknown_package_tree_fields_including_interface_wrappers() {
        let xml = std::str::from_utf8(SUPER_TREE_XML).unwrap();
        let base = AdtUri::parse("/sap/bc/adt/packages/$tree").unwrap();
        for tag in ["pak:packageTree", "pak:treeNode", "pak:packageInterfaces"] {
            for (from, to) in [
                (format!("<{tag}"), format!("<{tag} unexpected=\"true\"")),
                (format!("</{tag}>"), format!("<unexpected/></{tag}>")),
            ] {
                let body = xml.replacen(&from, &to, 1);
                let error = PackageTree::parse(body.as_bytes(), &base)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("unknown field"), "{tag}: {error}");
                assert!(error.contains("unexpected"), "{tag}: {error}");
            }
        }
    }

    #[test]
    fn parses_live_package_tree() {
        let base = AdtUri::parse("/sap/bc/adt/packages/$tree").unwrap();
        let tree = PackageTree::parse(SUPER_TREE_XML, &base).unwrap();

        assert!(tree.is_super_tree);
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].package.reference.name(), "SADT_TOOLS_CORE");
        assert_eq!(
            tree.nodes[0]
                .super_package
                .as_ref()
                .unwrap()
                .reference
                .name(),
            "SADT_MAIN"
        );
        assert_eq!(tree.nodes[0].package_interfaces.len(), 1);
        assert!(tree.nodes[1].has_subpackages);
    }

    #[test]
    fn rejects_an_unexpected_compact_tree_type() {
        let xml = String::from_utf8(SUPER_TREE_XML.to_vec())
            .unwrap()
            .replacen("adtcore:type=\"DEVCK\"", "adtcore:type=\"PROGP\"", 1);

        let base = AdtUri::parse("/sap/bc/adt/packages/$tree").unwrap();
        let error = PackageTree::parse(xml.as_bytes(), &base).unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::UnexpectedCompactObjectType {
                expected: PackageTree::COMPACT_OBJECT_TYPE,
                actual,
            }) if actual == "PROGP"
        ));
    }

    #[test]
    fn parses_package_settings() {
        assert_eq!(
            PackageSettings::parse(SETTINGS_XML).unwrap(),
            PackageSettings {
                show_package_check_errors: false
            }
        );
    }

    #[tokio::test]
    async fn expands_namespaced_package_tree_targets() {
        let package = ObjectRef::<Package>::new("/DMO/FLIGHT");
        let (client, requests) = discovered_client(DISCOVERY_XML);
        PackageTreeQuery::new(package, PackageTreeKind::Super)
            .execute(&client)
            .await
            .unwrap();
        let requests = requests.lock().unwrap();
        let request = &requests[0];

        assert_eq!(request.target().as_str(), "/sap/bc/adt/packages/$tree");
        assert_eq!(
            request.query(),
            [
                ("packagename".to_owned(), "/DMO/FLIGHT".to_owned()),
                ("type".to_owned(), "super".to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn follows_advertised_package_tree_variable_locations() {
        let discovery = String::from_utf8(DISCOVERY_XML.to_vec()).unwrap().replacen(
            "/sap/bc/adt/packages/$tree{?packagename,type}",
            "/sap/bc/adt/packages/$tree/{packagename}/{type}",
            1,
        );
        let (client, requests) = discovered_client(discovery.as_bytes());
        let package = ObjectRef::<Package>::new("SADT_MAIN");

        PackageTreeQuery::new(package, PackageTreeKind::Sub)
            .execute(&client)
            .await
            .unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(
            requests[0].target().as_str(),
            "/sap/bc/adt/packages/$tree/SADT_MAIN/sub"
        );
        assert!(requests[0].query().is_empty());
    }

    #[tokio::test]
    async fn rejects_a_tree_template_without_a_supplied_variable() {
        let discovery = String::from_utf8(DISCOVERY_XML.to_vec()).unwrap().replacen(
            "{?packagename,type}",
            "{?packagename}",
            1,
        );
        let (client, _) = discovered_client(discovery.as_bytes());
        let package = ObjectRef::<Package>::new("SADT_MAIN");

        let error = PackageTreeQuery::new(package, PackageTreeKind::Sub)
            .execute(&client)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OperationError::Encode(EncodeError::Resolve(ResolveError::Object(
                ObjectError::UnsupportedTemplateParameter { parameter: "type" }
            )))
        ));
    }

    #[test]
    fn rejects_an_unexpected_settings_content_type() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml"),
        );
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, Vec::new()),
            AdtUri::parse("/sap/bc/adt/packages/settings").unwrap(),
        );

        let error = ensure_xml_response(&response, PackageSettings::MEDIA_TYPE).unwrap_err();

        assert!(matches!(
            error,
            ResponseError::UnsupportedContentType { content_type, .. }
                if content_type == "application/xml"
        ));
    }
}
