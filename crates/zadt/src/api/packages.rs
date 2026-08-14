use std::collections::HashMap;

use http::{Method, StatusCode, header};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    AdtUri, AdvertisedObjectReference, CategoryId, Client, GlobalWorkbenchType, ObjectError,
    ObjectRef, ObjectType, Operation, OperationError, OperationResponse, Package, Ready,
    ResponseError, Stateless,
    protocol::{AdtRequest, AdtResponse},
    resource::{AdtUriTemplate, resolve_href},
    target::{CollectionTarget, TemplateTarget},
};

const PACKAGE_TYPE_KEY: &str = "DEVCK";
const PACKAGE_INTERFACE_TYPE: &str = "PINF/KI";
const PACKAGE_INTERFACE_TYPE_KEY: &str = "PINFKI";
const PACKAGE_TREE_RELATION: &str = "tree";
const PACKAGE_TREE_MEDIA_TYPE: &str = "application/vnd.sap.adt.packages.tree.v1+xml";
const PACKAGE_SETTINGS_MEDIA_TYPE: &str = "application/vnd.sap.adt.packages.settings+xml";
const PACKAGE_SETTINGS: CategoryId = CategoryId {
    scheme: "http://www.sap.com/wbobj/packages",
    term: "settings",
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
        if object_type.as_str() != PACKAGE_TYPE_KEY {
            return Err(ObjectError::UnexpectedCompactObjectType {
                expected: PACKAGE_TYPE_KEY,
                actual: object_type.to_string(),
            });
        }
    } else if object_type != Package::WORKBENCH_TYPE {
        return Err(ObjectError::UnexpectedObjectType {
            expected: Package::WORKBENCH_TYPE,
            actual: object_type,
        });
    }
    let target = resolve_href(base, &href)
        .map_err(|source| ObjectError::InvalidLink {
            href: href.clone(),
            source,
        })?
        .target;
    let reference = ObjectRef::<Package>::new(name, target);
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
    if object_type.as_str() != PACKAGE_INTERFACE_TYPE
        && object_type.as_str() != PACKAGE_INTERFACE_TYPE_KEY
    {
        return Err(ObjectError::UnexpectedCompactObjectType {
            expected: PACKAGE_INTERFACE_TYPE,
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
struct RawPackageInterfaces {
    #[serde(rename = "@pak:isVisible", default)]
    _visible: bool,
    #[serde(rename = "pak:packageInterfaceRef", default)]
    items: Vec<AdvertisedObjectReference>,
}

#[derive(Deserialize)]
#[serde(rename = "pak:packageTree")]
struct RawPackageTree {
    #[serde(rename = "@pak:isSuperTree")]
    is_super_tree: bool,
    #[serde(rename = "pak:treeNode", default)]
    nodes: Vec<RawPackageTreeNode>,
}

#[derive(Deserialize)]
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
#[serde(rename = "pkcs:settings")]
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
    const TARGET: TemplateTarget = TemplateTarget::new(Package::CATEGORY, PACKAGE_TREE_RELATION);

    /// Creates a package-tree query with the selected direction.
    pub fn new(package: ObjectRef<Package>, kind: PackageTreeKind) -> Self {
        Self { package, kind }
    }
}

impl Operation<Ready> for PackageTreeQuery {
    type Response = PackageTree;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let template = Self::TARGET.template(client)?;
        let (target, query) =
            expand_tree_target(template.as_str(), self.package.name(), self.kind)?;
        let mut request = AdtRequest::new(Method::GET, target);
        for (name, value) in query {
            request.push_query(name, value);
        }
        request.set_accept(PACKAGE_TREE_MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ensure_xml_response(
            response.response(),
            Package::CATEGORY,
            PACKAGE_TREE_MEDIA_TYPE,
        )?;
        PackageTree::parse(response.body(), response.request_target())
    }
}

/// Fetches global package editor settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct PackageSettingsQuery;

impl Operation<Ready> for PackageSettingsQuery {
    type Response = PackageSettings;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let mut request = CollectionTarget::new(PACKAGE_SETTINGS).request(client, Method::GET)?;
        request.set_accept(PACKAGE_SETTINGS_MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ensure_xml_response(
            response.response(),
            PACKAGE_SETTINGS,
            PACKAGE_SETTINGS_MEDIA_TYPE,
        )?;
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

fn expand_tree_target(
    template: &str,
    package_name: &str,
    kind: PackageTreeKind,
) -> Result<(AdtUri, Vec<(String, String)>), OperationError> {
    let template = AdtUriTemplate::new(template);
    for expected in ["packagename", "type"] {
        if !template.has_variable(expected) {
            return Err(ObjectError::InvalidTemplate {
                template: template.as_str().to_owned(),
                reason: format!("missing `{expected}` query variable"),
            }
            .into());
        }
    }
    let variables = HashMap::from([
        (
            "packagename".to_owned(),
            Value::String(package_name.to_owned()),
        ),
        ("type".to_owned(), Value::String(kind.as_str().to_owned())),
    ]);
    let (target, query) = template.expand(&variables)?;
    for expected in ["packagename", "type"] {
        if !query.iter().any(|(name, _)| name == expected) {
            return Err(ObjectError::InvalidTemplate {
                template: template.as_str().to_owned(),
                reason: format!("missing `{expected}` query variable"),
            }
            .into());
        }
    }
    Ok((target, query))
}

fn ensure_xml_response(
    response: &AdtResponse,
    category: CategoryId,
    expected_content_type: &'static str,
) -> Result<(), ResponseError> {
    if response.status() != StatusCode::OK {
        return Err(ResponseError::unexpected_status(response));
    }
    let Some(content_type) = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    else {
        return Err(ResponseError::MissingContentType { category });
    };
    let matches = content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(expected_content_type));
    if !matches {
        return Err(ResponseError::UnsupportedContentType {
            category,
            content_type: content_type.to_owned(),
            supported: vec![expected_content_type.to_owned()],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue};

    use super::*;
    use crate::PackageProperties;

    const PACKAGE_XML: &[u8] = include_bytes!("../../tests/fixtures/package-sadt-tools-core.xml");
    const SUPER_TREE_XML: &[u8] = include_bytes!("../../tests/fixtures/package-tree-super.xml");
    const SETTINGS_XML: &[u8] = include_bytes!("../../tests/fixtures/package-settings.xml");

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
                expected: PACKAGE_TYPE_KEY,
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

    #[test]
    fn expands_namespaced_package_tree_targets() {
        let (target, query) = expand_tree_target(
            "/sap/bc/adt/packages/$tree{?packagename,type}",
            "/DMO/FLIGHT",
            PackageTreeKind::Super,
        )
        .unwrap();

        assert_eq!(target.as_str(), "/sap/bc/adt/packages/$tree");
        assert_eq!(
            query,
            [
                ("packagename".to_owned(), "/DMO/FLIGHT".to_owned()),
                ("type".to_owned(), "super".to_owned()),
            ]
        );
    }

    #[test]
    fn rejects_a_tree_template_without_required_variables() {
        let error = expand_tree_target(
            "/sap/bc/adt/packages/$tree{?packagename}",
            "SADT_MAIN",
            PackageTreeKind::Sub,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            OperationError::Object(ObjectError::InvalidTemplate {
                reason,
                ..
            }) if reason.contains("`type`")
        ));
    }

    #[test]
    fn rejects_an_unexpected_settings_content_type() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml"),
        );
        let response = AdtResponse::new(StatusCode::OK, headers, Vec::new());

        let error = ensure_xml_response(&response, PACKAGE_SETTINGS, PACKAGE_SETTINGS_MEDIA_TYPE)
            .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::UnsupportedContentType { content_type, .. }
                if content_type == "application/xml"
        ));
    }
}
