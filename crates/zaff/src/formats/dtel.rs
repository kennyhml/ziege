//! Data Element mapping between ADT properties and an AFF `.dtel.json` document.
//!
//! Because of how different the structure of the data presentations is between
//! ZADT and ZAFF, significintly more work needs to be done than for other objects
//! and much more caution must be taken to properly invalidate stale data.
//!
//! # How To Read This Module
//!
//! AFF organizes a Data Element into small, named JSON blocks. ADT stores the same
//! information in a different shape: general metadata lives on
//! [`DataElementProperties`], while most Dictionary fields live in its
//! [`definition`](DataElementProperties::definition).
//!
//! Each AFF model below documents the individual fields it reads and writes.
//! The tables use JSON names for AFF and Rust field names for ADT. For example,
//! `header.originalLanguage` is an AFF JSON path, while `master_language` is a
//! field on the ADT properties. These are not ADT XML attribute names.

use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{DataElement, DataElementDefinition, DataElementProperties, ObjectSnapshot, ObjectType};

use crate::{
    AbapLanguageVersion, Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::{is_false, nonempty, nonzero, required},
    models::{language_from_adt, language_to_adt},
    validate::one_of,
};

pub(crate) static DATA_ELEMENT_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DTEL",
    version: "1",
    workbench_types: &[DataElement::WORKBENCH_TYPE],
    files: &[FileSpec::new(
        "<name>.dtel.json",
        Cardinality::One,
        Mapping::Properties(PropertiesMapping { render, merge }),
    )],
};

/// Renders ADT [`DataElementProperties`] as pretty-printed AFF JSON with a trailing newline.
fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<DataElement>()?;
    let document = ProjectedDataElementProperties::from_adt(properties)?;
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');
    Ok(content)
}

/// Converts an edited AFF document into an optional ADT update payload.
///
/// Maps `header` to top-level ADT properties and the remaining blocks to
/// `definition` through [`apply_definitions`]. Returns `None` for an unchanged result.
fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<DataElement>()?;

    let edited: ProjectedDataElementProperties = serde_json::from_str(edited)?;
    edited.validate()?;
    let language = language_to_adt(&edited.header.original_language, "header.originalLanguage")?;

    // Reject an editable predefinedType block on a domain or reference category.
    //
    // AFFs schema permits these combinations, but this mapping only writes a
    // predefinedType block for a directly predefined Data Element. For example,
    // a domain can supply resolved datatype information in ADT. Accepting that
    // information as an AFF edit would confuse the domain type with a type
    // defined directly on this Data Element.
    if edited.data_type_information.predefined_type.is_some()
        && edited.data_type_information.category != DataElementCategory::PredefinedType
    {
        return Err(ProjectionError::InvalidDataElementField {
            field: "dataTypeInformation.predefinedType",
            message: "mapping is supported only for category `predefinedType`".to_owned(),
        });
    }

    let previous = ProjectedDataElementProperties::from_adt(original)?;
    let mut merged = original.clone();

    // Update the Dictionary definition: type information, labels, and additional
    // properties. General header metadata is not stored in this nested object.
    apply_definitions(&previous, &edited, &mut merged.definition);

    // Header metadata is stored directly on DataElementProperties:
    //
    //   AFF header.description       -> ADT description
    //   AFF header.originalLanguage  -> ADT master_language
    //
    // The language was converted above, for example from AFF "en" to ADT "EN".
    merged.description = Some(edited.header.description);
    merged.master_language = Some(language);

    // AFF header.abapLanguageVersion maps to ADT abap_language_version.
    // Standard is written as "0" for DTEL, but an unchanged Standard value must
    // retain the original spelling, which may also have been absent or blank.
    if edited.header.abap_language_version != previous.header.abap_language_version {
        merged.abap_language_version = Some(edited.header.abap_language_version.to_adt_ddic());
    }

    if merged == *original {
        return Ok(None);
    }
    serde_json::to_value(merged).map(Some).map_err(Into::into)
}

/// Dictionary Data Element properties represented by the AFF v1 JSON document.
///
/// # Document Layout
///
/// ```text
/// AFF block             ADT storage
/// ---------             -----------
/// formatVersion         No backing field, this format supplies "1"
/// header                DataElementProperties (top-level fields)
/// dataTypeInformation   DataElementProperties.definition
/// fieldLabels           DataElementProperties.definition
/// additionalProperties  DataElementProperties.definition
/// ```
///
/// `header` contains the description and language settings. `dataTypeInformation`
/// contains the category, referenced name, or predefined type. `fieldLabels`
/// contains the short, medium, long, and heading labels with their lengths.
/// `additionalProperties` contains search help, parameter ID, flags, and text direction.
///
/// See the model for each block for its field-by-field mapping:
/// [`DataElementHeader`], [`DataElementTypeInformation`],
/// [`DataElementFieldLabels`], and [`DataElementAdditionalProperties`].
///
/// ADT identity, package references, links, users, timestamps, and other
/// unrepresented properties remain on the original snapshot. Merge preserves
/// them by copying the snapshot properties before applying edits.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedDataElementProperties {
    #[garde(custom(one_of([DATA_ELEMENT_FORMAT.version()])))]
    pub format_version: String,

    #[garde(dive)]
    pub header: DataElementHeader,

    #[garde(dive)]
    pub data_type_information: DataElementTypeInformation,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub field_labels: Option<DataElementFieldLabels>,

    #[serde(
        default,
        skip_serializing_if = "DataElementAdditionalProperties::is_empty"
    )]
    #[garde(dive)]
    pub additional_properties: DataElementAdditionalProperties,
}

