use serde::{Deserialize, Serialize};

use crate::{
    DataElement, GlobalWorkbenchType, MediaVersionNegotiation, ObjectError, ObjectRef, ObjectType,
    RawObjectProperties, ResponseError, WritableProperties,
};

const BLUE_NAMESPACE: &str = "http://www.sap.com/wbobj/dictionary/dtel";
const CORE_NAMESPACE: &str = "http://www.sap.com/adt/core";
const DATA_ELEMENT_NAMESPACE: &str = "http://www.sap.com/adt/dictionary/dataelements";
const ATOM_NAMESPACE: &str = "http://www.w3.org/2005/Atom";

/// The SAP media-type version used to decode Data Element properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataElementPropertiesVersion {
    /// Data Element properties V2.
    V2,
}

impl MediaVersionNegotiation for DataElementPropertiesVersion {
    const SUPPORTED: &'static [Self] = &[Self::V2];

    fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.dataelements.v2+xml",
        }
    }
}

/// The complete Data Element properties payload used by the V2 media type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename(deserialize = "blue:wbobj"),
    rename_all(serialize = "camelCase"),
    deny_unknown_fields
)]
pub struct DataElementProperties {
    /// The Data Element name supplied by SAP.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,

    /// The repository object type, normally `DTEL/DE`.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,

    /// The user responsible for the Data Element, when advertised.
    #[serde(rename(deserialize = "@adtcore:responsible"), alias = "responsible")]
    pub responsible: Option<String>,

    /// The Data Element's master language, when advertised.
    #[serde(
        rename(deserialize = "@adtcore:masterLanguage"),
        alias = "masterLanguage"
    )]
    pub master_language: Option<String>,

    /// The object's master system, when advertised.
    #[serde(rename(deserialize = "@adtcore:masterSystem"), alias = "masterSystem")]
    pub master_system: Option<String>,

    /// The configured ABAP language version, when advertised.
    #[serde(
        rename(deserialize = "@adtcore:abapLanguageVersion"),
        alias = "abapLanguageVersion"
    )]
    pub abap_language_version: Option<String>,

    /// The timestamp at which the object was last changed.
    #[serde(rename(deserialize = "@adtcore:changedAt"), alias = "lastChanged")]
    pub last_changed: Option<String>,

    /// The object version exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:version"), alias = "version")]
    pub version: Option<String>,

    /// The timestamp at which the object was created.
    #[serde(rename(deserialize = "@adtcore:createdAt"), alias = "createdAt")]
    pub created_at: Option<String>,

    /// The user who last changed the object.
    #[serde(rename(deserialize = "@adtcore:changedBy"), alias = "changedBy")]
    pub changed_by: Option<String>,

    /// The user who created the object.
    #[serde(rename(deserialize = "@adtcore:createdBy"), alias = "createdBy")]
    pub created_by: Option<String>,

    /// The Data Element description, when advertised.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: Option<String>,

    /// The language in which language-dependent values are represented.
    #[serde(rename(deserialize = "@adtcore:language"), alias = "language")]
    pub language: Option<String>,

    /// Atom links exactly as advertised at the payload root.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<DataElementLink>,

    /// The package reference exactly as embedded in the payload.
    #[serde(rename(deserialize = "adtcore:packageRef"), alias = "package")]
    pub package: Option<DataElementObjectReference>,

    /// The Data Element's type definition and field behavior.
    #[serde(rename(deserialize = "dtel:dataElement"), alias = "definition")]
    pub definition: DataElementDefinition,
}

/// The V2 Data Element media type uses the complete shared payload.
pub type DataElementPropertiesV2 = DataElementProperties;

impl TryFrom<RawObjectProperties<DataElement>> for DataElementProperties {
    type Error = ResponseError;

    fn try_from(raw: RawObjectProperties<DataElement>) -> Result<Self, Self::Error> {
        let properties: Self =
            serde_xml_rs::from_reader(raw.body.as_slice()).map_err(ObjectError::InvalidResponse)?;
        if properties.object_type != DataElement::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: DataElement::WORKBENCH_TYPE,
                actual: properties.object_type,
            }
            .into());
        }
        if !properties.name.eq_ignore_ascii_case(raw.resource.name()) {
            return Err(ObjectError::UnexpectedObjectName {
                expected: raw.resource.name().to_owned(),
                actual: properties.name,
            }
            .into());
        }
        Ok(properties)
    }
}

