use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = AccessControlProperties,
    workbench_type = "DCLS/DL",
    collection(
        scheme = "http://www.sap.com/adt/categories/acm/dclsources",
        term = "dclsources",
    ),
    capabilities(
        Create(AccessControlCreateProperties),
        Source(properties.source_uri),
    )
)]
/// An ABAP Core Data Services Access Control (DCL source).
pub struct AccessControl;

/// The complete Access Control properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = AccessControlCreateProperties,
    doc = "The sparse payload used to create a CDS Access Control."
)]
#[serde(rename = "dcl:dclSource", deny_unknown_fields)]
pub struct AccessControlProperties {
    /// The primary source URI exactly as advertised by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the Access Control.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The Access Control's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The Access Control's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: AbapLanguageVersion,

    /// The Access Control name supplied by ADT.
    #[for_create(identity, default, doc = "The Access Control name.")]
    #[serde(rename = "@adtcore:name")]
    pub name: String,

    /// The repository object type, normally `DCLS/DL`.
    #[for_create(
        identity,
        default = <AccessControl as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The Access Control's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    /// The timestamp at which the object was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub version: WorkbenchVersion,

    /// The timestamp at which the object was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,

    /// The user who last changed the object.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The user who created the object.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,

    /// The Access Control description.
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
    #[for_create(doc = "The package receiving the Access Control.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
}

impl MediaTyped for AccessControlProperties {
    const MEDIA_TYPES: &'static [&'static str] = &["application/vnd.sap.adt.dclSource+xml"];
}

impl ToXml for AccessControlProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("dcl", "http://www.sap.com/adt/acm/dclsources"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const ACCESS_CONTROL_XML: &str =
        include_str!("../../../tests/fixtures/access-control-sdsh-cds-domain-val-dcl.xml");

    fn parse(body: &str) -> Result<AccessControlProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn properties() -> AccessControlProperties {
        parse(ACCESS_CONTROL_XML).unwrap()
    }

    #[test]
    fn builds_the_handler_confirmed_sparse_creation_payload() {
        let properties = AccessControlCreateProperties::builder()
            .description("Created Access Control")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, AccessControl::WORKBENCH_TYPE);
        assert!(properties.abap_language_version.is_none());

        let reference = ObjectRef::<AccessControl>::new(
            "Z_ACCESS_CONTROL".to_owned(),
            AdtUri::parse("/sap/bc/adt/acm/dcl/sources/z_access_control").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<dcl:dclSource"));
        assert!(body.contains("adtcore:name=\"Z_ACCESS_CONTROL\""));
        assert!(body.contains("adtcore:type=\"DCLS/DL\""));
        assert!(body.contains("adtcore:description=\"Created Access Control\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("abapsource:sourceUri="));
        assert!(!body.contains("adtcore:changedAt="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_complete_live_access_control_properties() {
        let properties = properties();

        assert_eq!(properties.name, "SDSH_CDS_DOMAIN_VAL_DCL");
        assert_eq!(properties.object_type, AccessControl::WORKBENCH_TYPE);
        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(properties.source_uri, "source/main");
        assert_eq!(properties.package.name.as_deref(), Some("SDSH"));
        assert_eq!(properties.links.len(), 4);
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "SDSH_CDS_DOMAIN_VAL_DCL");
        assert_eq!(json["@adtcore:type"], "DCLS/DL");
        assert_eq!(json["@abapsource:sourceUri"], "source/main");
        assert_eq!(json["adtcore:packageRef"]["@adtcore:name"], "SDSH");
        assert!(json.get("name").is_none());

        let round_tripped: AccessControlProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties = properties();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<dcl:dclSource"));
        assert!(xml.contains("xmlns:dcl=\"http://www.sap.com/adt/acm/dclsources\""));
        assert!(xml.contains("abapsource:sourceUri=\"source/main\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn rejects_unmodeled_root_fields_before_updates_can_drop_them() {
        let body = ACCESS_CONTROL_XML.replacen(
            " abapsource:sourceUri=",
            " dcl:futureAttribute=\"value\" abapsource:sourceUri=",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