/// AFF Data Element header fields.
///
/// # Field Mapping
///
/// These ADT fields are on `DataElementProperties`, not on its `definition`.
///
/// ```text
/// AFF field                   ADT field
/// ---------                   ---------
/// header.description          description
/// header.originalLanguage     master_language
/// header.abapLanguageVersion  abap_language_version
/// ```
///
/// Description text is copied. An absent ADT description is an error.
/// Original language is converted from a SAP code to BCP47 on render, and back
/// on merge, for example `EN` and `en`. Language version uses the mapping below.
///
/// # Language Versions
///
/// ```text
/// AFF value         Accepted ADT value on render  ADT value written for an edit
/// ---------         ----------------------------  -----------------------------
/// standard          Absent, "", " ", or "0"       "0"
/// keyUser           "2"                           "2"
/// cloudDevelopment  "5"                           "5"
/// ```
///
/// Standard is omitted from rendered AFF JSON. If the edited document still
/// means Standard, merge keeps the original ADT value rather than normalizing
/// an absent or blank value to `"0"`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct DataElementHeader {
    #[garde(length(chars, max = 60))]
    pub description: String,

    pub original_language: String,

    #[serde(default, skip_serializing_if = "AbapLanguageVersion::is_standard")]
    pub abap_language_version: AbapLanguageVersion,
}

/// AFF Data Element type information.
///
/// # Field Mapping
///
/// ```text
/// AFF field                           ADT field
/// ---------                           ---------
/// dataTypeInformation.category        definition.type_kind
/// dataTypeInformation.typeName        definition.type_name
/// dataTypeInformation.predefinedType  See the mapping on PredefinedType
/// ```
///
/// Category selects how the type is defined. See [`DataElementCategory`].
/// Type name is the stored name, for example the name of a referenced domain.
/// The predefined-type block contains datatype, length, and decimals. See
/// [`PredefinedType`] for the individual ADT fields.
///
/// The predefined-type block is only rendered for the `predefinedType` category,
/// and only when ADT supplies both the datatype and its length. This mapping
/// rejects edits that attach that block to another category.
///
/// # Effects Of Type Edits
///
/// A reference change can make the old resolved datatype information stale.
/// For example, changing the domain name must not keep the datatype and length
/// resolved from the old domain. Merge clears that old type information and
/// its enablement flags when the reference changes outside `predefinedType`.
///
/// Changing category or removing a previously visible predefined-type block
/// also clears stale type information. An incomplete predefined type that was
/// not edited is left intact in the ADT properties, even though AFF omitted it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct DataElementTypeInformation {
    pub category: DataElementCategory,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 30))]
    pub type_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub predefined_type: Option<PredefinedType>,
}

/// AFF category names, which intentionally differ from some ADT wire values.
///
/// ```text
/// AFF dataTypeInformation.category  ADT definition.type_kind
/// --------------------------------  ------------------------
/// domain                            domain
/// predefinedType                    predefinedAbapType
/// referenceToPredefinedType         refToPredefinedAbapType
/// referenceDictionaryType           refToDictionaryType
/// referenceClasIntType              refToClifType
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DataElementCategory {
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

impl DataElementCategory {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "domain" => Ok(Self::Domain),
            "predefinedAbapType" => Ok(Self::PredefinedType),
            "refToPredefinedAbapType" => Ok(Self::ReferenceToPredefinedType),
            "refToDictionaryType" => Ok(Self::ReferenceDictionaryType),
            "refToClifType" => Ok(Self::ReferenceClassOrInterfaceType),
            value => Err(ProjectionError::InvalidDataElementField {
                field: "dataTypeInformation.category",
                message: format!("unsupported ADT type kind `{value}`"),
            }),
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
///
/// # Field Mapping
///
/// ```text
/// AFF field                                    ADT field
/// ---------                                    ---------
/// dataTypeInformation.predefinedType.dataType  definition.data_type
/// dataTypeInformation.predefinedType.length    definition.data_type_length
/// dataTypeInformation.predefinedType.decimals  definition.data_type_decimals
/// ```
///
/// # Rendering
///
/// Both datatype and length must be present to render this block. Decimals
/// are omitted when ADT stores either no value or zero. ADT may also contain
/// these fields as resolved information for a domain or reference. This mapper
/// does not expose that information as an editable predefined-type block.
///
/// # Merging
///
/// A newly added block writes datatype and length, and writes zero decimals
/// if the edited block omits them. For an existing block, unchanged omitted
/// decimals preserve the ADT distinction between `None` and `Some(0)`.
///
/// The ADT fields `definition.data_type_length_enabled` and
/// `definition.data_type_decimals_enabled` are not AFF fields. Merge clears
/// them when a block is added or its datatype changes. Editing only length or
/// decimals retains these flags. Removing the block clears both the type data
/// and the flags.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredefinedType {
    #[garde(custom(one_of(DATA_TYPES)))]
    pub data_type: String,

    #[garde(range(max = 999_999))]
    pub length: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(range(max = 999_999))]
    pub decimals: Option<u32>,
}