impl DataElementProperties {
    fn validate(&self, resource: &ObjectRef<DataElement>) -> Result<(), ObjectError> {
        if !self.name.eq_ignore_ascii_case(resource.name()) {
            return Err(ObjectError::UnexpectedObjectName {
                expected: resource.name().to_owned(),
                actual: self.name.clone(),
            });
        }
        if self.object_type != DataElement::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: DataElement::WORKBENCH_TYPE,
                actual: self.object_type.clone(),
            });
        }
        Ok(())
    }
}

impl WritableProperties<DataElement> for DataElementProperties {
    fn media_version(&self) -> DataElementPropertiesVersion {
        DataElementPropertiesVersion::V2
    }

    fn to_xml(&self, resource: &ObjectRef<DataElement>) -> Result<String, ObjectError> {
        self.validate(resource)?;
        serde_xml_rs::SerdeXml::new()
            .namespace("blue", BLUE_NAMESPACE)
            .namespace("adtcore", CORE_NAMESPACE)
            .namespace("dtel", DATA_ELEMENT_NAMESPACE)
            .namespace("atom", ATOM_NAMESPACE)
            .to_string(&WritableDataElement::new(self))
            .map_err(ObjectError::InvalidRequest)
    }
}

/// One raw Atom link embedded in a Data Element properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all(serialize = "camelCase"), deny_unknown_fields)]
pub struct DataElementLink {
    /// The target exactly as advertised by SAP.
    #[serde(rename(deserialize = "@href"), alias = "href")]
    pub href: String,
    /// The Atom relation URI, when advertised.
    #[serde(rename(deserialize = "@rel"), alias = "relation")]
    pub relation: Option<String>,
    /// The target representation's media type, when advertised.
    #[serde(rename(deserialize = "@type"), alias = "mediaType")]
    pub media_type: Option<String>,
    /// The target representation's language, when advertised.
    #[serde(rename(deserialize = "@hreflang"), alias = "hreflang")]
    pub hreflang: Option<String>,
    /// A human-readable link title, when advertised.
    #[serde(rename(deserialize = "@title"), alias = "title")]
    pub title: Option<String>,
    /// The target length exactly as advertised by SAP.
    #[serde(rename(deserialize = "@length"), alias = "length")]
    pub length: Option<String>,
    /// The target representation's entity tag, when advertised.
    #[serde(rename(deserialize = "@etag"), alias = "etag")]
    pub etag: Option<String>,
}

/// An object reference embedded in a Data Element properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all(serialize = "camelCase"), deny_unknown_fields)]
pub struct DataElementObjectReference {
    /// The referenced object name exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,
    /// The referenced object URI exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:uri"), alias = "uri")]
    pub uri: String,
    /// The referenced global Workbench type.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,
    /// The referenced object description, when advertised.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: Option<String>,
}

