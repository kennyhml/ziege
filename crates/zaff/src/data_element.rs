use serde::{Deserialize, Serialize};
use zadt::{
    DataElement, DataElementDefinition, DataElementFieldLabel, DataElementProperties,
    DataElementPropertiesV2, DataElementTypeKind, ObjectRef, ObjectType,
};

use crate::{ObjectFormat, ProjectionError};

const FORMAT_VERSION: &str = "1";
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

impl From<DataElementTypeKind> for AffDataElementCategory {
    fn from(value: DataElementTypeKind) -> Self {
        match value {
            DataElementTypeKind::Domain => Self::Domain,
            DataElementTypeKind::PredefinedAbapType => Self::PredefinedType,
            DataElementTypeKind::ReferenceToPredefinedAbapType => Self::ReferenceToPredefinedType,
            DataElementTypeKind::ReferenceToDictionaryType => Self::ReferenceDictionaryType,
            DataElementTypeKind::ReferenceToClassOrInterfaceType => {
                Self::ReferenceClassOrInterfaceType
            }
        }
    }
}

impl From<AffDataElementCategory> for DataElementTypeKind {
    fn from(value: AffDataElementCategory) -> Self {
        match value {
            AffDataElementCategory::Domain => Self::Domain,
            AffDataElementCategory::PredefinedType => Self::PredefinedAbapType,
            AffDataElementCategory::ReferenceToPredefinedType => {
                Self::ReferenceToPredefinedAbapType
            }
            AffDataElementCategory::ReferenceDictionaryType => Self::ReferenceToDictionaryType,
            AffDataElementCategory::ReferenceClassOrInterfaceType => {
                Self::ReferenceToClassOrInterfaceType
            }
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
            short: nonempty(definition.short_field_label.text.as_deref()),
            short_length: nonzero(definition.short_field_label.length),
            medium: nonempty(definition.medium_field_label.text.as_deref()),
            medium_length: nonzero(definition.medium_field_label.length),
            long: nonempty(definition.long_field_label.text.as_deref()),
            long_length: nonzero(definition.long_field_label.length),
            heading: nonempty(definition.heading_field_label.text.as_deref()),
            heading_length: nonzero(definition.heading_field_label.length),
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
    let document = document_from_properties(properties.properties())?;
    let mut content = serde_json::to_string_pretty(&document)
        .map_err(ProjectionError::InvalidDataElementDocument)?;
    content.push('\n');
    Ok(content)
}

fn document_from_properties(
    properties: &DataElementPropertiesV2,
) -> Result<AffDataElement, ProjectionError> {
    let definition = &properties.definition;
    let category = AffDataElementCategory::from(definition.type_kind);
    let predefined_type = if category == AffDataElementCategory::PredefinedType {
        Some(AffPredefinedType {
            data_type: required(
                definition.data_type.clone(),
                "dataTypeInformation.predefinedType.dataType",
            )?,
            length: required(
                definition.data_type_length,
                "dataTypeInformation.predefinedType.length",
            )?,
            decimals: definition.data_type_decimals.filter(|value| *value != 0),
        })
    } else {
        None
    };
    let document = AffDataElement {
        format_version: FORMAT_VERSION.to_owned(),
        header: AffDataElementHeader {
            description: required(properties.description.clone(), "header.description")?,
            original_language: required(
                properties.master_language.clone(),
                "header.originalLanguage",
            )?
            .to_ascii_lowercase(),
            abap_language_version: AffAbapLanguageVersion::from_adt(
                properties.abap_language_version.as_deref(),
            )?,
        },
        data_type_information: AffDataElementTypeInformation {
            category,
            type_name: (category != AffDataElementCategory::PredefinedType)
                .then(|| definition.type_name.clone())
                .flatten(),
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
    let original_properties = original.properties();
    let original_document = document_from_properties(original_properties)?;
    let original_language_version = original_properties.abap_language_version.clone();
    let mut definition = original_properties.definition.clone();
    apply_document(&original_document, &document, &mut definition);

    let mut merged = original.clone();
    let properties = merged.properties_mut();
    if document.header.description != original_document.header.description {
        properties.description = Some(document.header.description);
    }
    if document.header.original_language != original_document.header.original_language {
        properties.master_language = Some(document.header.original_language.to_ascii_uppercase());
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

pub(crate) fn validate_data_element_binding(
    object: &ObjectRef<DataElement>,
    properties: &DataElementProperties,
) -> Result<(), ProjectionError> {
    let properties = properties.properties();
    if !properties.name.eq_ignore_ascii_case(object.name())
        || !properties
            .reference
            .name()
            .eq_ignore_ascii_case(object.name())
    {
        return Err(ProjectionError::BindingNameMismatch {
            projected_name: object.name().to_owned(),
            repository_name: properties.name.clone(),
        });
    }
    if properties.object_type != DataElement::WORKBENCH_TYPE {
        return Err(ProjectionError::BindingTypeMismatch {
            projected_type: ObjectFormat::DataElement.object_type(),
            repository_type: properties.object_type.clone(),
        });
    }
    if properties.reference != *object {
        return Err(ProjectionError::BindingResourceMismatch {
            projected_uri: object.uri().to_string(),
            properties_uri: properties.reference.uri().to_string(),
        });
    }
    Ok(())
}

impl AffDataElement {
    fn validate(&self) -> Result<(), ProjectionError> {
        if self.format_version != FORMAT_VERSION {
            return Err(invalid_field(
                "formatVersion",
                format!("expected `{FORMAT_VERSION}`"),
            ));
        }
        max_length("header.description", &self.header.description, 60)?;
        if self.header.original_language.chars().count() < 2 {
            return Err(invalid_field(
                "header.originalLanguage",
                "must contain at least two characters",
            ));
        }
        if let Some(type_name) = &self.data_type_information.type_name {
            max_length("dataTypeInformation.typeName", type_name, 30)?;
        }
        match self.data_type_information.category {
            AffDataElementCategory::PredefinedType => {
                let predefined = self
                    .data_type_information
                    .predefined_type
                    .as_ref()
                    .ok_or_else(|| {
                        invalid_field(
                            "dataTypeInformation.predefinedType",
                            "is required for category `predefinedType`",
                        )
                    })?;
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
            _ if self.data_type_information.predefined_type.is_some() => {
                return Err(invalid_field(
                    "dataTypeInformation.predefinedType",
                    "is only valid for category `predefinedType`",
                ));
            }
            _ => {}
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
        definition.type_kind = edited_type.category.into();
    }
    match (&original_type.predefined_type, &edited_type.predefined_type) {
        (_, Some(predefined))
            if original_type.category != AffDataElementCategory::PredefinedType =>
        {
            definition.type_name = None;
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
        (None, None)
            if edited_type.category != original_type.category
                || edited_type.type_name != original_type.type_name =>
        {
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
        original_labels.and_then(|labels| labels.short.as_deref()),
        original_labels.and_then(|labels| labels.short_length),
        edited_labels.and_then(|labels| labels.short.as_deref()),
        edited_labels.and_then(|labels| labels.short_length),
    );
    apply_label(
        &mut definition.medium_field_label,
        original_labels.and_then(|labels| labels.medium.as_deref()),
        original_labels.and_then(|labels| labels.medium_length),
        edited_labels.and_then(|labels| labels.medium.as_deref()),
        edited_labels.and_then(|labels| labels.medium_length),
    );
    apply_label(
        &mut definition.long_field_label,
        original_labels.and_then(|labels| labels.long.as_deref()),
        original_labels.and_then(|labels| labels.long_length),
        edited_labels.and_then(|labels| labels.long.as_deref()),
        edited_labels.and_then(|labels| labels.long_length),
    );
    apply_label(
        &mut definition.heading_field_label,
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
    label: &mut DataElementFieldLabel,
    original_text: Option<&str>,
    original_length: Option<u32>,
    edited_text: Option<&str>,
    edited_length: Option<u32>,
) {
    if edited_text != original_text {
        label.text = Some(edited_text.unwrap_or_default().to_owned());
    }
    if edited_length != original_length {
        label.length = Some(edited_length.unwrap_or(0));
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
    use http::{HeaderMap, StatusCode};
    use serde_json::{Value, json};
    use zadt::{
        AdtResponse, AdtUri, DataElementDocumentationStatus, ObjectPropertiesQuery, Operation,
        OperationResponse, Ready, RepositoryContentQuery,
    };

    use super::*;

    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../zadt/tests/fixtures/data-element-ztfrwtfrt-v2.xml");

    fn reference(name: &str, uri: &str) -> ObjectRef<DataElement> {
        let response = AdtResponse::new(
            StatusCode::OK,
            HeaderMap::new(),
            format!(
                r#"<vfs:virtualFoldersResult xmlns:vfs="http://www.sap.com/adt/ris/virtualFolders" objectCount="1">
                    <vfs:object name="{name}" package="$TMP" type="DTEL/DE"
                        uri="{uri}" expandable="false" />
                </vfs:virtualFoldersResult>"#
            )
            .into_bytes(),
        );
        let target =
            AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents")
                .unwrap();
        let mut content = <RepositoryContentQuery as Operation<Ready>>::decode(
            &RepositoryContentQuery::new(),
            OperationResponse::new(response, target),
        )
        .unwrap();
        content
            .objects
            .pop()
            .unwrap()
            .typed_reference::<DataElement>()
            .unwrap()
    }

    fn properties() -> DataElementProperties {
        let reference = reference("ZTFRWTFRT", "/sap/bc/adt/ddic/dataelements/ztfrwtfrt");
        let query = reference.query();
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            "application/vnd.sap.adt.dataelements.v2+xml"
                .parse()
                .unwrap(),
        );
        headers.insert(http::header::ETAG, "data-element-etag".parse().unwrap());
        let response = AdtResponse::new(StatusCode::OK, headers, DATA_ELEMENT_XML.to_vec());
        let mut properties = <ObjectPropertiesQuery<DataElement> as Operation<Ready>>::decode(
            &query,
            OperationResponse::new(response, reference.uri().clone()),
        )
        .unwrap();
        let properties_v2 = properties.properties_mut();
        properties_v2.description = Some("Example data element".to_owned());
        properties_v2.master_language = Some("EN".to_owned());
        properties_v2.abap_language_version = Some("0".to_owned());
        properties_v2.definition.type_kind = DataElementTypeKind::Domain;
        properties_v2.definition.type_name = Some("Z_EXAMPLE_DOMAIN".to_owned());
        properties_v2.definition.data_type = Some("CHAR".to_owned());
        properties_v2.definition.data_type_length = Some(8);
        properties_v2.definition.data_type_decimals = Some(0);
        properties_v2.definition.short_field_label.text = Some("Example".to_owned());
        properties_v2.definition.short_field_label.length = Some(10);
        properties_v2.definition.medium_field_label.text = Some("Example field".to_owned());
        properties_v2.definition.medium_field_label.length = Some(13);
        properties_v2.definition.long_field_label.text =
            Some("Example data element field".to_owned());
        properties_v2.definition.long_field_label.length = Some(26);
        properties_v2.definition.heading_field_label.text = Some("Example data element".to_owned());
        properties_v2.definition.heading_field_label.length = Some(20);
        properties_v2.definition.left_to_right_direction = Some(true);
        properties_v2.definition.documentation_status =
            Some(DataElementDocumentationStatus::Required);
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
    fn translates_every_adt_type_kind_to_the_aff_schema_name() {
        for (adt, aff) in [
            (DataElementTypeKind::Domain, "domain"),
            (DataElementTypeKind::PredefinedAbapType, "predefinedType"),
            (
                DataElementTypeKind::ReferenceToPredefinedAbapType,
                "referenceToPredefinedType",
            ),
            (
                DataElementTypeKind::ReferenceToDictionaryType,
                "referenceDictionaryType",
            ),
            (
                DataElementTypeKind::ReferenceToClassOrInterfaceType,
                "referenceClasIntType",
            ),
        ] {
            let mut properties = properties();
            properties.properties_mut().definition.type_kind = adt;
            if adt == DataElementTypeKind::PredefinedAbapType {
                properties.properties_mut().definition.data_type = Some("CHAR".to_owned());
                properties.properties_mut().definition.data_type_length = Some(12);
            }

            let content = render_data_element_properties(&properties).unwrap();
            let document: Value = serde_json::from_str(&content).unwrap();

            assert_eq!(document["dataTypeInformation"]["category"], aff);
        }
    }

    #[test]
    fn renders_predefined_types_and_nonstandard_language_versions() {
        let mut properties = properties();
        let properties_v2 = properties.properties_mut();
        properties_v2.abap_language_version = Some("5".to_owned());
        properties_v2.definition.type_kind = DataElementTypeKind::PredefinedAbapType;
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
        properties
            .properties_mut()
            .definition
            .left_to_right_direction = Some(false);

        let content = render_data_element_properties(&properties).unwrap();
        let document: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            document["additionalProperties"]["bidirectionalOptions"]["basicDirection"],
            "rightToLeft"
        );

        let edited = content.replace("rightToLeft", "leftToRight");
        let merged = merge_data_element_properties(&properties, &edited).unwrap();
        assert_eq!(
            merged.properties().definition.left_to_right_direction,
            Some(true)
        );
    }

    #[test]
    fn merges_aff_edits_without_losing_adt_only_properties() {
        let original = properties();
        let original_relations = original.properties().relations().clone();
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
        let properties = merged.properties();
        let definition = &properties.definition;

        assert_eq!(properties.description.as_deref(), Some("Updated"));
        assert_eq!(properties.master_language.as_deref(), Some("DE"));
        assert_eq!(properties.abap_language_version.as_deref(), Some("2"));
        assert_eq!(
            definition.type_kind,
            DataElementTypeKind::PredefinedAbapType
        );
        assert_eq!(definition.type_name, None);
        assert_eq!(definition.data_type_length, Some(30));
        assert_eq!(definition.data_type_decimals, Some(0));
        assert_eq!(definition.short_field_label.text.as_deref(), Some("New"));
        assert_eq!(definition.short_field_label.length, Some(5));
        assert_eq!(definition.short_field_label.max_length, Some(10));
        assert_eq!(definition.medium_field_label.text.as_deref(), Some(""));
        assert_eq!(definition.search_help.as_deref(), Some("Z_SEARCH"));
        assert_eq!(definition.search_help_parameter.as_deref(), Some("VALUE"));
        assert_eq!(definition.set_get_parameter.as_deref(), Some("PID"));
        assert_eq!(definition.change_document, Some(true));
        assert_eq!(definition.deactivate_input_history, Some(true));
        assert_eq!(definition.left_to_right_direction, Some(true));
        assert_eq!(
            definition.documentation_status,
            Some(DataElementDocumentationStatus::Required)
        );
        assert_eq!(properties.responsible.as_deref(), Some("DEVELOPER"));
        assert_eq!(properties.relations(), &original_relations);
        assert_eq!(properties.etag.as_deref(), Some("data-element-etag"));
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
        let definition = &mut original.properties_mut().definition;
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
            label.text = None;
            label.length = None;
        }

        let content = render_data_element_properties(&original).unwrap();
        let merged = merge_data_element_properties(&original, &content).unwrap();

        assert_eq!(merged, original);
    }

    #[test]
    fn rejects_properties_for_another_bound_object() {
        let object = properties().properties().reference.clone();
        let mut properties = properties();
        validate_data_element_binding(&object, &properties).unwrap();

        properties.properties_mut().reference =
            reference("ZTFRWTFRT", "/sap/bc/adt/ddic/dataelements/zother");

        assert!(matches!(
            validate_data_element_binding(&object, &properties),
            Err(ProjectionError::BindingResourceMismatch { .. })
        ));
    }

    #[test]
    fn related_edits_preserve_untouched_sparse_members() {
        let mut original = properties();
        let definition = &mut original.properties_mut().definition;
        definition.type_kind = DataElementTypeKind::PredefinedAbapType;
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
        let definition = &merged.properties().definition;

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
        let definition = &merged.properties().definition;

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
}
