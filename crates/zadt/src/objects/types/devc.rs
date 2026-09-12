use serde::{Deserialize, Serialize};
use zadt_macros::object_type;

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, AdvertisedSwitchReference,
    GlobalWorkbenchType, MediaTypes, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = PackageProperties,
    media_types = MediaTypes::new(&[
        "application/vnd.sap.adt.packages.v2+xml",
        "application/vnd.sap.adt.packages.v1+xml",
    ]),
    workbench_type = "DEVC/K",
    collection(scheme = "http://www.sap.com/wbobj/packages", term = "devck",),
    capabilities()
)]
/// The package (devclass) object type.
pub struct Package;

/// The currently modeled package-properties payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "pak:package", deny_unknown_fields)]
pub struct PackageProperties {
    /// The package name supplied by SAP.
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,
    /// The repository object type, normally `DEVC/K`.
    #[serde(rename = "@adtcore:type")]
    pub(crate) workbench_type: GlobalWorkbenchType,
    /// The timestamp at which the package was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,
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
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
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
    /// The self package reference supplied by the shared main-object serializer.
    #[serde(rename = "adtcore:packageRef", skip_serializing_if = "Option::is_none")]
    pub package: Option<AdvertisedObjectReference>,

    /// Package behavior and editor capability flags.
    #[serde(rename = "pak:attributes")]
    pub attributes: PackageAttributes,
    /// The parent package, when this is not a root package.
    #[serde(rename = "pak:superPackage")]
    pub super_package: Option<AdvertisedObjectReference>,
    #[serde(rename = "pak:extensionAlias", skip_serializing_if = "Option::is_none")]
    pub extension_alias: Option<PackageExtensionAlias>,

    #[serde(rename = "pak:switch", skip_serializing_if = "Option::is_none")]
    pub switch: Option<AdvertisedSwitchReference>,

    /// The assigned application component.
    #[serde(rename = "pak:applicationComponent")]
    pub application_component: PackageAssignment,
    /// Software-component and transport-layer assignments.
    #[serde(rename = "pak:transport")]
    pub transport: PackageTransport,
    #[serde(rename = "pak:translation", skip_serializing_if = "Option::is_none")]
    pub translation: Option<PackageTranslation>,

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

impl ToXml for PackageProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("pak", "http://www.sap.com/adt/packages"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

/// Package behavior and editor capability flags.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageAttributes {
    /// The semantic package type, such as `development`.
    #[serde(rename = "@pak:packageType")]
    pub package_type: String,
    /// Whether the package type is editable.
    #[serde(rename = "@pak:isPackageTypeEditable")]
    pub package_type_editable: bool,
    /// Whether assigning repository objects to the package is prohibited.
    /// The ADT attribute is misleadingly named: SPAK_ST_PACKAGES serializes
    /// IS_ADDING_OBJECTS_NOT_ALLOWED without negating it.
    #[serde(rename = "@pak:isAddingObjectsAllowed")]
    pub adding_objects_not_allowed: bool,
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
    #[serde(rename = "@pak:languageVersion", default = "empty_language_version")]
    pub language_version: AbapLanguageVersion,
    /// Whether the language version is shown by the package editor.
    #[serde(rename = "@pak:isLanguageVersionVisible")]
    pub language_version_visible: bool,
    /// Whether the language version is editable.
    #[serde(rename = "@pak:isLanguageVersionEditable")]
    pub language_version_editable: bool,
}

fn empty_language_version() -> AbapLanguageVersion {
    AbapLanguageVersion::Other(String::new())
}

/// A named package assignment with editor visibility and mutability flags.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageAssignment {
    /// Software-component type when this assignment describes a software component.
    #[serde(rename = "@pak:type", skip_serializing_if = "Option::is_none")]
    pub assignment_type: Option<String>,

    #[serde(
        rename = "@pak:typeDescription",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_description: Option<String>,

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

/// Package extension alias and editor capabilities.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageExtensionAlias {
    #[serde(rename = "@pak:name")]
    pub name: String,