/// AFF field labels and their configured output lengths.
///
/// # Field Mapping
///
/// ```text
/// AFF field                  ADT field
/// ---------                  ---------
/// fieldLabels.short          definition.short_field_label
/// fieldLabels.shortLength    definition.short_field_length
/// fieldLabels.medium         definition.medium_field_label
/// fieldLabels.mediumLength   definition.medium_field_length
/// fieldLabels.long           definition.long_field_label
/// fieldLabels.longLength     definition.long_field_length
/// fieldLabels.heading        definition.heading_field_label
/// fieldLabels.headingLength  definition.heading_field_length
/// ```
///
/// # Empty Values And Edits
///
/// Rendering omits empty labels and zero lengths. If no members remain, it
/// omits the entire `fieldLabels` block.
///
/// Merge compares each label and each length independently. Removing a visible
/// label writes `Some("")`. Removing a visible length writes `Some(0)`.
/// An unchanged omitted member retains its original ADT representation.
///
/// For example, if ADT has no `short_field_length`, AFF omits `shortLength`.
/// Editing only `fieldLabels.short` leaves that ADT length absent, rather than
/// replacing it with zero.
///
/// The ADT maximum-length fields are separate and never edited through AFF:
/// `short_field_max_length`, `medium_field_max_length`, `long_field_max_length`,
/// and `heading_field_max_length` on `definition`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataElementFieldLabels {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 10))]
    pub short: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(range(max = 10))]
    pub short_length: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 20))]
    pub medium: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(range(max = 20))]
    pub medium_length: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 40))]
    pub long: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(range(max = 40))]
    pub long_length: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 55))]
    pub heading: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(range(max = 55))]
    pub heading_length: Option<u32>,
}

impl DataElementFieldLabels {
    fn from_definition(definition: &DataElementDefinition) -> Option<Self> {
        // Build each label and length independently. Empty text and zero lengths
        // become omitted AFF members. The original ADT fields remain untouched.
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

/// Additional Data Element properties beyond the type and labels.
///
/// # Field Mapping
///
/// All AFF paths below are inside `additionalProperties`.
///
/// ```text
/// AFF field                            ADT field
/// ---------                            ---------
/// searchHelp.name                      definition.search_help
/// searchHelp.parameter                 definition.search_help_parameter
/// parameterId                          definition.set_get_parameter
/// defaultComponentName                 definition.default_component_name
/// changeDocumentRelevant               definition.change_document
/// noInputHistory                       definition.deactivate_input_history
/// bidirectionalOptions.basicDirection  definition.left_to_right_direction
/// bidirectionalOptions.noFiltering     definition.deactivate_bidi_filtering
/// ```
///
/// # Defaults
///
/// Empty parameter IDs and default component names are omitted on render.
/// Removing a visible value on merge writes an empty ADT string. If the AFF
/// value has not changed, the original ADT absence or empty string is preserved.
///
/// `changeDocumentRelevant` and `noInputHistory` default to false. Both an absent
/// ADT flag and an explicit false flag render as that default. Merge writes a
/// boolean only when the corresponding AFF value changes.
///
/// Search-help members and text-direction settings have their own rules.
/// See [`SearchHelp`] and [`BidirectionalOptions`]. When all settings are at
/// their defaults, rendering omits the whole `additionalProperties` block.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct DataElementAdditionalProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub search_help: Option<SearchHelp>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidirectional_options: Option<BidirectionalOptions>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 20))]
    pub parameter_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(length(chars, max = 30))]
    pub default_component_name: Option<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub change_document_relevant: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub no_input_history: bool,
}

