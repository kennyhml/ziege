use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = DomainProperties,
    workbench_type = "DOMA/DD",
    collection(
        scheme = "http://www.sap.com/wbobj/dictionary",
        term = "domadd",
    ),
    capabilities(Create(DomainCreateProperties))
)]
/// An ABAP Dictionary Domain.
pub struct Domain;

/// The complete Domain properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = DomainCreateProperties,
    doc = "The sparse payload used to create an ABAP Dictionary Domain."
)]
#[serde(rename = "doma:domain", deny_unknown_fields)]
pub struct DomainProperties {
    /// The user responsible for the Domain.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The Domain's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The Domain's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: AbapLanguageVersion,

    /// The Domain name supplied by ADT.
    #[for_create(identity, default, doc = "The Domain name.")]
    #[serde(rename = "@adtcore:name")]
    pub name: String,

    /// The repository object type, normally `DOMA/DD`.
    #[for_create(
        identity,
        default = <Domain as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The Domain's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    /// The timestamp at which the object was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub version: WorkbenchVersion,

    /// The timestamp at which the object was created, when advertised.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: Option<String>,

    /// The user who last changed the object.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The user who created the object.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,

    /// The Domain description.
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
    #[for_create(doc = "The package receiving the Domain.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,

    /// The Domain's type, output, and value information.
    #[serde(rename = "doma:content")]
    pub content: DomainContent,
}

impl MediaTyped for DomainProperties {
    const MEDIA_TYPES: &'static [&'static str] = &["application/vnd.sap.adt.domains.v2+xml"];
}

impl ToXml for DomainProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("doma", "http://www.sap.com/dictionary/domain"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

/// The nested Domain definition exactly as represented by ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainContent {
    /// The Domain's storage type.
    #[serde(rename = "doma:typeInformation")]
    pub type_information: DomainTypeInformation,

    /// The Domain's display behavior.
    #[serde(rename = "doma:outputInformation")]
    pub output_information: DomainOutputInformation,

    /// The Domain's value table and fixed values, when advertised.
    #[serde(rename = "doma:valueInformation")]
    pub value_information: Option<DomainValueInformation>,
}

/// The storage type and dimensions of a Domain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainTypeInformation {
    /// The Dictionary datatype code.
    #[serde(rename = "doma:datatype")]
    pub datatype: String,

    /// The zero-padded wire length.
    #[serde(rename = "doma:length")]
    pub length: String,

    /// The zero-padded wire decimal count.
    #[serde(rename = "doma:decimals")]
    pub decimals: String,
}

/// The display behavior of a Domain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainOutputInformation {
    /// The zero-padded output length.
    #[serde(rename = "doma:length")]
    pub length: String,

    /// The output style code.
    #[serde(rename = "doma:style")]
    pub style: String,

    /// The conversion-exit name, or an empty string when absent.
    #[serde(rename = "doma:conversionExit")]
    pub conversion_exit: String,

    /// Whether the output supports a sign.
    #[serde(rename = "doma:signExists")]
    pub sign_exists: bool,

    /// Whether lowercase values are accepted.
    #[serde(rename = "doma:lowercase")]
    pub lowercase: bool,

    /// Whether time values use an AM/PM format.
    #[serde(rename = "doma:ampmFormat")]
    pub ampm_format: bool,
}

/// The value table and fixed-value definition of a Domain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainValueInformation {
    /// The value table reference, including an empty reference when advertised that way.
    #[serde(rename = "doma:valueTableRef")]
    pub value_table: AdvertisedObjectReference,

    /// Whether fixed values are extended by an append.
    #[serde(rename = "doma:appendExists")]
    pub append_exists: bool,

    /// The ordered fixed values.
    #[serde(rename = "doma:fixValues")]
    pub fixed_values: DomainFixedValues,
}

/// The ordered fixed-value collection exactly as represented by ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainFixedValues {
    /// The fixed values in server order.
    #[serde(rename = "doma:fixValue", default)]
    pub values: Vec<DomainFixedValue>,
}

