use serde::{Deserialize, Serialize};
use zadt::{
    DataElement, DataElementDefinition, DataElementProperties, Erased, GlobalWorkbenchType,
    JsonObjectProperties, ObjectRef, ObjectType,
};

use crate::{
    Cardinality, ComponentId, FileBacking, FileSpec, ObjectFormat, ProjectionError,
    format::{
        FileDescriptor, FormatDescriptor, PropertiesCodec, decode_properties, encode_properties,
    },
    language,
};

pub const DATA_ELEMENT_FORMAT: ObjectFormat = ObjectFormat::new("DTEL", "1");

#[derive(Debug)]
pub(crate) struct DataElementDescriptor;

#[derive(Debug)]
struct DataElementMetadata;

static DATA_ELEMENT_FILES: &[FileSpec] = &[FileSpec::new(
    "<name>.dtel.json",
    Cardinality::One,
    &DataElementMetadata,
)];

impl FormatDescriptor for DataElementDescriptor {
    fn format(&self) -> ObjectFormat {
        DATA_ELEMENT_FORMAT
    }

    fn repository_types(&self) -> &'static [GlobalWorkbenchType] {
        const TYPES: &[GlobalWorkbenchType] = &[DataElement::WORKBENCH_TYPE];
        TYPES
    }

    fn files(&self) -> &'static [FileSpec] {
        DATA_ELEMENT_FILES
    }

    fn repository_type_from_metadata(
        &self,
        metadata: &[u8],
    ) -> Result<GlobalWorkbenchType, ProjectionError> {
        let metadata: MetadataDiscriminator = serde_json::from_slice(metadata)?;
        if metadata.format_version != DATA_ELEMENT_FORMAT.version() {
            return Err(ProjectionError::UnsupportedFormatVersion {
                object_type: DATA_ELEMENT_FORMAT.object_type(),
                version: metadata.format_version,
            });
        }
        Ok(DataElement::WORKBENCH_TYPE)
    }
}

impl FileDescriptor for DataElementMetadata {
    fn component(&self) -> ComponentId {
        ComponentId::new("metadata")
    }

    fn bind(
        &self,
        object: &ObjectRef<Erased>,
        _language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError> {
        if object.object_type() != &DataElement::WORKBENCH_TYPE {
            return Err(ProjectionError::UnsupportedFileComponent {
                object_type: object.object_type().clone(),
                component: self.component(),
            });
        }
        Ok(Some(FileBacking::Properties(object.clone())))
    }

    fn properties_codec(&self) -> Option<&dyn PropertiesCodec> {
        Some(self)
    }
}

impl PropertiesCodec for DataElementMetadata {
    fn render(&self, properties: &JsonObjectProperties) -> Result<String, ProjectionError> {
        render_data_element_properties(&decode_properties::<DataElementProperties>(
            properties, "DTEL",
        )?)
    }

    fn merge(
        &self,
        original: &JsonObjectProperties,
        edited: &str,
    ) -> Result<JsonObjectProperties, ProjectionError> {
        let properties = decode_properties::<DataElementProperties>(original, "DTEL")?;
        encode_properties(
            original,
            merge_data_element_properties(&properties, edited)?,
            "DTEL",
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDiscriminator {
    format_version: String,
}

const DATA_TYPES: &[&str] = &[
    "ACCP",
    "CHAR",
    "CLNT",
    "CUKY",
    "CURR",
    "DF16_DEC",
    "DF16_RAW",
    "DF16_SCL",
    "DECFLOAT16",
    "DF34_DEC",
    "DF34_RAW",
    "DF34_SCL",
    "DECFLOAT34",
    "DATS",
    "DATN",
    "DEC",
    "FLTP",
    "GEOM_EWKB",
    "INT1",
    "INT2",
    "INT4",
    "INT8",
    "LANG",
    "LCHR",
    "LRAW",
    "NUMC",
    "PREC",
    "QUAN",
    "RAW",
    "RAWSTRING",
    "SSTRING",
    "STRING",
    "TIMS",
    "TIMN",
    "UNIT",
    "UTCLONG",
    "VARC",
];

/// The AFF v1 representation of a Dictionary Data Element.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffDataElement {
    pub format_version: String,
    pub header: AffDataElementHeader,
    pub data_type_information: AffDataElementTypeInformation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_labels: Option<AffDataElementFieldLabels>,
    #[serde(
        default,
        skip_serializing_if = "AffDataElementAdditionalProperties::is_empty"
    )]
    pub additional_properties: AffDataElementAdditionalProperties,
}

/// AFF Data Element header fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffDataElementHeader {
    pub description: String,
    pub original_language: String,
    #[serde(default, skip_serializing_if = "AffAbapLanguageVersion::is_standard")]
    pub abap_language_version: AffAbapLanguageVersion,
}

/// AFF ABAP language-version names for non-source objects.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffAbapLanguageVersion {
    #[default]
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "keyUser")]
    KeyUser,
    #[serde(rename = "cloudDevelopment")]
    CloudDevelopment,
}

