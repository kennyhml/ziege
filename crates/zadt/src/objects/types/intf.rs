use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, Source, SyntaxConfiguration, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = InterfaceProperties,
    workbench_type = "INTF/OI",
    collection(
        scheme = "http://www.sap.com/adt/categories/oo",
        term = "interfaces",
    ),
    capabilities(
        Create(InterfaceCreateProperties),
        Source,
        Structure,
    )
)]
/// A global ABAP interface.
pub struct Interface;

/// The complete interface properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = InterfaceCreateProperties,
    doc = "The sparse payload used to create an ABAP interface."
)]
#[serde(rename = "intf:abapInterface", deny_unknown_fields)]
pub struct InterfaceProperties {
    /// Whether this interface is maintained through a higher-level model.
    #[serde(rename = "@abapoo:modeled")]
    pub modeled: bool,

    /// The primary source URI exactly as advertised by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the interface.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The interface's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The interface's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version when supplied by the media version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: Option<AbapLanguageVersion>,

    /// The interface name supplied by ADT.
    #[for_create(identity, default, doc = "The interface name.")]
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,

    /// The repository object type, normally `INTF/OI`.
    #[for_create(
        identity,
        default = <Interface as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The interface's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,

    /// The timestamp at which the interface was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,

    /// The timestamp at which the interface was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,

    /// The user who last changed the interface.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The user who created the interface.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,

    /// The interface description.
    #[for_create(doc = "The description, limited by SAP to 60 characters.")]
    #[serde(rename = "@adtcore:description")]
    pub description: String,

    /// The maximum interface-description length.
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,

    /// The interface's logon language.
    #[serde(rename = "@adtcore:language")]
    pub language: String,

    /// Atom links advertised for the interface, in document order.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,

    /// The package reference advertised for the interface.
    #[for_create(doc = "The package receiving the interface.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,

    /// The source syntax configuration embedded in the payload.
    #[serde(rename = "abapsource:syntaxConfiguration")]
    pub syntax_configuration: SyntaxConfiguration,
}

impl MediaTyped for InterfaceProperties {
    const MEDIA_TYPES: &'static [&'static str] = &[
        "application/vnd.sap.adt.oo.interfaces.v5+xml",
        "application/vnd.sap.adt.oo.interfaces.v4+xml",
    ];
}

impl ToXml for InterfaceProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("intf", "http://www.sap.com/adt/oo/interfaces"),
        ("abapoo", "http://www.sap.com/adt/oo"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

impl Source for Interface {
    fn source_uri(properties: &Self::Properties) -> Option<&str> {
        (!properties.modeled).then_some(properties.source_uri.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const INTERFACE_XML: &str =
        include_str!("../../../tests/fixtures/interface-if-adt-uri-mapper-v5.xml");

    fn parse(body: &str) -> Result<InterfaceProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn properties() -> InterfaceProperties {
        parse(INTERFACE_XML).unwrap()
    }

    #[test]
    fn builds_the_handler_confirmed_sparse_creation_payload() {
        let properties = InterfaceCreateProperties::builder()
            .description("Created Interface")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, Interface::WORKBENCH_TYPE);
        assert!(properties.abap_language_version.is_none());

        let reference = ObjectRef::<Interface>::new(
            "ZIF_EXAMPLE".to_owned(),
            AdtUri::parse("/sap/bc/adt/oo/interfaces/zif_example").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<intf:abapInterface"));
        assert!(body.contains("adtcore:name=\"ZIF_EXAMPLE\""));
        assert!(body.contains("adtcore:type=\"INTF/OI\""));
        assert!(body.contains("adtcore:description=\"Created Interface\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("abapoo:modeled="));
        assert!(!body.contains("abapsource:sourceUri="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_complete_live_v5_interface_properties() {
        let properties = properties();

        assert_eq!(properties.name, "IF_ADT_URI_MAPPER");
        assert_eq!(properties.object_type, Interface::WORKBENCH_TYPE);
        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(properties.source_uri, "source/main");
        assert_eq!(
            properties
                .abap_language_version
                .as_ref()
                .map(AbapLanguageVersion::as_str),
            Some("X")
        );
        assert_eq!(properties.package.name.as_deref(), Some("SADT_TOOLS_CORE"));
        assert_eq!(properties.links.len(), 6);
    }

    #[test]
    fn parses_v4_without_the_direct_language_version() {
        let body = INTERFACE_XML.replace(" adtcore:abapLanguageVersion=\"X\"", "");
        let properties = parse(&body).unwrap();

        assert_eq!(properties.name, "IF_ADT_URI_MAPPER");
        assert!(properties.abap_language_version.is_none());
    }

    #[test]
    fn modeled_interfaces_do_not_advertise_an_editable_source() {
        let body = INTERFACE_XML.replace("abapoo:modeled=\"false\"", "abapoo:modeled=\"true\"");
        let properties = parse(&body).unwrap();

        assert!(<Interface as Source>::source_uri(&properties).is_none());
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "IF_ADT_URI_MAPPER");
        assert_eq!(json["@adtcore:type"], "INTF/OI");
        assert_eq!(json["@abapsource:sourceUri"], "source/main");
        assert_eq!(
            json["adtcore:packageRef"]["@adtcore:name"],
            "SADT_TOOLS_CORE"
        );
        assert!(json.get("name").is_none());

        let round_tripped: InterfaceProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties = properties();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<intf:abapInterface"));
        assert!(xml.contains("xmlns:intf=\"http://www.sap.com/adt/oo/interfaces\""));
        assert!(xml.contains("abapsource:sourceUri=\"source/main\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<abapsource:syntaxConfiguration"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn rejects_unmodeled_root_fields_before_updates_can_drop_them() {
        let body = INTERFACE_XML.replacen(
            " abapoo:modeled=",
            " intf:futureAttribute=\"value\" abapoo:modeled=",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
