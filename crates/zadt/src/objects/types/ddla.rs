use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType, MediaTyped, ObjectVersion,
    ToXml,
};

#[object_type(
    properties = AnnotationDefinitionProperties,
    workbench_type = "DDLA/ADF",
    collection(
        scheme = "http://www.sap.com/wbobj/cds",
        term = "ddlaadf",
    ),
    capabilities(
        Create(AnnotationDefinitionCreateProperties),
        Source(properties.source_uri),
    )
)]
/// An ABAP Core Data Services Annotation Definition.
pub struct AnnotationDefinition;

/// The complete Annotation Definition properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = AnnotationDefinitionCreateProperties,
    doc = "The sparse payload used to create a CDS Annotation Definition."
)]
#[serde(rename = "ddla:ddlaSource", deny_unknown_fields)]
pub struct AnnotationDefinitionProperties {
    /// The primary source URI exactly as advertised by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the Annotation Definition.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The Annotation Definition's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The Annotation Definition's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The Annotation Definition name supplied by ADT.
    #[for_create(identity, default, doc = "The Annotation Definition name.")]
    #[serde(rename = "@adtcore:name")]
    pub name: String,

    /// The repository object type, normally `DDLA/ADF`.
    #[for_create(
        identity,
        default = <AnnotationDefinition as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The Annotation Definition's global Workbench type."
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

    /// The Annotation Definition description.
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
    #[for_create(doc = "The package receiving the Annotation Definition.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
}

impl MediaTyped for AnnotationDefinitionProperties {
    const MEDIA_TYPES: &'static [&'static str] = &["application/vnd.sap.adt.ddic.ddla.v1+xml"];
}

impl ToXml for AnnotationDefinitionProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("ddla", "http://www.sap.com/adt/ddic/ddlasources"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const ANNOTATION_DEFINITION_XML: &str =
        include_str!("../../../tests/fixtures/annotation-definition-ui.xml");

    fn parse(body: &str) -> Result<AnnotationDefinitionProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn properties() -> AnnotationDefinitionProperties {
        parse(ANNOTATION_DEFINITION_XML).unwrap()
    }

    #[test]
    fn builds_the_sparse_creation_payload() {
        let properties = AnnotationDefinitionCreateProperties::builder()
            .description("Created Annotation Definition")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, AnnotationDefinition::WORKBENCH_TYPE);

        let reference = ObjectRef::<AnnotationDefinition>::new(
            "Z_ANNOTATION_DEFINITION".to_owned(),
            AdtUri::parse("/sap/bc/adt/ddic/ddla/sources/z_annotation_definition").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<ddla:ddlaSource"));
        assert!(body.contains("adtcore:name=\"Z_ANNOTATION_DEFINITION\""));
        assert!(body.contains("adtcore:type=\"DDLA/ADF\""));
        assert!(body.contains("adtcore:description=\"Created Annotation Definition\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("abapsource:sourceUri="));
        assert!(!body.contains("adtcore:changedAt="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_complete_live_annotation_definition_properties() {
        let properties = properties();

        assert_eq!(properties.name, "UI");
        assert_eq!(properties.object_type, AnnotationDefinition::WORKBENCH_TYPE);
        assert_eq!(properties.version, ObjectVersion::Active);
        assert_eq!(properties.source_uri, "./ui/source/main");
        assert_eq!(
            properties.package.name.as_deref(),
            Some("SADL_GW_EXPOSURE_VOCAN_DEF")
        );
        assert_eq!(properties.links.len(), 4);
    }

    #[test]
    fn relative_source_uri_resolves_against_the_object() {
        let properties = properties();
        let object = crate::Object::new(
            ObjectRef::<AnnotationDefinition>::new(
                properties.name.clone(),
                AdtUri::parse("/sap/bc/adt/ddic/ddla/sources/ui").unwrap(),
            ),
            AnnotationDefinitionProperties::MEDIA_TYPES[0],
            None,
            properties,
        );

        assert_eq!(
            object.source().unwrap().uri.as_str(),
            "/sap/bc/adt/ddic/ddla/sources/ui/source/main"
        );
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "UI");
        assert_eq!(json["@adtcore:type"], "DDLA/ADF");
        assert_eq!(json["@abapsource:sourceUri"], "./ui/source/main");
        assert_eq!(
            json["adtcore:packageRef"]["@adtcore:name"],
            "SADL_GW_EXPOSURE_VOCAN_DEF"
        );
        assert!(json.get("name").is_none());

        let round_tripped: AnnotationDefinitionProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties = properties();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<ddla:ddlaSource"));
        assert!(xml.contains("xmlns:ddla=\"http://www.sap.com/adt/ddic/ddlasources\""));
        assert!(xml.contains("abapsource:sourceUri=\"./ui/source/main\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn rejects_unmodeled_root_fields_before_updates_can_drop_them() {
        let body = ANNOTATION_DEFINITION_XML.replacen(
            " abapsource:sourceUri=",
            " ddla:futureAttribute=\"value\" abapsource:sourceUri=",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