impl AffAbapLanguageVersion {
    fn from_adt(value: Option<&str>) -> Result<Self, ProjectionError> {
        match value {
            None | Some("" | " " | "0") => Ok(Self::Standard),
            Some("2") => Ok(Self::KeyUser),
            Some("5") => Ok(Self::CloudDevelopment),
            Some(version) => Err(invalid_field(
                "header.abapLanguageVersion",
                format!("unsupported ADT value `{version}`"),
            )),
        }
    }

    const fn is_standard(&self) -> bool {
        matches!(self, Self::Standard)
    }

    fn adt_value(self, original: Option<&str>) -> Option<String> {
        match self {
            Self::Standard if original.is_none() => None,
            Self::Standard if original.is_some_and(|value| matches!(value, "" | " " | "0")) => {
                original.map(str::to_owned)
            }
            Self::Standard => Some("0".to_owned()),
            Self::KeyUser => Some("2".to_owned()),
            Self::CloudDevelopment => Some("5".to_owned()),
        }
    }
}

/// AFF Data Element type information.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffDataElementTypeInformation {
    pub category: AffDataElementCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predefined_type: Option<AffPredefinedType>,
}

/// AFF category names, which intentionally differ from some ADT wire values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffDataElementCategory {
    #[serde(rename = "domain")]
    Domain,
    #[serde(rename = "predefinedType")]
    PredefinedType,
    #[serde(rename = "referenceToPredefinedType")]
    ReferenceToPredefinedType,
    #[serde(rename = "referenceDictionaryType")]
    ReferenceDictionaryType,
    #[serde(rename = "referenceClasIntType")]
    ReferenceClassOrInterfaceType,
}

impl AffDataElementCategory {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "domain" => Ok(Self::Domain),
            "predefinedAbapType" => Ok(Self::PredefinedType),
            "refToPredefinedAbapType" => Ok(Self::ReferenceToPredefinedType),
            "refToDictionaryType" => Ok(Self::ReferenceDictionaryType),
            "refToClifType" => Ok(Self::ReferenceClassOrInterfaceType),
            value => Err(invalid_field(
                "dataTypeInformation.category",
                format!("unsupported ADT type kind `{value}`"),
            )),
        }
    }

    const fn adt_value(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::PredefinedType => "predefinedAbapType",
            Self::ReferenceToPredefinedType => "refToPredefinedAbapType",
            Self::ReferenceDictionaryType => "refToDictionaryType",
            Self::ReferenceClassOrInterfaceType => "refToClifType",
        }
    }
}

/// An AFF predefined ABAP type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffPredefinedType {
    pub data_type: String,
    pub length: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
}

/// AFF field labels and their configured output lengths.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffDataElementFieldLabels {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_length: Option<u32>,
}

impl AffDataElementFieldLabels {
    fn from_definition(definition: &DataElementDefinition) -> Option<Self> {
        let labels = Self {
            short: nonempty(definition.short_field_label.as_deref()),
            short_length: nonzero(definition.short_field_length),
            medium: nonempty(definition.medium_field_label.as_deref()),
            medium_length: nonzero(definition.medium_field_length),
            long: nonempty(definition.long_field_label.as_deref()),
            long_length: nonzero(definition.long_field_length),
            heading: nonempty(definition.heading_field_label.as_deref()),
            heading_length: nonzero(definition.heading_field_length),
        };
        (!labels.is_empty()).then_some(labels)
    }

    fn is_empty(&self) -> bool {
        self.short.is_none()
            && self.short_length.is_none()
            && self.medium.is_none()
            && self.medium_length.is_none()
            && self.long.is_none()
            && self.long_length.is_none()
            && self.heading.is_none()
            && self.heading_length.is_none()
    }
}

/// AFF properties outside the Data Element's type and labels.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffDataElementAdditionalProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_help: Option<AffSearchHelp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidirectional_options: Option<AffBidirectionalOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_component_name: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub change_document_relevant: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_input_history: bool,
}

impl AffDataElementAdditionalProperties {
    fn from_definition(definition: &DataElementDefinition) -> Self {
        let search_help_name = nonempty(definition.search_help.as_deref());
        let search_help_parameter = nonempty(definition.search_help_parameter.as_deref());
        let search_help =
            (search_help_name.is_some() || search_help_parameter.is_some()).then(|| {
                AffSearchHelp {
                    name: search_help_name.unwrap_or_default(),
                    parameter: search_help_parameter.unwrap_or_default(),
                }
            });
        let basic_direction = match definition.left_to_right_direction {
            Some(false) => AffBasicDirection::RightToLeft,
            Some(true) | None => AffBasicDirection::LeftToRight,
        };
        let no_filtering = definition.deactivate_bidi_filtering.unwrap_or(false);
        let bidirectional_options = (basic_direction != AffBasicDirection::LeftToRight
            || no_filtering)
            .then_some(AffBidirectionalOptions {
                basic_direction,
                no_filtering,
            });
        Self {
            search_help,
            bidirectional_options,
            parameter_id: nonempty(definition.set_get_parameter.as_deref()),
            default_component_name: nonempty(definition.default_component_name.as_deref()),
            change_document_relevant: definition.change_document.unwrap_or(false),
            no_input_history: definition.deactivate_input_history.unwrap_or(false),
        }
    }

