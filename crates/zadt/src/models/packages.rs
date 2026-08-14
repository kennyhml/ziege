use serde::{Deserialize, Serialize};

use crate::{
    AdtUri, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType, ObjectError, ObjectRef,
    ObjectType, Package, PropertyModel, ResponseError, resource::resolve_href,
};

const PACKAGE_TYPE_KEY: &str = "DEVCK";
const PACKAGE_INTERFACE_TYPE: &str = "PINF/KI";
const PACKAGE_INTERFACE_TYPE_KEY: &str = "PINFKI";

/// The SAP media-type version used to decode package properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PackagePropertiesVersion {
    /// Package properties V1.
    V1,

    /// Package properties V2.
    V2,
}

impl PackagePropertiesVersion {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::V1 => "application/vnd.sap.adt.packages.v1+xml",
            Self::V2 => "application/vnd.sap.adt.packages.v2+xml",
        }
    }
}

/// The currently modeled package-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "pak:package")]
pub struct PackageProperties {
    /// The package name supplied by SAP.
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    /// The repository object type, normally `DEVC/K`.
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which the package was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    /// The object version exactly as advertised by SAP.
    #[serde(rename = "@adtcore:version")]
    pub version: String,
    /// The timestamp at which the package was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    /// The user who last changed the package.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    /// The user who created the package.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,
    /// The package description.
    #[serde(rename = "@adtcore:description")]
    pub description: String,
    /// The maximum package-description length.
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,
    /// The package's logon language.
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    /// The user responsible for the package.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,
    /// The package's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,
    /// The package's master system, when advertised.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: Option<String>,
    /// Atom links exactly as advertised by the package representation.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
    /// Package behavior and editor capability flags.
    #[serde(rename = "pak:attributes")]
    pub attributes: PackageAttributes,
    /// The parent package, when this is not a root package.
    #[serde(rename = "pak:superPackage")]
    pub super_package: Option<AdvertisedObjectReference>,
    /// The assigned application component.
    #[serde(rename = "pak:applicationComponent")]
    pub application_component: PackageAssignment,
    /// Software-component and transport-layer assignments.
    #[serde(rename = "pak:transport")]
    pub transport: PackageTransport,
    /// Package-interface use accesses.
    #[serde(rename = "pak:useAccesses")]
    pub use_accesses: Option<PackageUseAccesses>,
    /// Interfaces defined by this package.
    #[serde(rename = "pak:packageInterfaces")]
    pub package_interfaces: Option<PackageInterfaces>,
    /// Direct subpackages included in the properties representation.
    #[serde(rename = "pak:subPackages")]
    pub sub_packages: Option<PackageSubpackages>,
}

impl PropertyModel for PackageProperties {
    type Version = PackagePropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] =
        &[PackagePropertiesVersion::V2, PackagePropertiesVersion::V1];

    fn media_type(version: Self::Version) -> &'static str {
        version.media_type()
    }
}

/// Package behavior and editor capability flags.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageAttributes {
    /// The semantic package type, such as `development`.
    #[serde(rename = "@pak:packageType")]
    pub package_type: String,
    /// Whether the package type is editable.
    #[serde(rename = "@pak:isPackageTypeEditable")]
    pub package_type_editable: bool,
    /// Whether repository objects can be assigned to the package.
    #[serde(rename = "@pak:isAddingObjectsAllowed")]
    pub adding_objects_allowed: bool,
    /// Whether object-assignment behavior is editable.
    #[serde(rename = "@pak:isAddingObjectsAllowedEditable")]
    pub adding_objects_allowed_editable: bool,
    /// Whether package encapsulation is enabled.
    #[serde(rename = "@pak:isEncapsulated")]
    pub encapsulated: bool,
    /// Whether encapsulation is editable.
    #[serde(rename = "@pak:isEncapsulationEditable")]
    pub encapsulation_editable: bool,
    /// Whether encapsulation is shown by the package editor.
    #[serde(rename = "@pak:isEncapsulationVisible")]
    pub encapsulation_visible: bool,
    /// Whether changes assigned to the package are recorded for transport.
    #[serde(rename = "@pak:recordChanges")]
    pub record_changes: bool,
    /// Whether change recording is editable.
    #[serde(rename = "@pak:isRecordChangesEditable")]
    pub record_changes_editable: bool,
    /// Whether switch assignment is shown by the package editor.
    #[serde(rename = "@pak:isSwitchVisible")]
    pub switch_visible: bool,
    /// The configured ABAP language version.
    #[serde(rename = "@pak:languageVersion", default)]
    pub language_version: String,
    /// Whether the language version is shown by the package editor.
    #[serde(rename = "@pak:isLanguageVersionVisible")]
    pub language_version_visible: bool,
    /// Whether the language version is editable.
    #[serde(rename = "@pak:isLanguageVersionEditable")]
    pub language_version_editable: bool,
}

