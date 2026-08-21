use serde::{Deserialize, Serialize};
use zadt_macros::object_type;

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    ObjectVersion, PropertyModel,
};

#[object_type(
    properties = DataElementProperties,
    workbench_type = "DTEL/DE",
    collection(scheme = "http://www.sap.com/wbobj/dictionary", term = "dtelde",),
    capabilities(UpdateProperties)
)]
/// The ABAP Dictionary Data Element object type.
pub struct DataElement;

/// The SAP media-type version used to decode Data Element properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataElementPropertiesVersion {
    /// Data Element properties V2.
    V2,
}

impl DataElementPropertiesVersion {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.dataelements.v2+xml",
        }
    }
}

/// The complete Data Element properties payload used by the V2 media type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "blue:wbobj", deny_unknown_fields)]
pub struct DataElementProperties {
    /// The Data Element name supplied by SAP.
    #[serde(rename = "@adtcore:name")]
    pub name: String,

    /// The repository object type, normally `DTEL/DE`.
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    /// The user responsible for the Data Element, when advertised.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: Option<String>,

    /// The Data Element's master language, when advertised.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: Option<String>,

    /// The object's master system, when advertised.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: Option<String>,

    /// The configured ABAP language version, when advertised.
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: Option<AbapLanguageVersion>,

    /// The timestamp at which the object was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: Option<String>,

    /// The object version, when advertised.
    #[serde(rename = "@adtcore:version")]
    pub version: Option<ObjectVersion>,

    /// The timestamp at which the object was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: Option<String>,

    /// The user who last changed the object.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: Option<String>,

    /// The user who created the object.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: Option<String>,

    /// The Data Element description, when advertised.
    #[serde(rename = "@adtcore:description")]
    pub description: Option<String>,

    /// The language in which language-dependent values are represented.
    #[serde(rename = "@adtcore:language")]
    pub language: Option<String>,

    /// Atom links exactly as advertised at the payload root.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,

    /// The package reference exactly as embedded in the payload.
    #[serde(rename = "adtcore:packageRef")]
    pub package: Option<AdvertisedObjectReference>,

    /// The Data Element's type definition and field behavior.
    #[serde(rename = "dtel:dataElement")]
    pub definition: DataElementDefinition,
}

impl PropertyModel for DataElementProperties {
    type Version = DataElementPropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] = &[DataElementPropertiesVersion::V2];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("blue", "http://www.sap.com/wbobj/dictionary/dtel"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("dtel", "http://www.sap.com/adt/dictionary/dataelements"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];

    fn object_name(&self) -> &str {
        &self.name
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }

    fn links(&self) -> &[AdvertisedLink] {
        &self.links
    }

    fn media_type(version: Self::Version) -> &'static str {
        version.media_type()
    }
}