    fn is_empty(&self) -> bool {
        self.search_help.is_none()
            && self.bidirectional_options.is_none()
            && self.parameter_id.is_none()
            && self.default_component_name.is_none()
            && !self.change_document_relevant
            && !self.no_input_history
    }
}

/// An AFF search-help assignment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffSearchHelp {
    pub name: String,
    pub parameter: String,
}

/// AFF bidirectional text options.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffBidirectionalOptions {
    #[serde(default, skip_serializing_if = "AffBasicDirection::is_left_to_right")]
    pub basic_direction: AffBasicDirection,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_filtering: bool,
}

/// The basic writing direction exposed by AFF.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffBasicDirection {
    #[default]
    #[serde(rename = "leftToRight")]
    LeftToRight,
    #[serde(rename = "rightToLeft")]
    RightToLeft,
}

impl AffBasicDirection {
    const fn is_left_to_right(&self) -> bool {
        matches!(self, Self::LeftToRight)
    }
}

/// Renders typed ZADT Data Element properties as canonical AFF JSON.
pub(crate) fn render_data_element_properties(
    properties: &DataElementProperties,
) -> Result<String, ProjectionError> {
    let document = document_from_properties(properties)?;
    let mut content = serde_json::to_string_pretty(&document)
        .map_err(ProjectionError::InvalidDataElementDocument)?;
    content.push('\n');
    Ok(content)
}

fn document_from_properties(
    properties: &DataElementProperties,
) -> Result<AffDataElement, ProjectionError> {
    let definition = &properties.definition;
    let category = AffDataElementCategory::from_adt(&definition.type_kind)?;
    let predefined_type = match (
        category,
        definition.data_type.clone(),
        definition.data_type_length,
    ) {
        (AffDataElementCategory::PredefinedType, Some(data_type), Some(length)) => {
            Some(AffPredefinedType {
                data_type,
                length,
                decimals: definition.data_type_decimals.filter(|value| *value != 0),
            })
        }
        _ => None,
    };
    let document = AffDataElement {
        format_version: DATA_ELEMENT_FORMAT.version().to_owned(),
        header: AffDataElementHeader {
            description: required(properties.description.clone(), "header.description")?,
            original_language: language::from_adt(
                &required(
                    properties.master_language.clone(),
                    "header.originalLanguage",
                )?,
                "header.originalLanguage",
            )?,
            abap_language_version: AffAbapLanguageVersion::from_adt(
                properties.abap_language_version.as_deref(),
            )?,
        },
        data_type_information: AffDataElementTypeInformation {
            category,
            type_name: definition.type_name.clone(),
            predefined_type,
        },
        field_labels: AffDataElementFieldLabels::from_definition(definition),
        additional_properties: AffDataElementAdditionalProperties::from_definition(definition),
    };
    document.validate()?;
    Ok(document)
}

/// Merges edited AFF JSON into the complete original ZADT properties value.
pub(crate) fn merge_data_element_properties(
    original: &DataElementProperties,
    edited: &str,
) -> Result<DataElementProperties, ProjectionError> {
    let document: AffDataElement =
        serde_json::from_str(edited).map_err(ProjectionError::InvalidDataElementDocument)?;
    document.validate()?;
    let original_properties = original;
    let original_document = document_from_properties(original_properties)?;
    let original_language_version = original_properties.abap_language_version.clone();
    let mut definition = original_properties.definition.clone();
    apply_document(&original_document, &document, &mut definition);

    let mut merged = original.clone();
    let properties = &mut merged;
    if document.header.description != original_document.header.description {
        properties.description = Some(document.header.description);
    }
    if document.header.original_language != original_document.header.original_language {
        properties.master_language = Some(language::to_adt(
            &document.header.original_language,
            "header.originalLanguage",
        )?);
    }
    if document.header.abap_language_version != original_document.header.abap_language_version {
        properties.abap_language_version = document
            .header
            .abap_language_version
            .adt_value(original_language_version.as_deref());
    }
    properties.definition = definition;
    Ok(merged)
}