impl DataElementAdditionalProperties {
    fn from_definition(definition: &DataElementDefinition) -> Self {
        // Render the search-help assignment.
        //
        // ADT stores the name and parameter separately, and either can be absent.
        // AFF requires both strings once the searchHelp block is present. If at
        // least one ADT member is nonempty, fill the missing AFF member with "".
        let search_help_name = nonempty(definition.search_help.as_deref());
        let search_help_parameter = nonempty(definition.search_help_parameter.as_deref());
        let search_help =
            (search_help_name.is_some() || search_help_parameter.is_some()).then(|| SearchHelp {
                name: search_help_name.unwrap_or_default(),
                parameter: search_help_parameter.unwrap_or_default(),
            });

        // Render the writing direction.
        // ADT false means rightToLeft. ADT true or absent means leftToRight.
        let basic_direction = match definition.left_to_right_direction {
            Some(false) => BasicDirection::RightToLeft,
            Some(true) | None => BasicDirection::LeftToRight,
        };

        // Render the filtering flag. Both names describe disabling filtering,
        // so noFiltering uses the ADT boolean directly, without inversion.
        let no_filtering = definition.deactivate_bidi_filtering.unwrap_or(false);
        let bidirectional_options = (basic_direction != BasicDirection::LeftToRight
            || no_filtering)
            .then_some(BidirectionalOptions {
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
///
/// `additionalProperties.searchHelp.name` maps to `definition.search_help`.
/// `additionalProperties.searchHelp.parameter` maps to
/// `definition.search_help_parameter`.
///
/// ADT stores these as independent optional strings. AFF requires both strings
/// inside a present block, so rendering fills a missing member with `""` when
/// the other member is nonempty. If both are absent or empty, the block is omitted.
///
/// Merge compares the two members independently. For example, if ADT stores
/// name `Z_SEARCH` and no parameter, AFF renders name `Z_SEARCH` and parameter
/// `""`. Changing only the name keeps the ADT parameter absent.
///
/// Removing the whole visible block clears both members to empty ADT strings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct SearchHelp {
    #[garde(length(chars, max = 30))]
    pub name: String,

    #[garde(length(chars, max = 30))]
    pub parameter: String,
}

/// AFF bidirectional text options.
///
/// # Direction
///
/// `additionalProperties.bidirectionalOptions.basicDirection` maps to the ADT
/// field `definition.left_to_right_direction`:
///
/// ```text
/// ADT value    Rendered AFF meaning
/// ---------    --------------------
/// Some(true)   leftToRight
/// None         leftToRight
/// Some(false)  rightToLeft
/// ```
///
/// `leftToRight` is the AFF default and is omitted from the JSON. A changed
/// direction writes `Some(true)` or `Some(false)`. An unchanged default keeps
/// the original ADT `None` or `Some(true)`.
///
/// # Filtering
///
/// `additionalProperties.bidirectionalOptions.noFiltering` maps directly to
/// `definition.deactivate_bidi_filtering`: true disables filtering in both
/// representations. An absent ADT flag renders as false, the AFF default.
///
/// The block is omitted when direction is left-to-right and filtering is not
/// disabled. Removing a visible block resets its nondefault settings, but
/// does not rewrite settings that already had their default AFF meaning.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BidirectionalOptions {
    #[serde(default, skip_serializing_if = "BasicDirection::is_left_to_right")]
    pub basic_direction: BasicDirection,

    #[serde(default, skip_serializing_if = "is_false")]
    pub no_filtering: bool,
}

/// The basic writing direction exposed by AFF.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum BasicDirection {
    #[default]
    #[serde(rename = "leftToRight")]
    LeftToRight,
    #[serde(rename = "rightToLeft")]
    RightToLeft,
}

impl BasicDirection {
    const fn is_left_to_right(&self) -> bool {
        matches!(self, Self::LeftToRight)
    }
}

impl ProjectedDataElementProperties {
    fn from_adt(properties: &DataElementProperties) -> Result<Self, ProjectionError> {
        let definition = &properties.definition;
        // First translate the ADT type category. The spelling is not always the
        // same in AFF: for example, predefinedAbapType becomes predefinedType.
        let category = DataElementCategory::from_adt(&definition.type_kind)?;

        // Build the predefinedType block only for a directly predefined type.
        //
        // The block needs both a datatype and a length. If either is missing,
        // leave the block out rather than inventing a value. That incomplete
        // ADT information still exists in the retained snapshot.
        //
        // A domain or reference can also have datatype information in ADT,
        // but that resolved information is not exposed as an editable block.
        let predefined_type = match (
            category,
            definition.data_type.as_ref(),
            definition.data_type_length,
        ) {
            (DataElementCategory::PredefinedType, Some(data_type), Some(length)) => {
                Some(PredefinedType {
                    data_type: data_type.clone(),
                    length,
                    decimals: definition.data_type_decimals.filter(|value| *value != 0),
                })
            }
            _ => None,
        };
        let document = Self {
            format_version: DATA_ELEMENT_FORMAT.version().to_owned(),
            // Read the header from the top-level ADT properties. Unlike the
            // optional ADT fields, AFF requires a description and language.
            header: DataElementHeader {
                description: required(properties.description.clone(), "header.description")?,
                original_language: language_from_adt(
                    required(
                        properties.master_language.as_deref(),
                        "header.originalLanguage",
                    )?,
                    "header.originalLanguage",
                )?,
                abap_language_version: AbapLanguageVersion::from_adt(
                    properties.abap_language_version.as_ref(),
                    "0",
                )
                .map_err(|value| ProjectionError::InvalidDataElementField {
                    field: "header.abapLanguageVersion",
                    message: format!("unsupported ADT value `{value}`"),
                })?,
            },
            data_type_information: DataElementTypeInformation {
                category,
                type_name: definition.type_name.clone(),
                predefined_type,
            },
            field_labels: DataElementFieldLabels::from_definition(definition),
            additional_properties: DataElementAdditionalProperties::from_definition(definition),
        };
        document.validate()?;
        Ok(document)
    }
}

/// Maps AFF `dataTypeInformation`, `fieldLabels`, and `additionalProperties`
/// to ADT [`DataElementDefinition`]. Header fields are handled by [`merge`].
fn apply_definitions(
    original: &ProjectedDataElementProperties,
    edited: &ProjectedDataElementProperties,
    definition: &mut DataElementDefinition,
) {
    // Type category and referenced name
    //
    //   AFF dataTypeInformation.category  -> ADT definition.type_kind
    //   AFF dataTypeInformation.typeName  -> ADT definition.type_name
    //
    // The category uses translated spellings. The optional name is copied.
    let original_type = &original.data_type_information;
    let edited_type = &edited.data_type_information;
    definition.type_kind = edited_type.category.adt_value().to_owned();
    definition.type_name = edited_type.type_name.clone();

    // When the edited category is not predefinedType, a changed name is a
    // changed reference. For example, switching from domain Z_OLD to Z_NEW
    // invalidates the datatype and length resolved from Z_OLD. Clear that data
    // and its enablement flags rather than carrying it into the new reference.
    if edited_type.type_name != original_type.type_name
        && edited_type.category != DataElementCategory::PredefinedType
    {
        definition.data_type = None;
        definition.data_type_length = None;
        definition.data_type_decimals = None;
        definition.data_type_length_enabled = None;
        definition.data_type_decimals_enabled = None;
    }

    // Predefined type
    //
    // Within AFF dataTypeInformation.predefinedType:
    //
    //   dataType  -> ADT definition.data_type
    //   length    -> ADT definition.data_type_length
    //   decimals  -> ADT definition.data_type_decimals
    //
    // The two ADT enablement flags have no AFF fields. They must be preserved
    // or cleared according to the kind of edit, not copied from the document.
    match (&original_type.predefined_type, &edited_type.predefined_type) {
        (None, Some(predefined)) => {
            // The original AFF view had no block. The edited document adds one.
            // This supplies a complete datatype and length. Missing decimals
            // now mean zero, and old enablement flags no longer describe this type.
            definition.data_type = Some(predefined.data_type.clone());
            definition.data_type_length = Some(predefined.length);
            definition.data_type_decimals = Some(predefined.decimals.unwrap_or(0));
            definition.data_type_length_enabled = None;
            definition.data_type_decimals_enabled = None;
        }
        (Some(original_predefined), Some(edited_predefined)) => {
            // Both documents have a block. Changing CHAR to DEC, for example,
            // invalidates the old enablement flags. Changing only the length
            // or decimals does not, so those edits keep the original flags.
            if edited_predefined.data_type != original_predefined.data_type {
                definition.data_type_length_enabled = None;
                definition.data_type_decimals_enabled = None;
            }
            definition.data_type = Some(edited_predefined.data_type.clone());
            definition.data_type_length = Some(edited_predefined.length);

            // ADT None and Some(0) both render with decimals omitted. If the AFF
            // value is unchanged, keep whichever representation ADT supplied.
            // Removing a previously visible decimals value explicitly writes zero.
            if edited_predefined.decimals != original_predefined.decimals {
                definition.data_type_decimals = Some(edited_predefined.decimals.unwrap_or(0));
            }
        }
        (_, None)
            if original_type.predefined_type.is_some()
                || edited_type.category != original_type.category =>
        {
            // The edited document has no block, and either removed the original
            // block or changed category. The old type data is no longer valid.
            definition.data_type = None;
            definition.data_type_length = None;
            definition.data_type_decimals = None;
            definition.data_type_length_enabled = None;
            definition.data_type_decimals_enabled = None;
        }
        // Neither document has a block and the category is unchanged. The ADT
        // snapshot may still contain incomplete type data, so leave it untouched.
        _ => {}
    }

    // Field labels
    //
    // Each call updates one label and its configured length independently.
    // For example, fieldLabels.short writes definition.short_field_label,
    // while fieldLabels.shortLength writes definition.short_field_length.
    //
    // Removing text clears it to an empty string. Removing a length clears it
    // to zero. Unchanged omitted values keep their original ADT representation.
    // The separate ADT maximum-length fields are not modified.
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

    // Search-help assignment
    //
    //   AFF additionalProperties.searchHelp.name
    //       -> ADT definition.search_help
    //   AFF additionalProperties.searchHelp.parameter
    //       -> ADT definition.search_help_parameter
    //
    // AFF fills in an empty sibling when only one ADT member exists. Compare
    // members independently so changing the name does not also turn an absent
    // ADT parameter into an explicitly stored empty string.
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

    // Parameter ID and default component name
    //
    //   AFF additionalProperties.parameterId
    //       -> ADT definition.set_get_parameter
    //   AFF additionalProperties.defaultComponentName
    //       -> ADT definition.default_component_name
    //
    // Removing a visible AFF value writes an empty string. Leaving an omitted
    // value unchanged preserves the original ADT None or empty string.
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

    // Change-document and input-history flags
    //
    //   AFF additionalProperties.changeDocumentRelevant
    //       -> ADT definition.change_document
    //   AFF additionalProperties.noInputHistory
    //       -> ADT definition.deactivate_input_history
    //
    // These booleans are not inverted. In particular, noInputHistory = true
    // means deactivate_input_history = true. An unchanged false AFF value
    // preserves whether the ADT flag was absent or explicitly false.
    if edited_additional.change_document_relevant != original_additional.change_document_relevant {
        definition.change_document = Some(edited_additional.change_document_relevant);
    }
    if edited_additional.no_input_history != original_additional.no_input_history {
        definition.deactivate_input_history = Some(edited_additional.no_input_history);
    }

    // Writing direction and bidirectional filtering
    //
    // AFF additionalProperties.bidirectionalOptions.basicDirection maps to
    // ADT definition.left_to_right_direction:
    //
    //   leftToRight  -> true
    //   rightToLeft  -> false
    //
    // An omitted AFF block has the default meaning leftToRight. Only an actual
    // direction change writes a boolean. An unchanged default preserves ADT None.
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
        definition.left_to_right_direction = Some(edited_direction == BasicDirection::LeftToRight);
    }

