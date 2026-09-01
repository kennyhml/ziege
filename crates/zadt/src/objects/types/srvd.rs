use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = ServiceDefinitionProperties,
    workbench_type = "SRVD/SRV",
    collection(
        scheme = "http://www.sap.com/wbobj/raps",
        term = "srvdsrv",
    ),
    capabilities(
        Create(ServiceDefinitionCreateProperties),
        Source(properties.source_uri),
    )
)]
/// An ABAP Core Data Services Service Definition.
pub struct ServiceDefinition;

/// The complete Service Definition properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = ServiceDefinitionCreateProperties,
    doc = "The sparse payload used to create a CDS Service Definition."
)]
#[serde(rename = "srvd:srvdSource", deny_unknown_fields)]
pub struct ServiceDefinitionProperties {
    /// The source-origin code assigned by SAP.
    #[serde(rename = "@srvd:sourceOrigin")]
    pub source_origin: String,

    /// The server-provided description of the source origin.
    #[serde(rename = "@srvd:sourceOriginDescription")]
    pub source_origin_description: String,

    /// The semantic Service Definition source type.
    #[for_create(
        default = String::from("S"),
        doc = "The Service Definition source-type discriminator."
    )]
    #[serde(rename = "@srvd:srvdSourceType")]
    pub source_type: String,

    /// The server-provided description of the source type.
    #[serde(rename = "@srvd:srvdSourceTypeDescription")]
    pub source_type_description: String,

    /// The primary source URI exactly as advertised by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the Service Definition.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The Service Definition's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The Service Definition's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: AbapLanguageVersion,

    /// The Service Definition name supplied by ADT.
    #[for_create(identity, default, doc = "The Service Definition name.")]
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,

    /// The repository object type, normally `SRVD/SRV`.
    #[for_create(
        identity,
        default = <ServiceDefinition as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The Service Definition's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,

    /// The timestamp at which the object was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,

    /// The timestamp at which the object was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,

    /// The user who last changed the object.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The user who created the object.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,

    /// The Service Definition description.
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
    #[for_create(doc = "The package receiving the Service Definition.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
}

impl MediaTyped for ServiceDefinitionProperties {
    const MEDIA_TYPES: &'static [&'static str] = &["application/vnd.sap.adt.ddic.srvd.v1+xml"];
}

impl ToXml for ServiceDefinitionProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("srvd", "http://www.sap.com/adt/ddic/srvdsources"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const SERVICE_DEFINITION_XML: &str =
        include_str!("../../../tests/fixtures/service-definition-managedistributions.xml");

    fn parse(body: &str) -> Result<ServiceDefinitionProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn properties() -> ServiceDefinitionProperties {
        parse(SERVICE_DEFINITION_XML).unwrap()
    }

    #[test]
    fn builds_the_sparse_creation_payload() {
        let properties = ServiceDefinitionCreateProperties::builder()
            .description("Created Service Definition")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, ServiceDefinition::WORKBENCH_TYPE);
        assert_eq!(properties.source_type, "S");
        assert!(properties.abap_language_version.is_none());

        let reference = ObjectRef::<ServiceDefinition>::new(
            "Z_SERVICE_DEFINITION".to_owned(),
            AdtUri::parse("/sap/bc/adt/ddic/srvd/sources/z_service_definition").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<srvd:srvdSource"));
        assert!(body.contains("adtcore:name=\"Z_SERVICE_DEFINITION\""));
        assert!(body.contains("adtcore:type=\"SRVD/SRV\""));
        assert!(body.contains("adtcore:description=\"Created Service Definition\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("srvd:sourceOrigin="));
        assert!(body.contains("srvd:srvdSourceType=\"S\""));
        assert!(!body.contains("abapsource:sourceUri="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_complete_live_service_definition_properties() {
        let properties = properties();

        assert_eq!(properties.name, "MANAGEDISTRIBUTIONS");
        assert_eq!(properties.object_type, ServiceDefinition::WORKBENCH_TYPE);
        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(properties.source_type, "S");
        assert_eq!(properties.source_type_description, "Definition");
        assert_eq!(properties.source_uri, "./managedistributions/source/main");
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
            ObjectRef::<ServiceDefinition>::new(
                properties.name.clone(),
                AdtUri::parse("/sap/bc/adt/ddic/srvd/sources/managedistributions").unwrap(),
            ),
            crate::WorkbenchVersion::Active,
            ServiceDefinitionProperties::MEDIA_TYPES[0],
            None,
            properties,
        );

        assert_eq!(
            object.source().unwrap().uri.as_str(),
            "/sap/bc/adt/ddic/srvd/sources/managedistributions/source/main"
        );
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "MANAGEDISTRIBUTIONS");
        assert_eq!(json["@adtcore:type"], "SRVD/SRV");
        assert_eq!(json["@srvd:srvdSourceType"], "S");
        assert_eq!(
            json["@abapsource:sourceUri"],
            "./managedistributions/source/main"
        );
        assert!(json.get("name").is_none());

        let round_tripped: ServiceDefinitionProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties = properties();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<srvd:srvdSource"));
        assert!(xml.contains("xmlns:srvd=\"http://www.sap.com/adt/ddic/srvdsources\""));
        assert!(xml.contains("srvd:srvdSourceType=\"S\""));
        assert!(xml.contains("abapsource:sourceUri=\"./managedistributions/source/main\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn rejects_unmodeled_root_fields_before_updates_can_drop_them() {
        let body = SERVICE_DEFINITION_XML.replacen(
            " srvd:sourceOrigin=",
            " srvd:futureAttribute=\"value\" srvd:sourceOrigin=",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