    #[serde(rename = "@pak:isVisible")]
    pub visible: bool,

    #[serde(rename = "@pak:isEditable")]
    pub editable: bool,
}

/// Translation settings advertised for a package.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageTranslation {
    #[serde(rename = "@pak:relevance")]
    pub relevance: String,

    #[serde(rename = "@pak:relevanceDescription")]
    pub relevance_description: String,

    #[serde(rename = "@pak:isVisible")]
    pub visible: bool,
}

/// Software-component and transport-layer assignments.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageTransport {
    /// The package's software component.
    #[serde(rename = "pak:softwareComponent")]
    pub software_component: PackageAssignment,
    /// The package's transport layer.
    #[serde(rename = "pak:transportLayer")]
    pub transport_layer: PackageAssignment,
}

/// Use-access visibility and entries in a package-properties payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageUseAccesses {
    #[serde(rename = "@pak:isVisible", default)]
    pub visible: bool,
    #[serde(rename = "pak:useAccess", default)]
    pub use_access: Vec<PackageUseAccess>,
}

/// A package-interface use access exactly as represented in package XML.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageUseAccess {
    #[serde(rename = "@pak:severity")]
    pub severity: String,
    #[serde(rename = "pak:packageInterfaceRef")]
    pub package_interface: AdvertisedObjectReference,
    #[serde(rename = "pak:packageRef")]
    pub package_ref: Option<AdvertisedObjectReference>,
}

/// Package-interface visibility and references in a package-properties payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageInterfaces {
    #[serde(rename = "@pak:isVisible", default)]
    pub visible: bool,
    #[serde(rename = "pak:packageInterfaceRef", default)]
    pub package_interface_ref: Vec<AdvertisedObjectReference>,
}