impl AffDataElement {
    fn validate(&self) -> Result<(), ProjectionError> {
        if self.format_version != DATA_ELEMENT_FORMAT.version() {
            return Err(invalid_field(
                "formatVersion",
                format!("expected `{}`", DATA_ELEMENT_FORMAT.version()),
            ));
        }
        max_length("header.description", &self.header.description, 60)?;
        language::to_adt(&self.header.original_language, "header.originalLanguage")?;
        if let Some(type_name) = &self.data_type_information.type_name {
            max_length("dataTypeInformation.typeName", type_name, 30)?;
        }
        if let Some(predefined) = &self.data_type_information.predefined_type {
            if !DATA_TYPES.contains(&predefined.data_type.as_str()) {
                return Err(invalid_field(
                    "dataTypeInformation.predefinedType.dataType",
                    format!("unsupported data type `{}`", predefined.data_type),
                ));
            }
            maximum(
                "dataTypeInformation.predefinedType.length",
                predefined.length,
                999_999,
            )?;
            if let Some(decimals) = predefined.decimals {
                maximum(
                    "dataTypeInformation.predefinedType.decimals",
                    decimals,
                    999_999,
                )?;
            }
        }
        if let Some(labels) = &self.field_labels {
            validate_optional_text("fieldLabels.short", labels.short.as_deref(), 10)?;
            validate_optional_number("fieldLabels.shortLength", labels.short_length, 10)?;
            validate_optional_text("fieldLabels.medium", labels.medium.as_deref(), 20)?;
            validate_optional_number("fieldLabels.mediumLength", labels.medium_length, 20)?;
            validate_optional_text("fieldLabels.long", labels.long.as_deref(), 40)?;
            validate_optional_number("fieldLabels.longLength", labels.long_length, 40)?;
            validate_optional_text("fieldLabels.heading", labels.heading.as_deref(), 55)?;
            validate_optional_number("fieldLabels.headingLength", labels.heading_length, 55)?;
        }
        if let Some(search_help) = &self.additional_properties.search_help {
            max_length(
                "additionalProperties.searchHelp.name",
                &search_help.name,
                30,
            )?;
            max_length(
                "additionalProperties.searchHelp.parameter",
                &search_help.parameter,
                30,
            )?;
        }
        if let Some(parameter) = &self.additional_properties.parameter_id {
            max_length("additionalProperties.parameterId", parameter, 20)?;
        }
        if let Some(component) = &self.additional_properties.default_component_name {
            max_length("additionalProperties.defaultComponentName", component, 30)?;
        }
        Ok(())
    }
}