/// The nested Data Element definition exactly as represented by ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataElementDefinition {
    #[serde(rename = "dtel:typeKind")]
    pub type_kind: String,
    #[serde(rename = "dtel:typeName")]
    pub type_name: Option<String>,
    #[serde(rename = "dtel:dataType")]
    pub data_type: Option<String>,
    #[serde(rename = "dtel:dataTypeLength")]
    pub data_type_length: Option<u32>,
    #[serde(rename = "dtel:dataTypeLengthEnabled")]
    pub data_type_length_enabled: Option<bool>,
    #[serde(rename = "dtel:dataTypeDecimals")]
    pub data_type_decimals: Option<u32>,
    #[serde(rename = "dtel:dataTypeDecimalsEnabled")]
    pub data_type_decimals_enabled: Option<bool>,
    #[serde(rename = "dtel:shortFieldLabel")]
    pub short_field_label: Option<String>,
    #[serde(rename = "dtel:shortFieldLength")]
    pub short_field_length: Option<u32>,
    #[serde(rename = "dtel:shortFieldMaxLength")]
    pub short_field_max_length: Option<u32>,
    #[serde(rename = "dtel:mediumFieldLabel")]
    pub medium_field_label: Option<String>,
    #[serde(rename = "dtel:mediumFieldLength")]
    pub medium_field_length: Option<u32>,
    #[serde(rename = "dtel:mediumFieldMaxLength")]
    pub medium_field_max_length: Option<u32>,
    #[serde(rename = "dtel:longFieldLabel")]
    pub long_field_label: Option<String>,
    #[serde(rename = "dtel:longFieldLength")]
    pub long_field_length: Option<u32>,
    #[serde(rename = "dtel:longFieldMaxLength")]
    pub long_field_max_length: Option<u32>,
    #[serde(rename = "dtel:headingFieldLabel")]
    pub heading_field_label: Option<String>,
    #[serde(rename = "dtel:headingFieldLength")]
    pub heading_field_length: Option<u32>,
    #[serde(rename = "dtel:headingFieldMaxLength")]
    pub heading_field_max_length: Option<u32>,
    #[serde(rename = "dtel:searchHelp")]
    pub search_help: Option<String>,
    #[serde(rename = "dtel:searchHelpParameter")]
    pub search_help_parameter: Option<String>,
    #[serde(rename = "dtel:setGetParameter")]
    pub set_get_parameter: Option<String>,
    #[serde(rename = "dtel:defaultComponentName")]
    pub default_component_name: Option<String>,
    #[serde(rename = "dtel:deactivateInputHistory")]
    pub deactivate_input_history: Option<bool>,
    #[serde(rename = "dtel:changeDocument")]
    pub change_document: Option<bool>,
    #[serde(rename = "dtel:leftToRightDirection")]
    pub left_to_right_direction: Option<bool>,
    #[serde(rename = "dtel:deactivateBIDIFiltering")]
    pub deactivate_bidi_filtering: Option<bool>,
    #[serde(rename = "dtel:documentationStatus")]
    pub documentation_status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectType;

    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/data-element-ztfrwtfrt-v2.xml");

    fn parse(body: &[u8]) -> Result<DataElementProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_reader(body)
    }

    fn properties() -> DataElementProperties {
        parse(DATA_ELEMENT_XML).unwrap()
    }

    fn to_xml(properties: &DataElementProperties) -> Result<String, serde_xml_rs::Error> {
        DataElementProperties::XML_NAMESPACES
            .iter()
            .fold(
                serde_xml_rs::SerdeXml::new(),
                |serializer, &(prefix, namespace)| serializer.namespace(prefix, namespace),
            )
            .to_string(properties)
    }

    fn sparse_properties_xml(type_kind: &str, extra: &str) -> String {
        let mut xml = String::from_utf8(DATA_ELEMENT_XML.to_vec()).unwrap();
        let start = xml.find("<dtel:dataElement").unwrap();
        let end_tag = "</dtel:dataElement>";
        let end = xml[start..].find(end_tag).unwrap() + start + end_tag.len();
        xml.replace_range(
            start..end,
            &format!(
                "<dtel:dataElement xmlns:dtel=\"http://www.sap.com/adt/dictionary/dataelements\"><dtel:typeKind>{type_kind}</dtel:typeKind>{extra}</dtel:dataElement>"
            ),
        );
        xml
    }

    fn without_root_attributes(names: &[&str]) -> String {
        let mut xml = String::from_utf8(DATA_ELEMENT_XML.to_vec()).unwrap();
        for name in names {
            let prefix = format!(" adtcore:{name}=\"");
            let start = xml.find(&prefix).unwrap();
            let value_start = start + prefix.len();
            let value_end = xml[value_start..].find('"').unwrap() + value_start;
            xml.replace_range(start..=value_end, "");
        }
        xml
    }

    #[test]
    fn parses_live_data_element_properties_as_one_representation() {
        let properties = properties();

        assert_eq!(properties.version, Some(ObjectVersion::New));
        assert_eq!(
            properties
                .abap_language_version
                .as_ref()
                .map(AbapLanguageVersion::as_str),
            Some("0")
        );
        assert_eq!(properties.master_system.as_deref(), Some("A4H"));
        assert_eq!(
            properties.package.as_ref().unwrap().name.as_deref(),
            Some("$TMP")
        );
        assert_eq!(properties.links.len(), 4);
        assert_eq!(properties.description.as_deref(), Some("tfarFAR"));
        assert_eq!(properties.definition.type_kind, "domain");
        assert_eq!(properties.definition.type_name.as_deref(), Some("CHAR0008"));
        assert_eq!(properties.definition.data_type_length, Some(8));
        assert_eq!(
            properties.definition.short_field_label.as_deref(),
            Some("123")
        );
        assert_eq!(properties.definition.search_help.as_deref(), Some(""));
    }

    #[test]
    fn json_uses_the_wire_vocabulary() {
        let properties = properties();
        let json = serde_json::to_value(&properties).unwrap();

        assert_eq!(json["@adtcore:name"], "ZTFRWTFRT");
        assert_eq!(json["@adtcore:type"], "DTEL/DE");
        assert_eq!(json["atom:link"][0]["@href"], "versions");
        assert_eq!(json["adtcore:packageRef"]["@adtcore:name"], "$TMP");
        assert_eq!(json["dtel:dataElement"]["dtel:typeKind"], "domain");
        assert!(json.get("name").is_none());

        let round_tripped: DataElementProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, properties);
    }

    #[test]
    fn update_xml_reuses_the_complete_properties_representation() {
        let properties = properties();
        let xml = to_xml(&properties).unwrap();

        assert!(xml.contains("<blue:wbobj"));
        assert!(xml.contains("adtcore:name=\"ZTFRWTFRT\""));
        assert!(xml.contains("adtcore:type=\"DTEL/DE\""));
        assert!(xml.contains("<dtel:dataTypeLength>8</dtel:dataTypeLength>"));
        assert!(xml.contains("<dtel:searchHelp"));
        assert!(xml.contains("<dtel:defaultComponentName"));
        assert!(xml.contains("adtcore:changedAt="));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert!(xml.contains("href=\"versions\""));
        assert!(xml.contains("rel=\"http://www.sap.com/adt/relations/versions\""));
        assert!(!xml.contains("<href>"));
        assert!(!xml.contains("data-element-etag"));

        let decoded = parse(xml.as_bytes()).unwrap();
        assert_eq!(decoded, properties);
        assert_eq!(decoded.object_type, DataElement::WORKBENCH_TYPE);
        assert_eq!(decoded.links.len(), 4);
        assert_eq!(
            decoded.package.unwrap().description.as_deref(),
            Some("Temporary Objects (never transported!)")
        );
        assert_eq!(decoded.definition.data_type_length, Some(8));
        assert_eq!(decoded.definition.search_help.as_deref(), Some(""));
    }

    #[test]
    fn preserves_variant_specific_omissions_for_every_type_kind() {
        for wire_value in [
            "domain",
            "predefinedAbapType",
            "refToPredefinedAbapType",
            "refToDictionaryType",
            "refToClifType",
        ] {
            let properties = parse(sparse_properties_xml(wire_value, "").as_bytes()).unwrap();
            let definition = &properties.definition;

            assert_eq!(definition.type_kind, wire_value);
            assert!(definition.type_name.is_none());
            assert!(definition.data_type.is_none());
            assert!(definition.short_field_label.is_none());

            let xml = to_xml(&properties).unwrap();
            assert!(xml.contains(&format!("<dtel:typeKind>{wire_value}</dtel:typeKind>")));
            assert!(!xml.contains("dtel:dataType"));
            assert!(!xml.contains("dtel:shortFieldLabel"));
        }
    }

    #[test]
    fn preserves_optional_root_attribute_omissions() {
        let xml =
            without_root_attributes(&["responsible", "masterLanguage", "description", "language"]);
        let properties = parse(xml.as_bytes()).unwrap();
        assert!(properties.responsible.is_none());
        assert!(properties.master_language.is_none());
        assert!(properties.description.is_none());
        assert!(properties.language.is_none());

        let update = to_xml(&properties).unwrap();
        let decoded = parse(update.as_bytes()).unwrap();
        assert!(decoded.responsible.is_none());
        assert!(decoded.master_language.is_none());
        assert!(decoded.description.is_none());
        assert!(decoded.language.is_none());
    }

    #[test]
    fn documentation_status_round_trips_with_the_properties() {
        let xml = sparse_properties_xml(
            "domain",
            "<dtel:documentationStatus>required</dtel:documentationStatus>",
        );
        let properties = parse(xml.as_bytes()).unwrap();

        assert_eq!(
            properties.definition.documentation_status.as_deref(),
            Some("required")
        );
        assert!(
            to_xml(&properties)
                .unwrap()
                .contains("<dtel:documentationStatus>required</dtel:documentationStatus>")
        );
    }

    #[test]
    fn rejects_unmodeled_definition_fields_before_they_can_be_dropped() {
        let xml = sparse_properties_xml("domain", "<dtel:futureField>value</dtel:futureField>");
        assert!(parse(xml.as_bytes()).is_err());
    }

    #[test]
    fn preserves_advertised_root_identity() {
        let xml = String::from_utf8(DATA_ELEMENT_XML.to_vec())
            .unwrap()
            .replacen("adtcore:type=\"DTEL/DE\"", "adtcore:type=\"DOMA/DD\"", 1)
            .replacen("adtcore:name=\"ZTFRWTFRT\"", "adtcore:name=\"ZOTHER\"", 1);
        let properties = parse(xml.as_bytes()).unwrap();

        assert_eq!(properties.object_type.as_str(), "DOMA/DD");
        assert_eq!(properties.name, "ZOTHER");
        let update = to_xml(&properties).unwrap();
        assert!(update.contains("adtcore:type=\"DOMA/DD\""));
        assert!(update.contains("adtcore:name=\"ZOTHER\""));
    }
}