/// Direct subpackage references in a package-properties payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
    fn retains_switch_translation_alias_and_software_metadata() {
        let original: PackageProperties = serde_xml_rs::from_reader(PACKAGE_XML).unwrap();
        let mut wire = serde_json::to_value(original).unwrap();
        wire["adtcore:packageRef"] = serde_json::json!({"@adtcore:name":"SADT_TOOLS_CORE"});
        wire["pak:switch"] = serde_json::json!({"@adtcore:name":"DTINF_FW", "@adtcore:type":"SFSW/6S", "@adtcore:state":"off"});
        wire["pak:extensionAlias"] = serde_json::json!({"@pak:name":"Z_ALIAS", "@pak:isVisible":true, "@pak:isEditable":false});
        wire["pak:translation"] = serde_json::json!({"@pak:relevance":"TRANSL_NONE", "@pak:relevanceDescription":"No translation", "@pak:isVisible":true});
        wire["pak:transport"]["pak:softwareComponent"]["@pak:type"] = serde_json::json!("S");
        wire["pak:transport"]["pak:softwareComponent"]["@pak:typeDescription"] =
            serde_json::json!("Netweaver Basis Component");
        let properties: PackageProperties = serde_json::from_value(wire.clone()).unwrap();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();
        let loaded: PackageProperties = serde_xml_rs::from_str(&xml).unwrap();
        assert_eq!(serde_json::to_value(loaded).unwrap(), wire);
    }

    #[test]
    fn complete_wire_payload_has_canonical_wire_round_trip_json() {
        let properties: PackageProperties = serde_xml_rs::from_reader(PACKAGE_XML).unwrap();

        assert_eq!(properties.name, "SADT_TOOLS_CORE");
        assert_eq!(properties.workbench_type, Package::WORKBENCH_TYPE);
        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(properties.attributes.language_version.as_str(), "");
        assert_eq!(properties.links.len(), 1);
        assert_eq!(properties.links[0].href, "versions");
        assert_eq!(
            properties.super_package.as_ref().unwrap().uri.as_deref(),
            Some("/sap/bc/adt/packages/sadt_main")
        );
        assert_eq!(
            properties.use_accesses.as_ref().unwrap().use_access[0]
                .package_interface
                .workbench_type
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
    fn rejects_unknown_xml_fields_in_root_and_nested_models() {
        let original = std::str::from_utf8(PACKAGE_XML).unwrap();
        for element in [
            "pak:package",
            "atom:link",
            "pak:attributes",
            "pak:superPackage",
            "pak:applicationComponent",
            "pak:transport",
            "pak:softwareComponent",
            "pak:useAccesses",
            "pak:useAccess",
            "pak:packageInterfaceRef",
            "pak:packageInterfaces",
            "pak:subPackages",
            "pak:packageRef",
        ] {
            let marker = format!("<{element}");
            let (offset, _) = original
                .match_indices(&marker)
                .find(|(offset, _)| {
                    let next = original.as_bytes()[offset + marker.len()];
                    next.is_ascii_whitespace() || next == b'>' || next == b'/'
                })
                .unwrap();
            let mut xml = original.to_owned();
            xml.insert_str(offset + marker.len(), " unexpected=\"value\"");
            let error = serde_xml_rs::from_str::<PackageProperties>(&xml).unwrap_err();
            assert!(
                error.to_string().contains("unknown field `@unexpected`"),
                "{element}: {error}"
            );
        }
        for element in [
            "pak:package",
            "pak:transport",
            "pak:useAccesses",
            "pak:useAccess",
            "pak:packageInterfaces",
            "pak:subPackages",
        ] {
            let marker = format!("</{element}>");
            let xml = original.replacen(&marker, &format!("<unexpected/>{marker}"), 1);
            assert_ne!(xml, original, "{element}");
            let error = serde_xml_rs::from_str::<PackageProperties>(&xml).unwrap_err();
            assert!(
                error.to_string().contains("unknown field `unexpected`"),
                "{element}: {error}"
            );
        }
    }

    #[test]
    fn nested_wire_values_are_not_validated_or_resolved() {
        let xml = String::from_utf8(PACKAGE_XML.to_vec())
            .unwrap()
            .replacen(
                "adtcore:uri=\"/sap/bc/adt/packages/sadt_main\"",
                "adtcore:uri=\"https://example.test/package\"",
                1,
            )
            .replacen("adtcore:type=\"PINF/KI\"", "adtcore:type=\"FUTURE/I\"", 1);
        let properties: PackageProperties = serde_xml_rs::from_str(&xml).unwrap();

        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(
            properties.super_package.unwrap().uri.as_deref(),
            Some("https://example.test/package")
        );
        assert_eq!(
            properties.use_accesses.unwrap().use_access[0]
                .package_interface
                .workbench_type
                .as_ref()
                .map(GlobalWorkbenchType::as_str),
            Some("FUTURE/I")
        );
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties: PackageProperties = serde_xml_rs::from_reader(PACKAGE_XML).unwrap();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<pak:package"));
        assert!(xml.contains("xmlns:pak=\"http://www.sap.com/adt/packages\""));
        assert!(xml.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
        assert!(xml.contains("xmlns:atom=\"http://www.w3.org/2005/Atom\""));
        assert!(xml.contains("<pak:attributes"));
        assert!(xml.contains("<atom:link"));
        assert!(xml.contains("adtcore:version=\"active\""));
        let roundtrip: PackageProperties = serde_xml_rs::from_str(&xml).unwrap();
        assert_eq!(roundtrip.name, properties.name);
        assert_eq!(roundtrip.description, properties.description);
    }

    #[test]
    fn rejects_unknown_object_versions() {
        let xml = String::from_utf8(PACKAGE_XML.to_vec()).unwrap().replacen(
            "adtcore:version=\"active\"",
            "adtcore:version=\"future\"",
            1,
        );

        assert!(serde_xml_rs::from_str::<PackageProperties>(&xml).is_err());
    }
}