/// The nested Data Element definition exactly as represented by ADT.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all(serialize = "camelCase"), deny_unknown_fields)]
pub struct DataElementDefinition {
    #[serde(rename(deserialize = "dtel:typeKind"), alias = "typeKind")]
    pub type_kind: String,
    #[serde(rename(deserialize = "dtel:typeName"), alias = "typeName")]
    pub type_name: Option<String>,
    #[serde(rename(deserialize = "dtel:dataType"), alias = "dataType")]
    pub data_type: Option<String>,
    #[serde(rename(deserialize = "dtel:dataTypeLength"), alias = "dataTypeLength")]
    pub data_type_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:dataTypeLengthEnabled"),
        alias = "dataTypeLengthEnabled"
    )]
    pub data_type_length_enabled: Option<bool>,
    #[serde(
        rename(deserialize = "dtel:dataTypeDecimals"),
        alias = "dataTypeDecimals"
    )]
    pub data_type_decimals: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:dataTypeDecimalsEnabled"),
        alias = "dataTypeDecimalsEnabled"
    )]
    pub data_type_decimals_enabled: Option<bool>,
    #[serde(
        rename(deserialize = "dtel:shortFieldLabel"),
        alias = "shortFieldLabel"
    )]
    pub short_field_label: Option<String>,
    #[serde(
        rename(deserialize = "dtel:shortFieldLength"),
        alias = "shortFieldLength"
    )]
    pub short_field_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:shortFieldMaxLength"),
        alias = "shortFieldMaxLength"
    )]
    pub short_field_max_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:mediumFieldLabel"),
        alias = "mediumFieldLabel"
    )]
    pub medium_field_label: Option<String>,
    #[serde(
        rename(deserialize = "dtel:mediumFieldLength"),
        alias = "mediumFieldLength"
    )]
    pub medium_field_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:mediumFieldMaxLength"),
        alias = "mediumFieldMaxLength"
    )]
    pub medium_field_max_length: Option<u32>,
    #[serde(rename(deserialize = "dtel:longFieldLabel"), alias = "longFieldLabel")]
    pub long_field_label: Option<String>,
    #[serde(
        rename(deserialize = "dtel:longFieldLength"),
        alias = "longFieldLength"
    )]
    pub long_field_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:longFieldMaxLength"),
        alias = "longFieldMaxLength"
    )]
    pub long_field_max_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:headingFieldLabel"),
        alias = "headingFieldLabel"
    )]
    pub heading_field_label: Option<String>,
    #[serde(
        rename(deserialize = "dtel:headingFieldLength"),
        alias = "headingFieldLength"
    )]
    pub heading_field_length: Option<u32>,
    #[serde(
        rename(deserialize = "dtel:headingFieldMaxLength"),
        alias = "headingFieldMaxLength"
    )]
    pub heading_field_max_length: Option<u32>,
    #[serde(rename(deserialize = "dtel:searchHelp"), alias = "searchHelp")]
    pub search_help: Option<String>,
    #[serde(
        rename(deserialize = "dtel:searchHelpParameter"),
        alias = "searchHelpParameter"
    )]
    pub search_help_parameter: Option<String>,
    #[serde(
        rename(deserialize = "dtel:setGetParameter"),
        alias = "setGetParameter"
    )]
    pub set_get_parameter: Option<String>,
    #[serde(
        rename(deserialize = "dtel:defaultComponentName"),
        alias = "defaultComponentName"
    )]
    pub default_component_name: Option<String>,
    #[serde(
        rename(deserialize = "dtel:deactivateInputHistory"),
        alias = "deactivateInputHistory"
    )]
    pub deactivate_input_history: Option<bool>,
    #[serde(rename(deserialize = "dtel:changeDocument"), alias = "changeDocument")]
    pub change_document: Option<bool>,
    #[serde(
        rename(deserialize = "dtel:leftToRightDirection"),
        alias = "leftToRightDirection"
    )]
    pub left_to_right_direction: Option<bool>,
    #[serde(
        rename(deserialize = "dtel:deactivateBIDIFiltering"),
        alias = "deactivateBidiFiltering"
    )]
    pub deactivate_bidi_filtering: Option<bool>,
    #[serde(
        rename(deserialize = "dtel:documentationStatus"),
        alias = "documentationStatus"
    )]
    pub documentation_status: Option<String>,
}

#[derive(Serialize)]
#[serde(rename = "blue:wbobj")]
struct WritableDataElement<'a> {
    #[serde(
        rename = "@adtcore:responsible",
        skip_serializing_if = "Option::is_none"
    )]
    responsible: Option<&'a str>,
    #[serde(
        rename = "@adtcore:masterLanguage",
        skip_serializing_if = "Option::is_none"
    )]
    master_language: Option<&'a str>,
    #[serde(
        rename = "@adtcore:masterSystem",
        skip_serializing_if = "Option::is_none"
    )]
    master_system: Option<&'a str>,
    #[serde(
        rename = "@adtcore:abapLanguageVersion",
        skip_serializing_if = "Option::is_none"
    )]
    abap_language_version: Option<&'a str>,
    #[serde(rename = "@adtcore:name")]
    name: &'a str,
    #[serde(rename = "@adtcore:type")]
    object_type: &'a str,
    #[serde(rename = "@adtcore:changedAt", skip_serializing_if = "Option::is_none")]
    last_changed: Option<&'a str>,
    #[serde(rename = "@adtcore:version", skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    #[serde(rename = "@adtcore:createdAt", skip_serializing_if = "Option::is_none")]
    created_at: Option<&'a str>,
    #[serde(rename = "@adtcore:changedBy", skip_serializing_if = "Option::is_none")]
    changed_by: Option<&'a str>,
    #[serde(rename = "@adtcore:createdBy", skip_serializing_if = "Option::is_none")]
    created_by: Option<&'a str>,
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<&'a str>,
    #[serde(rename = "@adtcore:language", skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
    #[serde(rename = "atom:link")]
    links: &'a [DataElementLink],
    #[serde(rename = "adtcore:packageRef", skip_serializing_if = "Option::is_none")]
    package: Option<WritablePackageReference<'a>>,
    #[serde(rename = "dtel:dataElement")]
    definition: WritableDataElementDefinition<'a>,
}