fn apply_document(
    original: &AffDataElement,
    edited: &AffDataElement,
    definition: &mut DataElementDefinition,
) {
    let original_type = &original.data_type_information;
    let edited_type = &edited.data_type_information;
    if edited_type.category != original_type.category {
        definition.type_kind = edited_type.category.adt_value().to_owned();
    }
    if edited_type.type_name != original_type.type_name {
        definition.type_name = edited_type.type_name.clone();
        if edited_type.category != AffDataElementCategory::PredefinedType {
            definition.data_type = None;
            definition.data_type_length = None;
            definition.data_type_decimals = None;
            definition.data_type_length_enabled = None;
            definition.data_type_decimals_enabled = None;
        }
    }
    match (&original_type.predefined_type, &edited_type.predefined_type) {
        (None, Some(predefined)) => {
            definition.data_type = Some(predefined.data_type.clone());
            definition.data_type_length = Some(predefined.length);
            definition.data_type_decimals = Some(predefined.decimals.unwrap_or(0));
            definition.data_type_length_enabled = None;
            definition.data_type_decimals_enabled = None;
        }
        (Some(original_predefined), Some(edited_predefined)) => {
            if edited_predefined.data_type != original_predefined.data_type {
                definition.data_type = Some(edited_predefined.data_type.clone());
            }
            if edited_predefined.length != original_predefined.length {
                definition.data_type_length = Some(edited_predefined.length);
            }
            if edited_predefined.decimals != original_predefined.decimals {
                definition.data_type_decimals = Some(edited_predefined.decimals.unwrap_or(0));
            }
        }
        (Some(_), None) => {
            definition.type_name = edited_type.type_name.clone();
            definition.data_type = None;
            definition.data_type_length = None;
            definition.data_type_decimals = None;
            definition.data_type_length_enabled = None;
            definition.data_type_decimals_enabled = None;
        }
        (None, None) if edited_type.category != original_type.category => {
            definition.type_name = edited_type.type_name.clone();
            definition.data_type = None;
            definition.data_type_length = None;
            definition.data_type_decimals = None;
            definition.data_type_length_enabled = None;
            definition.data_type_decimals_enabled = None;
        }
        _ => {}
    }

    let original_labels = original.field_labels.as_ref();
    let edited_labels = edited.field_labels.as_ref();
    apply_label(
        &mut definition.short_field_label,
        &mut definition.short_field_length,
        original_labels.and_then(|labels| labels.short.as_deref()),
        original_labels.and_then(|labels| labels.short_length),
        edited_labels.and_then(|labels| labels.short.as_deref()),
        edited_labels.and_then(|labels| labels.short_length),
    );
    apply_label(
        &mut definition.medium_field_label,
        &mut definition.medium_field_length,
        original_labels.and_then(|labels| labels.medium.as_deref()),
        original_labels.and_then(|labels| labels.medium_length),
        edited_labels.and_then(|labels| labels.medium.as_deref()),
        edited_labels.and_then(|labels| labels.medium_length),
    );
    apply_label(
        &mut definition.long_field_label,
        &mut definition.long_field_length,
        original_labels.and_then(|labels| labels.long.as_deref()),
        original_labels.and_then(|labels| labels.long_length),
        edited_labels.and_then(|labels| labels.long.as_deref()),
        edited_labels.and_then(|labels| labels.long_length),
    );
    apply_label(
        &mut definition.heading_field_label,
        &mut definition.heading_field_length,
        original_labels.and_then(|labels| labels.heading.as_deref()),
        original_labels.and_then(|labels| labels.heading_length),
        edited_labels.and_then(|labels| labels.heading.as_deref()),
        edited_labels.and_then(|labels| labels.heading_length),
    );

    let original_additional = &original.additional_properties;
    let edited_additional = &edited.additional_properties;
    let original_search_help = original_additional.search_help.as_ref();
    let edited_search_help = edited_additional.search_help.as_ref();
    if edited_search_help.map(|search_help| &search_help.name)
        != original_search_help.map(|search_help| &search_help.name)
    {
        definition.search_help = Some(
            edited_search_help
                .map(|search_help| search_help.name.clone())
                .unwrap_or_default(),
        );
    }
    if edited_search_help.map(|search_help| &search_help.parameter)
        != original_search_help.map(|search_help| &search_help.parameter)
    {
        definition.search_help_parameter = Some(
            edited_search_help
                .map(|search_help| search_help.parameter.clone())
                .unwrap_or_default(),
        );
    }
    if edited_additional.parameter_id != original_additional.parameter_id {
        definition.set_get_parameter =
            Some(edited_additional.parameter_id.clone().unwrap_or_default());
    }
    if edited_additional.default_component_name != original_additional.default_component_name {
        definition.default_component_name = Some(
            edited_additional
                .default_component_name
                .clone()
                .unwrap_or_default(),
        );
    }
    if edited_additional.change_document_relevant != original_additional.change_document_relevant {
        definition.change_document = Some(edited_additional.change_document_relevant);
    }
    if edited_additional.no_input_history != original_additional.no_input_history {
        definition.deactivate_input_history = Some(edited_additional.no_input_history);
    }
    let original_direction = original_additional
        .bidirectional_options
        .as_ref()
        .map(|options| options.basic_direction)
        .unwrap_or_default();
    let edited_direction = edited_additional
        .bidirectional_options
        .as_ref()
        .map(|options| options.basic_direction)
        .unwrap_or_default();
    if edited_direction != original_direction {
        definition.left_to_right_direction =
            Some(edited_direction == AffBasicDirection::LeftToRight);
    }
    let original_no_filtering = original_additional
        .bidirectional_options
        .as_ref()
        .is_some_and(|options| options.no_filtering);
    let edited_no_filtering = edited_additional
        .bidirectional_options
        .as_ref()
        .is_some_and(|options| options.no_filtering);
    if edited_no_filtering != original_no_filtering {
        definition.deactivate_bidi_filtering = Some(edited_no_filtering);
    }
}