/// A single Domain fixed value or value range.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DomainFixedValue {
    /// The zero-padded wire position.
    #[serde(rename = "doma:position")]
    pub position: String,

    /// The low value.
    #[serde(rename = "doma:low")]
    pub low: String,

    /// The high value, or an empty string for a single value.
    #[serde(rename = "doma:high")]
    pub high: String,

    /// The language-dependent fixed-value description.
    #[serde(rename = "doma:text")]
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ObjectRef, ObjectType, Operation};

    const DOMAIN_TRKORR_XML: &str = include_str!("../../../tests/fixtures/domain-trkorr.xml");
    const DOMAIN_XFELD_XML: &str = include_str!("../../../tests/fixtures/domain-xfeld.xml");

    fn parse(body: &str) -> Result<DomainProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn trkorr() -> DomainProperties {
        parse(DOMAIN_TRKORR_XML).unwrap()
    }

    #[test]
    fn builds_the_sparse_creation_payload() {
        let properties = DomainCreateProperties::builder()
            .description("Created Domain")
            .package("$TMP")
            .build()
            .unwrap();
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, Domain::WORKBENCH_TYPE);
        assert!(properties.abap_language_version.is_none());

        let reference = ObjectRef::<Domain>::new(
            "Z_DOMAIN".to_owned(),
            AdtUri::parse("/sap/bc/adt/ddic/domains/z_domain").unwrap(),
        );
        let request = reference.create(properties).encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("<doma:domain"));
        assert!(body.contains("adtcore:name=\"Z_DOMAIN\""));
        assert!(body.contains("adtcore:type=\"DOMA/DD\""));
        assert!(body.contains("adtcore:description=\"Created Domain\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("doma:content"));
        assert!(!body.contains("adtcore:changedAt="));
        assert!(!body.contains("atom:link"));
    }

    #[test]
    fn parses_a_live_domain_with_a_value_table() {
        let properties = trkorr();
        let values = properties.content.value_information.as_ref().unwrap();

        assert_eq!(properties.name, "TRKORR");
        assert_eq!(properties.object_type, Domain::WORKBENCH_TYPE);
        assert_eq!(properties.version, WorkbenchVersion::Active);
        assert_eq!(properties.created_at, None);
        assert_eq!(properties.content.type_information.datatype, "CHAR");
        assert_eq!(properties.content.type_information.length, "000020");
        assert_eq!(values.value_table.name.as_deref(), Some("E070"));
        assert!(values.fixed_values.values.is_empty());
        assert_eq!(properties.links.len(), 4);
    }

    #[test]
    fn parses_a_live_domain_with_fixed_values() {
        let properties = parse(DOMAIN_XFELD_XML).unwrap();
        let values = properties.content.value_information.unwrap();

        assert_eq!(properties.name, "XFELD");
        assert_eq!(values.value_table, AdvertisedObjectReference::default());
        assert_eq!(values.fixed_values.values.len(), 2);
        assert_eq!(values.fixed_values.values[0].position, "0001");
        assert_eq!(values.fixed_values.values[0].low, "X");
        assert_eq!(values.fixed_values.values[1].low, "");
        assert_eq!(values.fixed_values.values[1].text, "Nein");
    }

    #[test]
    fn json_uses_the_wire_vocabulary_and_round_trips() {
        let properties = trkorr();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "TRKORR");
        assert_eq!(json["@adtcore:type"], "DOMA/DD");
        assert_eq!(
            json["doma:content"]["doma:typeInformation"]["doma:length"],
            "000020"
        );
        assert_eq!(
            json["doma:content"]["doma:valueInformation"]["doma:valueTableRef"]["@adtcore:name"],
            "E070"
        );
        assert!(json.get("name").is_none());

        let round_tripped: DomainProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn serializes_complete_properties_without_losing_wire_widths() {
        let properties = trkorr();
        let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();

        assert!(xml.contains("<doma:domain"));
        assert!(xml.contains("xmlns:doma=\"http://www.sap.com/dictionary/domain\""));
        assert!(xml.contains("<doma:length>000020</doma:length>"));
        assert!(xml.contains("<doma:decimals>000000</doma:decimals>"));
        assert!(xml.contains("<doma:valueTableRef adtcore:uri="));
        assert!(xml.contains("<doma:fixValues"));
        assert_eq!(parse(&xml).unwrap(), properties);
    }

    #[test]
    fn rejects_unmodeled_nested_fields_before_updates_can_drop_them() {
        let body = DOMAIN_TRKORR_XML.replacen(
            "<doma:datatype>",
            "<doma:futureField>value</doma:futureField><doma:datatype>",
            1,
        );

        assert!(parse(&body).is_err());
    }
}
