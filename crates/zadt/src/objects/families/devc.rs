use serde::{Deserialize, Serialize};
use zadt_macros::object_type;

use crate::{AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType, PropertyModel};

#[object_type(
    workbench_type = "DEVC/K",
    collection(scheme = "http://www.sap.com/wbobj/packages", term = "devck",),
    capabilities()
)]
/// The package (devclass) object type.
pub type Package = PackageProperties;

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

    fn object_name(&self) -> &str {
        &self.name
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
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
    use crate::ObjectType;

    const PACKAGE_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/package-sadt-tools-core.xml");

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