fn apply_label(
    text: &mut Option<String>,
    length: &mut Option<u32>,
    original_text: Option<&str>,
    original_length: Option<u32>,
    edited_text: Option<&str>,
    edited_length: Option<u32>,
) {
    if edited_text != original_text {
        *text = Some(edited_text.unwrap_or_default().to_owned());
    }
    if edited_length != original_length {
        *length = Some(edited_length.unwrap_or(0));
    }
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, ProjectionError> {
    value.ok_or_else(|| invalid_field(field, "is required by AFF"))
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(str::to_owned)
}

fn nonzero(value: Option<u32>) -> Option<u32> {
    value.filter(|value| *value != 0)
}

fn is_false(value: &bool) -> bool {
    !value
}

fn validate_optional_text(
    field: &'static str,
    value: Option<&str>,
    maximum: usize,
) -> Result<(), ProjectionError> {
    if let Some(value) = value {
        max_length(field, value, maximum)?;
    }
    Ok(())
}

fn validate_optional_number(
    field: &'static str,
    value: Option<u32>,
    maximum_value: u32,
) -> Result<(), ProjectionError> {
    if let Some(value) = value {
        maximum(field, value, maximum_value)?;
    }
    Ok(())
}

fn max_length(field: &'static str, value: &str, maximum: usize) -> Result<(), ProjectionError> {
    let length = value.chars().count();
    if length > maximum {
        return Err(invalid_field(
            field,
            format!("length {length} exceeds maximum {maximum}"),
        ));
    }
    Ok(())
}

fn maximum(field: &'static str, value: u32, maximum: u32) -> Result<(), ProjectionError> {
    if value > maximum {
        return Err(invalid_field(
            field,
            format!("value {value} exceeds maximum {maximum}"),
        ));
    }
    Ok(())
}

fn invalid_field(field: &'static str, message: impl Into<String>) -> ProjectionError {
    ProjectionError::InvalidDataElementField {
        field,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use zadt::{DataElement, DataElementPropertiesVersion};

    use super::*;

    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../../zadt/tests/fixtures/data-element-ztfrwtfrt-v2.xml");

    fn properties() -> DataElementProperties {
        let reference = crate::test_support::reference::<DataElement>(
            "ZTFRWTFRT",
            "/sap/bc/adt/ddic/dataelements/ztfrwtfrt",
        );
        let mut properties = crate::test_support::properties(
            &reference,
            DataElementPropertiesVersion::V2.media_type(),
            "data-element-etag",
            DATA_ELEMENT_XML,
        )
        .payload;
        let properties_v2 = &mut properties;
        properties_v2.description = Some("Example data element".to_owned());
        properties_v2.master_language = Some("EN".to_owned());
        properties_v2.abap_language_version = Some("0".to_owned());
        properties_v2.definition.type_kind = "domain".to_owned();
        properties_v2.definition.type_name = Some("Z_EXAMPLE_DOMAIN".to_owned());
        properties_v2.definition.data_type = Some("CHAR".to_owned());
        properties_v2.definition.data_type_length = Some(8);
        properties_v2.definition.data_type_decimals = Some(0);
        properties_v2.definition.short_field_label = Some("Example".to_owned());
        properties_v2.definition.short_field_length = Some(10);
        properties_v2.definition.medium_field_label = Some("Example field".to_owned());
        properties_v2.definition.medium_field_length = Some(13);
        properties_v2.definition.long_field_label = Some("Example data element field".to_owned());
        properties_v2.definition.long_field_length = Some(26);
        properties_v2.definition.heading_field_label = Some("Example data element".to_owned());
        properties_v2.definition.heading_field_length = Some(20);
        properties_v2.definition.left_to_right_direction = Some(true);
        properties_v2.definition.documentation_status = Some("required".to_owned());
        properties
    }

    #[test]
    fn renders_domain_properties_as_aff_v1() {
        let content = render_data_element_properties(&properties()).unwrap();
        let document: Value = serde_json::from_str(&content).unwrap();

        assert!(content.ends_with('\n'));
        assert_eq!(document["formatVersion"], "1");
        assert_eq!(document["header"]["description"], "Example data element");
        assert_eq!(document["header"]["originalLanguage"], "en");
        assert!(document["header"].get("abapLanguageVersion").is_none());
        assert_eq!(document["dataTypeInformation"]["category"], "domain");
        assert_eq!(
            document["dataTypeInformation"]["typeName"],
            "Z_EXAMPLE_DOMAIN"
        );
        assert!(
            document["dataTypeInformation"]
                .get("predefinedType")
                .is_none()
        );
        assert_eq!(document["fieldLabels"]["short"], "Example");
        assert!(document.get("additionalProperties").is_none());
    }

    #[test]
    fn merges_an_aff_edit() {
        let original = properties();
        let content = render_data_element_properties(&original).unwrap();
        let edited = content.replacen(
            "\"description\": \"Example data element\"",
            "\"description\": \"Updated data element\"",
            1,
        );
        let merged = merge_data_element_properties(&original, &edited).unwrap();

        assert_eq!(merged.description.as_deref(), Some("Updated data element"));
    }

    #[test]
    fn translates_every_adt_type_kind_to_the_aff_schema_name() {
        for (adt, aff) in [
            ("domain", "domain"),
            ("predefinedAbapType", "predefinedType"),
            ("refToPredefinedAbapType", "referenceToPredefinedType"),
            ("refToDictionaryType", "referenceDictionaryType"),
            ("refToClifType", "referenceClasIntType"),
        ] {
            let mut properties = properties();
            properties.definition.type_kind = adt.to_owned();
            if adt == "predefinedAbapType" {
                properties.definition.data_type = Some("CHAR".to_owned());
                properties.definition.data_type_length = Some(12);
            }

            let content = render_data_element_properties(&properties).unwrap();
            let document: Value = serde_json::from_str(&content).unwrap();

            assert_eq!(document["dataTypeInformation"]["category"], aff);
        }
    }

    #[test]
    fn rejects_unmodeled_adt_type_kinds() {
        let mut properties = properties();
        properties.definition.type_kind = "futureTypeKind".to_owned();

        assert!(matches!(
            render_data_element_properties(&properties),
            Err(ProjectionError::InvalidDataElementField {
                field: "dataTypeInformation.category",
                ..
            })
        ));
    }

    #[test]
    fn renders_predefined_types_and_nonstandard_language_versions() {
        let mut properties = properties();
        let properties_v2 = &mut properties;
        properties_v2.abap_language_version = Some("5".to_owned());
        properties_v2.definition.type_kind = "predefinedAbapType".to_owned();
        properties_v2.definition.type_name = None;
        properties_v2.definition.data_type = Some("DEC".to_owned());
        properties_v2.definition.data_type_length = Some(12);
        properties_v2.definition.data_type_decimals = Some(3);

        let content = render_data_element_properties(&properties).unwrap();
        let document: Value = serde_json::from_str(&content).unwrap();

        assert_eq!(
            document["header"]["abapLanguageVersion"],
            "cloudDevelopment"
        );
        assert_eq!(
            document["dataTypeInformation"]["predefinedType"],
            json!({ "dataType": "DEC", "length": 12, "decimals": 3 })
        );
    }

    #[test]
    fn maps_writing_direction_in_both_directions() {
        let mut properties = properties();
        properties.definition.left_to_right_direction = Some(false);

        let content = render_data_element_properties(&properties).unwrap();
        let document: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            document["additionalProperties"]["bidirectionalOptions"]["basicDirection"],
            "rightToLeft"
        );

        let edited = content.replace("rightToLeft", "leftToRight");
        let merged = merge_data_element_properties(&properties, &edited).unwrap();
        assert_eq!(merged.definition.left_to_right_direction, Some(true));
    }

    #[test]
    fn merges_aff_edits_without_losing_adt_only_properties() {
        let original = properties();
        let original_links = original.links.clone();
        let edited = r#"{
            "formatVersion": "1",
            "header": {
                "description": "Updated",
                "originalLanguage": "de",
                "abapLanguageVersion": "keyUser"
            },
            "dataTypeInformation": {
                "category": "predefinedType",
                "predefinedType": {
                    "dataType": "CHAR",
                    "length": 30
                }
            },
            "fieldLabels": {
                "short": "New",
                "shortLength": 5
            },
            "additionalProperties": {
                "searchHelp": {
                    "name": "Z_SEARCH",
                    "parameter": "VALUE"
                },
                "parameterId": "PID",
                "changeDocumentRelevant": true,
                "noInputHistory": true
            }
        }"#;

        let merged = merge_data_element_properties(&original, edited).unwrap();
        let properties = &merged;
        let definition = &properties.definition;

        assert_eq!(properties.description.as_deref(), Some("Updated"));
        assert_eq!(properties.master_language.as_deref(), Some("DE"));
        assert_eq!(properties.abap_language_version.as_deref(), Some("2"));
        assert_eq!(definition.type_kind, "predefinedAbapType");
        assert_eq!(definition.type_name, None);
        assert_eq!(definition.data_type_length, Some(30));
        assert_eq!(definition.data_type_decimals, Some(0));
        assert_eq!(definition.short_field_label.as_deref(), Some("New"));
        assert_eq!(definition.short_field_length, Some(5));
        assert_eq!(definition.short_field_max_length, Some(10));
        assert_eq!(definition.medium_field_label.as_deref(), Some(""));
        assert_eq!(definition.search_help.as_deref(), Some("Z_SEARCH"));
        assert_eq!(definition.search_help_parameter.as_deref(), Some("VALUE"));
        assert_eq!(definition.set_get_parameter.as_deref(), Some("PID"));
        assert_eq!(definition.change_document, Some(true));
        assert_eq!(definition.deactivate_input_history, Some(true));
        assert_eq!(definition.left_to_right_direction, Some(true));
        assert_eq!(definition.documentation_status.as_deref(), Some("required"));
        assert_eq!(properties.responsible.as_deref(), Some("DEVELOPER"));
        assert_eq!(properties.links, original_links);
    }

    #[test]
    fn an_unedited_aff_file_preserves_the_complete_adt_properties() {
        let original = properties();
        let content = render_data_element_properties(&original).unwrap();

        let merged = merge_data_element_properties(&original, &content).unwrap();

        assert_eq!(merged, original);
    }

    #[test]
    fn an_unedited_aff_file_preserves_sparse_adt_properties() {
        let mut original = properties();
        let definition = &mut original.definition;
        definition.search_help = None;
        definition.search_help_parameter = None;
        definition.set_get_parameter = None;
        definition.default_component_name = None;
        definition.deactivate_input_history = None;
        definition.change_document = None;
        definition.left_to_right_direction = None;
        definition.deactivate_bidi_filtering = None;
        for label in [
            &mut definition.short_field_label,
            &mut definition.medium_field_label,
            &mut definition.long_field_label,
            &mut definition.heading_field_label,
        ] {
            *label = None;
        }
        for length in [
            &mut definition.short_field_length,
            &mut definition.medium_field_length,
            &mut definition.long_field_length,
            &mut definition.heading_field_length,
        ] {
            *length = None;
        }

        let content = render_data_element_properties(&original).unwrap();
        let merged = merge_data_element_properties(&original, &content).unwrap();

        assert_eq!(merged, original);
    }

    #[test]
    fn preserves_unmodeled_adt_only_wire_values() {
        let mut original = properties();
        original.definition.documentation_status = Some("futureStatus".to_owned());
        original.definition.short_field_max_length = Some(9);
        original.definition.medium_field_max_length = Some(19);

        let content = render_data_element_properties(&original).unwrap();
        let merged = merge_data_element_properties(&original, &content).unwrap();

        assert_eq!(
            merged.definition.documentation_status.as_deref(),
            Some("futureStatus")
        );
        assert_eq!(merged.definition.short_field_max_length, Some(9));
        assert_eq!(merged.definition.medium_field_max_length, Some(19));
    }

    #[test]
    fn related_edits_preserve_untouched_sparse_members() {
        let mut original = properties();
        let definition = &mut original.definition;
        definition.type_kind = "predefinedAbapType".to_owned();
        definition.type_name = None;
        definition.data_type = Some("CHAR".to_owned());
        definition.data_type_length = Some(10);
        definition.data_type_decimals = None;
        definition.search_help = Some("Z_OLD".to_owned());
        definition.search_help_parameter = None;
        let content = render_data_element_properties(&original).unwrap();
        let edited = content
            .replace("\"length\": 10", "\"length\": 11")
            .replace("\"name\": \"Z_OLD\"", "\"name\": \"Z_NEW\"");

        let merged = merge_data_element_properties(&original, &edited).unwrap();
        let definition = &merged.definition;

        assert_eq!(definition.data_type_length, Some(11));
        assert_eq!(definition.data_type_decimals, None);
        assert_eq!(definition.search_help.as_deref(), Some("Z_NEW"));
        assert_eq!(definition.search_help_parameter, None);
    }

    #[test]
    fn changing_a_referenced_type_clears_stale_resolved_type_data() {
        let original = properties();
        let content = render_data_element_properties(&original).unwrap();
        let edited = content.replace("Z_EXAMPLE_DOMAIN", "Z_OTHER_DOMAIN");

        let merged = merge_data_element_properties(&original, &edited).unwrap();
        let definition = &merged.definition;

        assert_eq!(definition.type_name.as_deref(), Some("Z_OTHER_DOMAIN"));
        assert_eq!(definition.data_type, None);
        assert_eq!(definition.data_type_length, None);
        assert_eq!(definition.data_type_length_enabled, None);
        assert_eq!(definition.data_type_decimals, None);
        assert_eq!(definition.data_type_decimals_enabled, None);
    }

    #[test]
    fn rejects_documents_outside_the_aff_schema() {
        let properties = properties();
        let invalid_version = render_data_element_properties(&properties)
            .unwrap()
            .replace("\"formatVersion\": \"1\"", "\"formatVersion\": \"2\"");
        assert!(matches!(
            merge_data_element_properties(&properties, &invalid_version),
            Err(ProjectionError::InvalidDataElementField {
                field: "formatVersion",
                ..
            })
        ));

        let unknown = render_data_element_properties(&properties)
            .unwrap()
            .replacen('{', "{\n  \"unknown\": true,", 1);
        assert!(matches!(
            merge_data_element_properties(&properties, &unknown),
            Err(ProjectionError::InvalidDataElementDocument(_))
        ));
    }

    #[test]
    fn accepts_schema_optional_type_information() {
        let original = properties();
        let mut missing_type_name: Value =
            serde_json::from_str(&render_data_element_properties(&original).unwrap()).unwrap();
        missing_type_name["dataTypeInformation"]
            .as_object_mut()
            .unwrap()
            .remove("typeName");
        let merged =
            merge_data_element_properties(&original, &missing_type_name.to_string()).unwrap();
        assert_eq!(merged.definition.type_name, None);

        let mut predefined = properties();
        predefined.definition.type_kind = "predefinedAbapType".to_owned();
        predefined.definition.type_name = None;
        predefined.definition.data_type = Some("CHAR".to_owned());
        predefined.definition.data_type_length = Some(10);
        let mut unexpected_type_name: Value =
            serde_json::from_str(&render_data_element_properties(&predefined).unwrap()).unwrap();
        unexpected_type_name["dataTypeInformation"]["typeName"] = json!("Z_DOMAIN");
        let merged =
            merge_data_element_properties(&predefined, &unexpected_type_name.to_string()).unwrap();
        assert_eq!(merged.definition.type_name.as_deref(), Some("Z_DOMAIN"));

        predefined.definition.type_name = None;
        predefined.definition.data_type = None;
        predefined.definition.data_type_length = None;
        let rendered = render_data_element_properties(&predefined).unwrap();
        let document: Value = serde_json::from_str(&rendered).unwrap();
        assert!(
            document["dataTypeInformation"]
                .get("predefinedType")
                .is_none()
        );

        let mut completed = document;
        completed["dataTypeInformation"]["predefinedType"] =
            json!({ "dataType": "CHAR", "length": 12 });
        let merged = merge_data_element_properties(&predefined, &completed.to_string()).unwrap();
        assert_eq!(merged.definition.data_type.as_deref(), Some("CHAR"));
        assert_eq!(merged.definition.data_type_length, Some(12));
    }
}