/// A named package assignment with editor visibility and mutability flags.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageAssignment {
    /// The assigned value.
    #[serde(rename = "@pak:name", default)]
    pub name: String,
    /// The server-provided value description.
    #[serde(rename = "@pak:description", default)]
    pub description: String,
    /// Whether this assignment is shown by the package editor.
    #[serde(rename = "@pak:isVisible")]
    pub visible: bool,
    /// Whether this assignment is editable.
    #[serde(rename = "@pak:isEditable")]
    pub editable: bool,
}

/// Software-component and transport-layer assignments.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageTransport {
    /// The package's software component.
    #[serde(rename = "pak:softwareComponent")]
    pub software_component: PackageAssignment,
    /// The package's transport layer.
    #[serde(rename = "pak:transportLayer")]
    pub transport_layer: PackageAssignment,
}

/// Use-access visibility and entries in a package-properties payload.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageUseAccesses {
    #[serde(rename = "@pak:isVisible", default)]
    pub visible: bool,
    #[serde(rename = "pak:useAccess", default)]
    pub use_access: Vec<PackageUseAccess>,
}

/// A package-interface use access exactly as represented in package XML.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageUseAccess {
    #[serde(rename = "@pak:severity")]
    pub severity: String,
    #[serde(rename = "pak:packageInterfaceRef")]
    pub package_interface: AdvertisedObjectReference,
    #[serde(rename = "pak:packageRef")]
    pub package_ref: Option<AdvertisedObjectReference>,
}

/// Package-interface visibility and references in a package-properties payload.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageInterfaces {
    #[serde(rename = "@pak:isVisible", default)]
    pub visible: bool,
    #[serde(rename = "pak:packageInterfaceRef", default)]
    pub package_interface_ref: Vec<AdvertisedObjectReference>,
}

/// Direct subpackage references in a package-properties payload.
#[derive(Debug, Deserialize, Serialize)]
pub struct PackageSubpackages {
    #[serde(rename = "pak:packageRef", default)]
    pub package_ref: Vec<AdvertisedObjectReference>,
}

#[cfg(test)]
mod property_tests {
    use super::*;

    const PACKAGE_XML: &[u8] = include_bytes!("../../tests/fixtures/package-sadt-tools-core.xml");

    #[test]
    fn complete_wire_payload_has_canonical_wire_round_trip_json() {
        let properties: PackageProperties = serde_xml_rs::from_reader(PACKAGE_XML).unwrap();

        assert_eq!(properties.name, "SADT_TOOLS_CORE");
        assert_eq!(properties.object_type, Package::WORKBENCH_TYPE);
        assert_eq!(properties.version, "active");
        assert_eq!(properties.links.len(), 1);
        assert_eq!(properties.links[0].href, "versions");
        assert_eq!(
            properties.super_package.as_ref().unwrap().uri.as_deref(),
            Some("/sap/bc/adt/packages/sadt_main")
        );
        assert_eq!(
            properties.use_accesses.as_ref().unwrap().use_access[0]
                .package_interface
                .object_type
                .as_ref()
                .map(GlobalWorkbenchType::as_str),
            Some("PINF/KI")
        );
        assert_eq!(
            properties
                .package_interfaces
                .as_ref()
                .unwrap()
                .package_interface_ref
                .len(),
            1
        );
        assert_eq!(
            properties.sub_packages.as_ref().unwrap().package_ref.len(),
            1
        );

        let json = serde_json::to_value(&properties).unwrap();
        assert_eq!(json["@adtcore:type"], "DEVC/K");
        assert_eq!(
            json["pak:useAccesses"]["pak:useAccess"][0]["@pak:severity"],
            "none"
        );
        assert!(json.get("objectType").is_none());
        let roundtrip: PackageProperties = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(), json);
    }

    #[test]
    fn nested_wire_values_are_not_validated_or_resolved() {
        let xml = String::from_utf8(PACKAGE_XML.to_vec())
            .unwrap()
            .replacen(
                "adtcore:version=\"active\"",
                "adtcore:version=\"future\"",
                1,
            )
            .replacen(
                "adtcore:uri=\"/sap/bc/adt/packages/sadt_main\"",
                "adtcore:uri=\"https://example.test/package\"",
                1,
            )
            .replacen("adtcore:type=\"PINF/KI\"", "adtcore:type=\"FUTURE/I\"", 1);
        let properties: PackageProperties = serde_xml_rs::from_str(&xml).unwrap();

        assert_eq!(properties.version, "future");
        assert_eq!(
            properties.super_package.unwrap().uri.as_deref(),
            Some("https://example.test/package")
        );
        assert_eq!(
            properties.use_accesses.unwrap().use_access[0]
                .package_interface
                .object_type
                .as_ref()
                .map(GlobalWorkbenchType::as_str),
            Some("FUTURE/I")
        );
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