impl<'a> WritableDataElement<'a> {
    fn new(properties: &'a DataElementPropertiesV2) -> Self {
        Self {
            responsible: properties.responsible.as_deref(),
            master_language: properties.master_language.as_deref(),
            master_system: properties.master_system.as_deref(),
            abap_language_version: properties.abap_language_version.as_deref(),
            name: &properties.name,
            object_type: properties.object_type.as_str(),
            last_changed: properties.last_changed.as_deref(),
            version: properties.version.as_deref(),
            created_at: properties.created_at.as_deref(),
            changed_by: properties.changed_by.as_deref(),
            created_by: properties.created_by.as_deref(),
            description: properties.description.as_deref(),
            language: properties.language.as_deref(),
            links: &properties.links,
            package: properties
                .package
                .as_ref()
                .map(WritablePackageReference::new),
            definition: WritableDataElementDefinition::new(&properties.definition),
        }
    }
}

#[derive(Serialize)]
struct WritablePackageReference<'a> {
    #[serde(rename = "@adtcore:name")]
    name: &'a str,
    #[serde(rename = "@adtcore:uri")]
    uri: &'a str,
    #[serde(rename = "@adtcore:type")]
    object_type: &'a str,
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<&'a str>,
}

impl<'a> WritablePackageReference<'a> {
    fn new(package: &'a DataElementObjectReference) -> Self {
        Self {
            name: &package.name,
            uri: &package.uri,
            object_type: package.object_type.as_str(),
            description: package.description.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct WritableDataElementDefinition<'a> {
    #[serde(rename = "dtel:typeKind")]
    type_kind: &'a str,
    #[serde(rename = "dtel:typeName", skip_serializing_if = "Option::is_none")]
    type_name: Option<&'a str>,
    #[serde(rename = "dtel:dataType", skip_serializing_if = "Option::is_none")]
    data_type: Option<&'a str>,
    #[serde(
        rename = "dtel:dataTypeLength",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_length: Option<u32>,
    #[serde(
        rename = "dtel:dataTypeLengthEnabled",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_length_enabled: Option<bool>,
    #[serde(
        rename = "dtel:dataTypeDecimals",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_decimals: Option<u32>,
    #[serde(
        rename = "dtel:dataTypeDecimalsEnabled",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_decimals_enabled: Option<bool>,
    #[serde(
        rename = "dtel:shortFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    short_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:shortFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    short_field_length: Option<u32>,
    #[serde(
        rename = "dtel:shortFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    short_field_max_length: Option<u32>,
    #[serde(
        rename = "dtel:mediumFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    medium_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:mediumFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    medium_field_length: Option<u32>,
    #[serde(
        rename = "dtel:mediumFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    medium_field_max_length: Option<u32>,
    #[serde(
        rename = "dtel:longFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    long_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:longFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    long_field_length: Option<u32>,
    #[serde(
        rename = "dtel:longFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    long_field_max_length: Option<u32>,
    #[serde(
        rename = "dtel:headingFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    heading_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:headingFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    heading_field_length: Option<u32>,
    #[serde(
        rename = "dtel:headingFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    heading_field_max_length: Option<u32>,
    #[serde(rename = "dtel:searchHelp", skip_serializing_if = "Option::is_none")]
    search_help: Option<&'a str>,
    #[serde(
        rename = "dtel:searchHelpParameter",
        skip_serializing_if = "Option::is_none"
    )]
    search_help_parameter: Option<&'a str>,
    #[serde(
        rename = "dtel:setGetParameter",
        skip_serializing_if = "Option::is_none"
    )]
    set_get_parameter: Option<&'a str>,
    #[serde(
        rename = "dtel:defaultComponentName",
        skip_serializing_if = "Option::is_none"
    )]
    default_component_name: Option<&'a str>,
    #[serde(
        rename = "dtel:deactivateInputHistory",
        skip_serializing_if = "Option::is_none"
    )]
    deactivate_input_history: Option<bool>,
    #[serde(
        rename = "dtel:changeDocument",
        skip_serializing_if = "Option::is_none"
    )]
    change_document: Option<bool>,
    #[serde(
        rename = "dtel:leftToRightDirection",
        skip_serializing_if = "Option::is_none"
    )]
    left_to_right_direction: Option<bool>,
    #[serde(
        rename = "dtel:deactivateBIDIFiltering",
        skip_serializing_if = "Option::is_none"
    )]
    deactivate_bidi_filtering: Option<bool>,
    #[serde(
        rename = "dtel:documentationStatus",
        skip_serializing_if = "Option::is_none"
    )]
    documentation_status: Option<&'a str>,
}

