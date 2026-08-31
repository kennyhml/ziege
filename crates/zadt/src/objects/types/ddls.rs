use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = DataDefinitionProperties,
    workbench_type = "DDLS/DF",
    collection(
        scheme = "http://www.sap.com/adt/categories/ddic/ddlsources",
        term = "ddlsources",
    ),
    capabilities(
        Create(DataDefinitionCreateProperties),
        Source(properties.source_uri),
    )
)]
/// An ABAP Core Data Services Data Definition.
pub struct DataDefinition;

/// The complete Data Definition properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = DataDefinitionCreateProperties,
    doc = "The sparse payload used to create a CDS Data Definition."
)]
#[serde(rename = "ddl:ddlSource", deny_unknown_fields)]
pub struct DataDefinitionProperties {
    /// The origin code assigned to the DDL source by SAP.
    #[serde(rename = "@ddl:source_origin")]
    pub source_origin: String,

    /// The semantic DDL source type, such as `view`.
    #[serde(rename = "@ddl:source_type")]
    pub source_type: Option<String>,

    /// The server-provided description of the semantic source type.
    #[serde(rename = "@ddl:source_type_description")]
    pub source_type_description: Option<String>,

    /// The server-provided description of the source origin.
    #[serde(rename = "@ddl:source_origin_description")]
    pub source_origin_description: String,

    /// The primary source URI exactly as advertised by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the Data Definition.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The Data Definition's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The Data Definition's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: AbapLanguageVersion,

    /// The Data Definition name supplied by ADT.
    #[for_create(identity, default, doc = "The Data Definition name.")]
    #[serde(rename = "@adtcore:name")]
    pub name: String,

    /// The repository object type, normally `DDLS/DF`.
    #[for_create(
        identity,
        default = <DataDefinition as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The Data Definition's global Workbench type."
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

    /// The Data Definition description.
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
    #[for_create(doc = "The package receiving the Data Definition.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
}

impl MediaTyped for DataDefinitionProperties {
    const MEDIA_TYPES: &'static [&'static str] = &["application/vnd.sap.adt.ddlSource+xml"];
}

impl ToXml for DataDefinitionProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("ddl", "http://www.sap.com/adt/ddic/ddlsources"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const DATA_DEFINITION_XML: &str =
        include_str!("../../../tests/fixtures/data-definition-i-businesspartner.xml");

    fn parse(body: &str) -> Result<DataDefinitionProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn properties() -> DataDefinitionProperties {
        parse(DATA_DEFINITION_XML).unwrap()
    }

    #[test]
    fn builds_the_handler_confirmed_sparse_creation_payload() {
        let properties = DataDefinitionCreateProperties::builder()
            .description("Created Data Definition")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, DataDefinition::WORKBENCH_TYPE);
        assert!(properties.abap_language_version.is_none());

        let reference = ObjectRef::<DataDefinition>::new(
            "Z_DATA_DEFINITION".to_owned(),
            AdtUri::parse("/sap/bc/adt/ddic/ddl/sources/z_data_definition").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<ddl:ddlSource"));
        assert!(body.contains("adtcore:name=\"Z_DATA_DEFINITION\""));
        assert!(body.contains("adtcore:type=\"DDLS/DF\""));
        assert!(body.contains("adtcore:description=\"Created Data Definition\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("ddl:source_origin="));
        assert!(!body.contains("ddl:source_type="));
        assert!(!body.contains("abapsource:sourceUri="));
        assert!(!body.contains("adtcore:changedAt="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_complete_live_data_definition_properties() {
        let properties = properties();

        assert_eq!(properties.name, "I_BUSINESSPARTNER");
        assert_eq!(properties.object_type, DataDefinition::WORKBENCH_TYPE);
        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(properties.source_type.as_deref(), Some("view"));
        assert_eq!(
            properties.source_type_description.as_deref(),
            Some("View Entity")
        );
        assert_eq!(properties.source_uri, "source/main");
        assert_eq!(properties.package.name.as_deref(), Some("VDM_MD_BP_BASE"));
        assert_eq!(properties.links.len(), 6);
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "I_BUSINESSPARTNER");
        assert_eq!(json["@adtcore:type"], "DDLS/DF");
        assert_eq!(json["@ddl:source_type"], "view");
        assert_eq!(json["@abapsource:sourceUri"], "source/main");
        assert_eq!(
            json["adtcore:packageRef"]["@adtcore:name"],
            "VDM_MD_BP_BASE"
        );
        assert!(json.get("name").is_none());

        let round_tripped: DataDefinitionProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let properties = properties();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<ddl:ddlSource"));
        assert!(xml.contains("xmlns:ddl=\"http://www.sap.com/adt/ddic/ddlsources\""));
        assert!(xml.contains("ddl:source_type=\"view\""));
        assert!(xml.contains("abapsource:sourceUri=\"source/main\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn new_definitions_may_omit_source_classification() {
        let body = DATA_DEFINITION_XML
            .replace(" ddl:source_type=\"view\"", "")
            .replace(" ddl:source_type_description=\"View Entity\"", "");
        let properties = parse(&body).unwrap();

        assert!(properties.source_type.is_none());
        assert!(properties.source_type_description.is_none());
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();
        assert!(!xml.contains("ddl:source_type="));
        assert!(!xml.contains("ddl:source_type_description="));
    }

    #[test]
    fn rejects_unmodeled_root_fields_before_updates_can_drop_them() {
        let body = DATA_DEFINITION_XML.replacen(
            " ddl:source_origin=",
            " ddl:future_attribute=\"value\" ddl:source_origin=",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