    // AFF additionalProperties.bidirectionalOptions.noFiltering maps directly
    // to ADT definition.deactivate_bidi_filtering. Both mean "disable filtering".
    // As with the other flags, write only when the AFF boolean changes.
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

/// Updates one ADT label and its configured length from the corresponding AFF pair.
///
/// Removed text maps to `Some("")`. Removed length maps to `Some(0)`.
/// Unchanged members retain their original ADT values.
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

pub(super) const DATA_TYPES: &[&str] = &[
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use zadt::{
        AbapLanguageVersion as AdtAbapLanguageVersion, DataElement, DataElementProperties,
        ObjectSnapshot, ToXml,
    };

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
            DataElement::MEDIA_TYPES[0],
            "data-element-etag",
            DATA_ELEMENT_XML,
        )
        .properties()
        .clone();
        let properties_v2 = &mut properties;
        properties_v2.description = Some("Example data element".to_owned());
        properties_v2.master_language = Some("EN".to_owned());
        properties_v2.abap_language_version = Some(AdtAbapLanguageVersion::Other("0".to_owned()));
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

    fn snapshot(properties: &DataElementProperties) -> ObjectSnapshot<()> {
        let reference = crate::test_support::reference::<DataElement>(
            "ZTFRWTFRT",
            "/sap/bc/adt/ddic/dataelements/ztfrwtfrt",
        );
        crate::test_support::properties(
            &reference,
            DataElement::MEDIA_TYPES[0],
            "data-element-etag",
            &properties.to_xml().unwrap(),
        )
        .into_erased()
    }

