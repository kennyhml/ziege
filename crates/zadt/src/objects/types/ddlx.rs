use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, ObjectVersion, ToXml,
};

#[object_type(
    properties = MetadataExtensionProperties,
    workbench_type = "DDLX/EX",
    collection(
        scheme = "http://www.sap.com/wbobj/cds",
        term = "ddlxex",
    ),
    capabilities(
        Create(MetadataExtensionCreateProperties),
        Source(properties.source_uri),
    )
)]
/// An ABAP Core Data Services Metadata Extension.
pub struct MetadataExtension;

/// The complete Metadata Extension properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = MetadataExtensionCreateProperties,
    doc = "The sparse payload used to create a CDS Metadata Extension."
)]
#[serde(rename = "ddlx:ddlxSource", deny_unknown_fields)]
pub struct MetadataExtensionProperties {
    /// The primary source URI exactly as advertised by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the Metadata Extension.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The Metadata Extension's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The Metadata Extension's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: AbapLanguageVersion,

    /// The Metadata Extension name supplied by ADT.
    #[for_create(identity, default, doc = "The Metadata Extension name.")]
    #[serde(rename = "@adtcore:name")]
    pub name: String,

    /// The repository object type, normally `DDLX/EX`.
    #[for_create(
        identity,
        default = <MetadataExtension as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The Metadata Extension's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    /// The timestamp at which the object was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub version: ObjectVersion,

    /// The timestamp at which the object was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,

    /// The user who last changed the object.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The user who created the object.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,

    /// The Metadata Extension description.
    #[for_create(doc = "The description, limited by SAP to 60 characters.")]
    #[serde(rename = "@adtcore:description")]
    pub description: String,

    /// The language in which language-dependent values are represented.
    #[serde(rename = "@adtcore:language")]
    pub language: String,

    /// Atom links exactly as advertised at the payload root.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,

    /// The package reference exactly as embedded in the payload.
    #[for_create(doc = "The package receiving the Metadata Extension.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
}

impl MediaTyped for MetadataExtensionProperties {
    const MEDIA_TYPES: &'static [&'static str] = &["application/vnd.sap.adt.ddic.ddlx.v1+xml"];
}

impl ToXml for MetadataExtensionProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("ddlx", "http://www.sap.com/adt/ddic/ddlxsources"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const METADATA_EXTENSION_XML: &str =
        include_str!("../../../tests/fixtures/metadata-extension-c-mdoapplicationscope.xml");

    fn parse(body: &str) -> Result<MetadataExtensionProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn properties() -> MetadataExtensionProperties {
        parse(METADATA_EXTENSION_XML).unwrap()
    }

    #[test]
    fn builds_a_sparse_payload_for_the_eclipse_confirmed_creation_collection() {
        let properties = MetadataExtensionCreateProperties::builder()
            .description("Created Metadata Extension")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, MetadataExtension::WORKBENCH_TYPE);
        assert!(properties.abap_language_version.is_none());

        let reference = ObjectRef::<MetadataExtension>::new(
            "Z_METADATA_EXTENSION".to_owned(),
            AdtUri::parse("/sap/bc/adt/ddic/ddlx/sources/z_metadata_extension").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<ddlx:ddlxSource"));
        assert!(body.contains("adtcore:name=\"Z_METADATA_EXTENSION\""));
        assert!(body.contains("adtcore:type=\"DDLX/EX\""));
        assert!(body.contains("adtcore:description=\"Created Metadata Extension\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("abapsource:sourceUri="));
        assert!(!body.contains("adtcore:changedAt="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_complete_live_metadata_extension_properties() {
        let properties = properties();

        assert_eq!(properties.name, "C_MDOAPPLICATIONSCOPE");
        assert_eq!(properties.object_type, MetadataExtension::WORKBENCH_TYPE);
        assert_eq!(properties.version, ObjectVersion::Active);
        assert_eq!(properties.source_uri, "./c_mdoapplicationscope/source/main");
        assert_eq!(
            properties.package.name.as_deref(),
            Some("MDO_DISTRIBUTION_MODEL")
        );
        assert_eq!(properties.links.len(), 4);
    }

    #[test]
    fn relative_source_uri_resolves_against_the_object() {
        let properties = properties();
        let object = crate::ObjectSnapshot::new(
            ObjectRef::<MetadataExtension>::new(
                properties.name.clone(),
                AdtUri::parse("/sap/bc/adt/ddic/ddlx/sources/c_mdoapplicationscope").unwrap(),
            ),
            MetadataExtensionProperties::MEDIA_TYPES[0],
            None,
            properties,
        );

        assert_eq!(
            object.source().unwrap().uri.as_str(),
            "/sap/bc/adt/ddic/ddlx/sources/c_mdoapplicationscope/source/main"
        );
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "C_MDOAPPLICATIONSCOPE");
        assert_eq!(json["@adtcore:type"], "DDLX/EX");
        assert_eq!(
            json["@abapsource:sourceUri"],
            "./c_mdoapplicationscope/source/main"
        );
        assert_eq!(
            json["adtcore:packageRef"]["@adtcore:name"],
            "MDO_DISTRIBUTION_MODEL"
        );
        assert!(json.get("name").is_none());

        let round_tripped: MetadataExtensionProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties = properties();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<ddlx:ddlxSource"));
        assert!(xml.contains("xmlns:ddlx=\"http://www.sap.com/adt/ddic/ddlxsources\""));
        assert!(xml.contains("abapsource:sourceUri=\"./c_mdoapplicationscope/source/main\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn rejects_unmodeled_root_fields_before_updates_can_drop_them() {
        let body = METADATA_EXTENSION_XML.replacen(
            " abapsource:sourceUri=",
            " ddlx:futureAttribute=\"value\" abapsource:sourceUri=",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