impl<'a> WritableDataElementDefinition<'a> {
    fn new(definition: &'a DataElementDefinition) -> Self {
        Self {
            type_kind: &definition.type_kind,
            type_name: definition.type_name.as_deref(),
            data_type: definition.data_type.as_deref(),
            data_type_length: definition.data_type_length,
            data_type_length_enabled: definition.data_type_length_enabled,
            data_type_decimals: definition.data_type_decimals,
            data_type_decimals_enabled: definition.data_type_decimals_enabled,
            short_field_label: definition.short_field_label.as_deref(),
            short_field_length: definition.short_field_length,
            short_field_max_length: definition.short_field_max_length,
            medium_field_label: definition.medium_field_label.as_deref(),
            medium_field_length: definition.medium_field_length,
            medium_field_max_length: definition.medium_field_max_length,
            long_field_label: definition.long_field_label.as_deref(),
            long_field_length: definition.long_field_length,
            long_field_max_length: definition.long_field_max_length,
            heading_field_label: definition.heading_field_label.as_deref(),
            heading_field_length: definition.heading_field_length,
            heading_field_max_length: definition.heading_field_max_length,
            search_help: definition.search_help.as_deref(),
            search_help_parameter: definition.search_help_parameter.as_deref(),
            set_get_parameter: definition.set_get_parameter.as_deref(),
            default_component_name: definition.default_component_name.as_deref(),
            deactivate_input_history: definition.deactivate_input_history,
            change_document: definition.change_document,
            left_to_right_direction: definition.left_to_right_direction,
            deactivate_bidi_filtering: definition.deactivate_bidi_filtering,
            documentation_status: definition.documentation_status.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, EntityTag};

    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../tests/fixtures/data-element-ztfrwtfrt-v2.xml");

    fn reference() -> ObjectRef<DataElement> {
        ObjectRef::<DataElement>::for_test(
            "ZTFRWTFRT",
            AdtUri::parse("/sap/bc/adt/ddic/dataelements/ztfrwtfrt").unwrap(),
        )
    }

    fn parse(
        resource: &ObjectRef<DataElement>,
        version: DataElementPropertiesVersion,
        body: &[u8],
        etag: Option<EntityTag>,
    ) -> Result<DataElementProperties, ResponseError> {
        DataElementProperties::try_from(RawObjectProperties {
            resource: resource.clone(),
            version,
            body: body.to_vec(),
            etag,
        })
    }

    fn properties() -> DataElementPropertiesV2 {
        parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            DATA_ELEMENT_XML,
            Some(EntityTag::from_static("data-element-etag")),
        )
        .unwrap()
    }