    #[test]
    fn renders_domain_properties_as_aff_v1() {
        let content = render(&snapshot(&properties())).unwrap();
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
        let content = render(&snapshot(&original)).unwrap();
        let edited = content.replacen(
            "\"description\": \"Example data element\"",
            "\"description\": \"Updated data element\"",
            1,
        );
        let merged: DataElementProperties =
            serde_json::from_value(merge(&snapshot(&original), &edited).unwrap().unwrap()).unwrap();

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

            let content = render(&snapshot(&properties)).unwrap();
            let document: Value = serde_json::from_str(&content).unwrap();

            assert_eq!(document["dataTypeInformation"]["category"], aff);
        }
    }

    #[test]
    fn rejects_unmodeled_adt_type_kinds() {
        let mut properties = properties();
        properties.definition.type_kind = "futureTypeKind".to_owned();

        assert!(matches!(
            render(&snapshot(&properties)),
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
        properties_v2.abap_language_version = Some(AdtAbapLanguageVersion::CloudDevelopment);
        properties_v2.definition.type_kind = "predefinedAbapType".to_owned();
        properties_v2.definition.type_name = None;
        properties_v2.definition.data_type = Some("DEC".to_owned());
        properties_v2.definition.data_type_length = Some(12);
        properties_v2.definition.data_type_decimals = Some(3);

        let content = render(&snapshot(&properties)).unwrap();
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
    fn language_version_edits_preserve_unchanged_encodings() {
        for original_version in [
            None,
            Some(AdtAbapLanguageVersion::Other(String::new())),
            Some(AdtAbapLanguageVersion::Other(" ".to_owned())),
            Some(AdtAbapLanguageVersion::Other("0".to_owned())),
            Some(AdtAbapLanguageVersion::KeyUser),
            Some(AdtAbapLanguageVersion::CloudDevelopment),
        ] {
            let mut original = properties();
            original.abap_language_version = original_version;
            let baseline = ProjectedDataElementProperties::from_adt(&original).unwrap();
            for (aff, adt) in [
                (
                    AbapLanguageVersion::Standard,
                    AdtAbapLanguageVersion::Other("0".to_owned()),
                ),
                (
                    AbapLanguageVersion::KeyUser,
                    AdtAbapLanguageVersion::KeyUser,
                ),
                (
                    AbapLanguageVersion::CloudDevelopment,
                    AdtAbapLanguageVersion::CloudDevelopment,
                ),
            ] {
                let mut edited = baseline.clone();
                edited.header.description = "Updated data element".to_owned();
                edited.header.abap_language_version = aff;
                let mut expected = original.clone();
                expected.description = Some(edited.header.description.clone());
                if aff != baseline.header.abap_language_version {
                    expected.abap_language_version = Some(adt);
                }
                let merged = merge(
                    &snapshot(&original),
                    &serde_json::to_string(&edited).unwrap(),
                )
                .unwrap()
                .unwrap();
                assert_eq!(merged, serde_json::to_value(expected).unwrap());
                let merged: DataElementProperties = serde_json::from_value(merged).unwrap();
                assert_eq!(
                    ProjectedDataElementProperties::from_adt(&merged).unwrap(),
                    edited
                );
            }
        }
    }

    #[test]
    fn maps_writing_direction_in_both_directions() {
        let mut properties = properties();
        properties.definition.left_to_right_direction = Some(false);

        let content = render(&snapshot(&properties)).unwrap();
        let document: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            document["additionalProperties"]["bidirectionalOptions"]["basicDirection"],
            "rightToLeft"
        );

        let edited = content.replace("rightToLeft", "leftToRight");
        let merged: DataElementProperties =
            serde_json::from_value(merge(&snapshot(&properties), &edited).unwrap().unwrap())
                .unwrap();
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

        let merged: DataElementProperties =
            serde_json::from_value(merge(&snapshot(&original), edited).unwrap().unwrap()).unwrap();
        let properties = &merged;
        let definition = &properties.definition;

        assert_eq!(properties.description.as_deref(), Some("Updated"));
        assert_eq!(properties.master_language.as_deref(), Some("DE"));
        assert_eq!(
            properties.abap_language_version,
            Some(AdtAbapLanguageVersion::KeyUser)
        );
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
        let content = render(&snapshot(&original)).unwrap();

        let merged = merge(&snapshot(&original), &content).unwrap();

        assert_eq!(merged, None);
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

        let content = render(&snapshot(&original)).unwrap();
        let merged = merge(&snapshot(&original), &content).unwrap();

        assert_eq!(merged, None);
    }

    #[test]
    fn preserves_unmodeled_adt_only_wire_values() {
        let mut original = properties();
        original.definition.documentation_status = Some("futureStatus".to_owned());
        original.definition.short_field_max_length = Some(9);
        original.definition.medium_field_max_length = Some(19);

        let content = render(&snapshot(&original)).unwrap();
        assert_eq!(merge(&snapshot(&original), &content).unwrap(), None);
        let edited = content.replacen(
            "\"description\": \"Example data element\"",
            "\"description\": \"Updated data element\"",
            1,
        );
        let merged: DataElementProperties =
            serde_json::from_value(merge(&snapshot(&original), &edited).unwrap().unwrap()).unwrap();

        assert_eq!(merged.description.as_deref(), Some("Updated data element"));
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
        let content = render(&snapshot(&original)).unwrap();
        let edited = content
            .replace("\"length\": 10", "\"length\": 11")
            .replace("\"name\": \"Z_OLD\"", "\"name\": \"Z_NEW\"");

        let merged: DataElementProperties =
            serde_json::from_value(merge(&snapshot(&original), &edited).unwrap().unwrap()).unwrap();
        let definition = &merged.definition;

        assert_eq!(definition.data_type_length, Some(11));
        assert_eq!(definition.data_type_decimals, None);
        assert_eq!(definition.search_help.as_deref(), Some("Z_NEW"));
        assert_eq!(definition.search_help_parameter, None);
    }

    #[test]
    fn changing_a_predefined_data_type_clears_only_dependent_flags() {
        for (from, to) in [("CHAR", "DEC"), ("DEC", "CHAR")] {
            let mut original = properties();
            original.definition.type_kind = "predefinedAbapType".to_owned();
            original.definition.data_type = Some(from.to_owned());
            original.definition.data_type_decimals = None;
            original.definition.data_type_length_enabled = Some(true);
            original.definition.data_type_decimals_enabled = Some(from == "DEC");
            let content = render(&snapshot(&original)).unwrap();
            let edited = content.replace(
                &format!("\"dataType\": \"{from}\""),
                &format!("\"dataType\": \"{to}\""),
            );

            let merged: DataElementProperties =
                serde_json::from_value(merge(&snapshot(&original), &edited).unwrap().unwrap())
                    .unwrap();
            let mut expected = original.clone();
            expected.definition.data_type = Some(to.to_owned());
            expected.definition.data_type_length_enabled = None;
            expected.definition.data_type_decimals_enabled = None;
            assert_eq!(merged, expected, "{from} -> {to}");
            assert_eq!(render(&snapshot(&merged)).unwrap(), edited);
            assert_eq!(merge(&snapshot(&merged), &edited).unwrap(), None);
        }
    }

    #[test]
    fn predefined_length_and_decimals_edits_preserve_dependent_flags() {
        for data_type in ["CHAR", "DEC"] {
            let mut original = properties();
            original.definition.type_kind = "predefinedAbapType".to_owned();
            original.definition.data_type = Some(data_type.to_owned());
            original.definition.data_type_decimals = None;
            original.definition.data_type_length_enabled = Some(true);
            original.definition.data_type_decimals_enabled = Some(data_type == "DEC");
            for field in ["length", "decimals"] {
                let mut edited: Value =
                    serde_json::from_str(&render(&snapshot(&original)).unwrap()).unwrap();
                edited["dataTypeInformation"]["predefinedType"][field] = json!(3);

                let merged: DataElementProperties = serde_json::from_value(
                    merge(&snapshot(&original), &edited.to_string())
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                let mut expected = original.clone();
                if field == "length" {
                    expected.definition.data_type_length = Some(3);
                } else {
                    expected.definition.data_type_decimals = Some(3);
                }
                assert_eq!(merged, expected, "{data_type}: {field}");
                let rendered = render(&snapshot(&merged)).unwrap();
                assert_eq!(serde_json::from_str::<Value>(&rendered).unwrap(), edited);
                assert_eq!(merge(&snapshot(&merged), &rendered).unwrap(), None);
            }
        }
    }

    #[test]
    fn changing_a_referenced_type_clears_stale_resolved_type_data() {
        let original = properties();
        let content = render(&snapshot(&original)).unwrap();
        let edited = content.replace("Z_EXAMPLE_DOMAIN", "Z_OTHER_DOMAIN");

        let merged: DataElementProperties =
            serde_json::from_value(merge(&snapshot(&original), &edited).unwrap().unwrap()).unwrap();
        let definition = &merged.definition;

        assert_eq!(definition.type_name.as_deref(), Some("Z_OTHER_DOMAIN"));
        assert_eq!(definition.data_type, None);
        assert_eq!(definition.data_type_length, None);
        assert_eq!(definition.data_type_length_enabled, None);
        assert_eq!(definition.data_type_decimals, None);
        assert_eq!(definition.data_type_decimals_enabled, None);
    }

    #[test]
    fn rejects_schema_valid_predefined_blocks_that_cannot_round_trip() {
        for original_category in ["domain", "predefinedAbapType"] {
            let mut original = properties();
            original.definition.type_kind = original_category.to_owned();
            for category in [
                "domain",
                "referenceToPredefinedType",
                "referenceDictionaryType",
                "referenceClasIntType",
            ] {
                let mut edited: Value =
                    serde_json::from_str(&render(&snapshot(&original)).unwrap()).unwrap();
                edited["dataTypeInformation"]["category"] = json!(category);
                for length in [8, 12] {
                    edited["dataTypeInformation"]["predefinedType"] =
                        json!({ "dataType": "CHAR", "length": length });
                    let document: ProjectedDataElementProperties =
                        serde_json::from_value(edited.clone()).unwrap();
                    document.validate().unwrap();

                    assert!(matches!(
                        merge(&snapshot(&original), &edited.to_string()),
                        Err(ProjectionError::InvalidDataElementField {
                            field: "dataTypeInformation.predefinedType",
                            ..
                        })
                    ));
                }
            }
        }
    }

    #[test]
    fn category_transitions_preserve_untouched_properties_and_round_trip() {
        let categories = [
            DataElementCategory::Domain,
            DataElementCategory::PredefinedType,
            DataElementCategory::ReferenceToPredefinedType,
            DataElementCategory::ReferenceDictionaryType,
            DataElementCategory::ReferenceClassOrInterfaceType,
        ];
        for original_category in categories {
            let mut original = properties();
            original.definition.type_kind = original_category.adt_value().to_owned();
            original.definition.data_type_length_enabled = Some(true);
            original.definition.data_type_decimals_enabled = Some(false);
            for category in categories {
                let mut edited = ProjectedDataElementProperties::from_adt(&original).unwrap();
                edited.data_type_information.category = category;
                edited.data_type_information.predefined_type =
                    (category == DataElementCategory::PredefinedType).then(|| PredefinedType {
                        data_type: "CHAR".to_owned(),
                        length: 8,
                        decimals: None,
                    });
                let content = serde_json::to_string(&edited).unwrap();
                let payload = merge(&snapshot(&original), &content).unwrap();

                let mut expected = original.clone();
                expected.definition.type_kind = category.adt_value().to_owned();
                if category != original_category {
                    expected.definition.data_type_length_enabled = None;
                    expected.definition.data_type_decimals_enabled = None;
                    if category != DataElementCategory::PredefinedType {
                        expected.definition.data_type = None;
                        expected.definition.data_type_length = None;
                        expected.definition.data_type_decimals = None;
                    }
                }
                assert_eq!(
                    payload,
                    (expected != original).then(|| serde_json::to_value(&expected).unwrap()),
                    "{original_category:?} -> {category:?}"
                );
                let merged: DataElementProperties = payload
                    .map(|payload| serde_json::from_value(payload).unwrap())
                    .unwrap_or_else(|| original.clone());
                assert_eq!(merged, expected, "{original_category:?} -> {category:?}");
                assert_eq!(
                    ProjectedDataElementProperties::from_adt(&merged).unwrap(),
                    edited
                );
                let rendered = render(&snapshot(&merged)).unwrap();
                assert_eq!(merge(&snapshot(&merged), &rendered).unwrap(), None);
            }
        }
    }

    #[test]
    fn incomplete_predefined_types_preserve_unedited_dependent_fields() {
        for (data_type, length) in [(None, None), (Some("CHAR"), None), (None, Some(8))] {
            let mut original = properties();
            original.definition.type_kind = "predefinedAbapType".to_owned();
            original.definition.data_type = data_type.map(str::to_owned);
            original.definition.data_type_length = length;
            original.definition.data_type_decimals = Some(3);
            original.definition.data_type_length_enabled = Some(true);
            original.definition.data_type_decimals_enabled = Some(false);
            let content = render(&snapshot(&original)).unwrap();
            assert_eq!(merge(&snapshot(&original), &content).unwrap(), None);

            let edited = content.replace("Z_EXAMPLE_DOMAIN", "Z_OTHER_DOMAIN");
            let merged: DataElementProperties =
                serde_json::from_value(merge(&snapshot(&original), &edited).unwrap().unwrap())
                    .unwrap();
            let mut expected = original.clone();
            expected.definition.type_name = Some("Z_OTHER_DOMAIN".to_owned());
            assert_eq!(merged, expected);
            assert_eq!(render(&snapshot(&merged)).unwrap(), edited);
        }
    }

    #[test]
    fn switching_to_an_incomplete_predefined_type_clears_only_stale_type_data() {
        let original = properties();
        let mut edited = ProjectedDataElementProperties::from_adt(&original).unwrap();
        edited.data_type_information.category = DataElementCategory::PredefinedType;
        let merged: DataElementProperties = serde_json::from_value(
            merge(
                &snapshot(&original),
                &serde_json::to_string(&edited).unwrap(),
            )
            .unwrap()
            .unwrap(),
        )
        .unwrap();

        let mut expected = original.clone();
        expected.definition.type_kind = "predefinedAbapType".to_owned();
        expected.definition.data_type = None;
        expected.definition.data_type_length = None;
        expected.definition.data_type_decimals = None;
        expected.definition.data_type_length_enabled = None;
        expected.definition.data_type_decimals_enabled = None;
        assert_eq!(merged, expected);
        assert_eq!(
            ProjectedDataElementProperties::from_adt(&merged).unwrap(),
            edited
        );
        let rendered = render(&snapshot(&merged)).unwrap();
        assert_eq!(merge(&snapshot(&merged), &rendered).unwrap(), None);
    }

    #[test]
    fn rejects_documents_outside_the_aff_schema() {
        let properties = properties();
        let invalid_version = render(&snapshot(&properties))
            .unwrap()
            .replace("\"formatVersion\": \"1\"", "\"formatVersion\": \"2\"");
        assert!(matches!(
            merge(&snapshot(&properties), &invalid_version),
            Err(ProjectionError::Validation(_))
        ));

        let unknown =
            render(&snapshot(&properties))
                .unwrap()
                .replacen('{', "{\n  \"unknown\": true,", 1);
        assert!(matches!(
            merge(&snapshot(&properties), &unknown),
            Err(ProjectionError::Json(_))
        ));
    }

    #[test]
    fn accepts_schema_optional_type_information() {
        let original = properties();
        let mut missing_type_name: Value =
            serde_json::from_str(&render(&snapshot(&original)).unwrap()).unwrap();
        missing_type_name["dataTypeInformation"]
            .as_object_mut()
            .unwrap()
            .remove("typeName");
        let merged: DataElementProperties = serde_json::from_value(
            merge(&snapshot(&original), &missing_type_name.to_string())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(merged.definition.type_name, None);

        let mut predefined = properties();
        predefined.definition.type_kind = "predefinedAbapType".to_owned();
        predefined.definition.type_name = None;
        predefined.definition.data_type = Some("CHAR".to_owned());
        predefined.definition.data_type_length = Some(10);
        let mut unexpected_type_name: Value =
            serde_json::from_str(&render(&snapshot(&predefined)).unwrap()).unwrap();
        unexpected_type_name["dataTypeInformation"]["typeName"] = json!("Z_DOMAIN");
        let merged: DataElementProperties = serde_json::from_value(
            merge(&snapshot(&predefined), &unexpected_type_name.to_string())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(merged.definition.type_name.as_deref(), Some("Z_DOMAIN"));

        predefined.definition.type_name = None;
        predefined.definition.data_type = None;
        predefined.definition.data_type_length = None;
        let rendered = render(&snapshot(&predefined)).unwrap();
        let document: Value = serde_json::from_str(&rendered).unwrap();
        assert!(
            document["dataTypeInformation"]
                .get("predefinedType")
                .is_none()
        );

        let mut completed = document;
        completed["dataTypeInformation"]["predefinedType"] =
            json!({ "dataType": "CHAR", "length": 12 });
        let merged: DataElementProperties = serde_json::from_value(
            merge(&snapshot(&predefined), &completed.to_string())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(merged.definition.data_type.as_deref(), Some("CHAR"));
        assert_eq!(merged.definition.data_type_length, Some(12));
    }
}