    fn sparse_properties_xml(type_kind: &str, extra: &str) -> String {
        let mut xml = String::from_utf8(DATA_ELEMENT_XML.to_vec()).unwrap();
        let start = xml.find("<dtel:dataElement").unwrap();
        let end_tag = "</dtel:dataElement>";
        let end = xml[start..].find(end_tag).unwrap() + start + end_tag.len();
        xml.replace_range(
            start..end,
            &format!(
                "<dtel:dataElement xmlns:dtel=\"{DATA_ELEMENT_NAMESPACE}\"><dtel:typeKind>{type_kind}</dtel:typeKind>{extra}</dtel:dataElement>"
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

        assert_eq!(properties.version.as_deref(), Some("new"));
        assert_eq!(properties.master_system.as_deref(), Some("A4H"));
        assert_eq!(properties.package.as_ref().unwrap().name, "$TMP");
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
    fn update_xml_reuses_the_complete_properties_representation() {
        let properties = properties();
        let xml = properties.to_xml(&reference()).unwrap();

        assert!(xml.contains("<blue:wbobj"));
        assert!(xml.contains("adtcore:name=\"ZTFRWTFRT\""));
        assert!(xml.contains("adtcore:type=\"DTEL/DE\""));
        assert!(xml.contains("<dtel:dataTypeLength>8</dtel:dataTypeLength>"));
        assert!(xml.contains("<dtel:searchHelp"));
        assert!(xml.contains("<dtel:defaultComponentName"));
        assert!(xml.contains("adtcore:changedAt="));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert!(!xml.contains("data-element-etag"));

        let decoded = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap();
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
            let properties = parse(
                &reference(),
                DataElementPropertiesVersion::V2,
                sparse_properties_xml(wire_value, "").as_bytes(),
                None,
            )
            .unwrap();
            let definition = &properties.definition;

            assert_eq!(definition.type_kind, wire_value);
            assert!(definition.type_name.is_none());
            assert!(definition.data_type.is_none());
            assert!(definition.short_field_label.is_none());

            let xml = properties.to_xml(&reference()).unwrap();
            assert!(xml.contains(&format!("<dtel:typeKind>{wire_value}</dtel:typeKind>")));
            assert!(!xml.contains("dtel:dataType"));
            assert!(!xml.contains("dtel:shortFieldLabel"));
        }
    }

    #[test]
    fn preserves_optional_root_attribute_omissions() {
        let xml =
            without_root_attributes(&["responsible", "masterLanguage", "description", "language"]);
        let properties = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap();
        assert!(properties.responsible.is_none());
        assert!(properties.master_language.is_none());
        assert!(properties.description.is_none());
        assert!(properties.language.is_none());

        let update = properties.to_xml(&reference()).unwrap();
        let decoded = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            update.as_bytes(),
            None,
        )
        .unwrap();
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
        let properties = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap();

        assert_eq!(
            properties.definition.documentation_status.as_deref(),
            Some("required")
        );
        assert!(
            properties
                .to_xml(&reference())
                .unwrap()
                .contains("<dtel:documentationStatus>required</dtel:documentationStatus>")
        );
    }

    #[test]
    fn rejects_unmodeled_definition_fields_before_they_can_be_dropped() {
        let xml = sparse_properties_xml("domain", "<dtel:futureField>value</dtel:futureField>");
        let error = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::InvalidResponse(_))
        ));
    }

    #[test]
    fn rejects_properties_fetched_for_another_resource() {
        let properties = properties();
        let other = ObjectRef::<DataElement>::for_test(
            "ZOTHER",
            AdtUri::parse("/sap/bc/adt/ddic/dataelements/zother").unwrap(),
        );

        assert!(matches!(
            properties.to_xml(&other),
            Err(ObjectError::UnexpectedObjectName { .. })
        ));
    }

    #[test]
    fn rejects_an_unexpected_object_type() {
        let xml = String::from_utf8(DATA_ELEMENT_XML.to_vec())
            .unwrap()
            .replacen("adtcore:type=\"DTEL/DE\"", "adtcore:type=\"DOMA/DD\"", 1);
        let error = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::UnexpectedObjectType { expected, actual })
                if expected == DataElement::WORKBENCH_TYPE && actual.as_str() == "DOMA/DD"
        ));
    }

    #[test]
    fn rejects_an_unexpected_object_name() {
        let xml = String::from_utf8(DATA_ELEMENT_XML.to_vec())
            .unwrap()
            .replacen("adtcore:name=\"ZTFRWTFRT\"", "adtcore:name=\"ZOTHER\"", 1);
        let error = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::UnexpectedObjectName { expected, actual })
                if expected == "ZTFRWTFRT" && actual == "ZOTHER"
        ));
    }
}
