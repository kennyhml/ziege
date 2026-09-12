//! Function Group, include, and function-module mappings to AFF metadata and source files.
//!
//! Fictional group, with each snapshot projected separately (assuming advertised sources):
//!
//! ```text
//! Z_CALCULATOR (function group)
//! |   -> z_calculator.fugr.json
//! |   -> z_calculator.fugr.saplz_calculator.reps.json
//! |   -> z_calculator.fugr.saplz_calculator.reps.abap
//! |
//! +-- LZ_CALCULATORTOP (include)
//! |       -> z_calculator.fugr.lz_calculatortop.reps.json
//! |       -> z_calculator.fugr.lz_calculatortop.reps.abap
//! |
//! +-- Z_ADD (function module)
//!         -> z_calculator.fugr.z_add.func.json
//!         -> z_calculator.fugr.z_add.func.abap
//! ```
//!
//! A group, its includes, and its function modules are projected separately.
//! The group snapshot supplies its own metadata and advertised main source.
//! Main-program REPS metadata uses the group snapshot. Each include uses its own
//! FunctionGroupInclude snapshot. Each FunctionModule snapshot supplies FUNC v1
//! metadata and an advertised main [`zadt::SourceRef`]. Source text is not fetched
//! or transformed here.
//! Child membership and folder assembly belong to the caller, not this format.
//! Mapping tables use AFF JSON fields and ADT Rust field names, not XML attributes.
//!
//! FUNC schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/fugr/func-v1.json>.

use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{
    FunctionGroup, FunctionGroupInclude, FunctionGroupProperties, FunctionModule, ObjectSnapshot,
    ObjectType,
};

use crate::{
    AbapLanguageVersion, Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, NameSource, PropertiesMapping},
    helpers::{is_false, object, objects, optional_object, parse_object, present, string_enum},
    models::{language_from_adt, language_to_adt},
    validate::{numeric_string, one_of, optional_date, unique_items},
};

// Group
pub(crate) static FUNCTION_GROUP_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "FUGR",
    version: "1",
    workbench_types: &[FunctionGroup::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.fugr.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.fugr.sapl<name>.reps.abap",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
        FileSpec::new(
            "<name>.fugr.sapl<name>.reps.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping {
                render: render_include,
                merge: merge_include,
            }),
        ),
        FileSpec::new(
            "<name>.fugr.texts.<lang>.properties",
            Cardinality::ZeroOrMore,
            Mapping::Unavailable,
        ),
    ],
};

/// Renders group metadata as AFF JSON with a trailing newline.
fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let document =
        ProjectedFunctionGroupProperties::from_adt(obj.typed_properties::<FunctionGroup>()?)?;
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');
    Ok(content)
}

/// Maps AFF edits to complete ADT properties, or None for a no-op.
fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<FunctionGroup>()?;
    let edited: ProjectedFunctionGroupProperties = parse_object(edited)?;
    edited.validate()?;
    let previous = ProjectedFunctionGroupProperties::from_adt(original)?;
    let mut merged = original.clone();

    // AFF header.description       -> ADT description
    // AFF header.originalLanguage  -> ADT master_language, converted from BCP47
    // AFF fixPointArithmetic       -> ADT fix_point_arithmetic
    merged.description = edited.header.description;
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    merged.fix_point_arithmetic = edited.fix_point_arithmetic;
    if edited.status != previous.status {
        merged.source_object_status = Some(edited.status.adt_value());
    }

    // AFF Standard can represent absent, blank, or "X" ADT values.
    // Only a changed language version replaces that original representation.
    if edited.header.abap_language_version != previous.header.abap_language_version {
        merged.abap_language_version = Some(edited.header.abap_language_version.to_adt_reps());
    }
    if merged == *original {
        return Ok(None);
    }
    serde_json::to_value(merged).map(Some).map_err(Into::into)
}

/// Function Group properties represented by the AFF FUGR v1 document.
///
/// ```text
/// AFF field                   ADT FunctionGroupProperties field
/// ---------                   ---------------------------------
/// formatVersion               No field, fixed to "1"
/// header.description          description
/// header.originalLanguage     master_language
/// header.abapLanguageVersion  abap_language_version
/// fixPointArithmetic          fix_point_arithmetic
/// status                      source_object_status
/// ```
///
/// `fixPointArithmetic` is required even when false. It does not change the
/// separate ADT `unicode_check_active` flag. Unchanged status values retain
/// their original ADT representation.
///
/// The main-program REPS document also exposes the same ADT description.
/// Saving either document requires a fresh snapshot before editing or saving
/// the other against a new baseline. The two files are not independent ADT records.
///
/// Includes and function modules have separate snapshots and projections.
/// Text elements, dynpros, and documentation are not provided by this mapping.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProjectedFunctionGroupProperties {
    #[garde(custom(one_of([FUNCTION_GROUP_FORMAT.version()])))]
    pub format_version: String,

    #[serde(deserialize_with = "object")]
    #[garde(dive)]
    pub header: FunctionGroupHeader,

    pub fix_point_arithmetic: bool,

    #[serde(
        default,
        skip_serializing_if = "FunctionGroupStatus::is_default",
        deserialize_with = "string_enum"
    )]
    pub status: FunctionGroupStatus,
}

/// Description and language fields for a Function Group.
///
/// ```text
/// AFF field                   ADT field
/// ---------                   ---------
/// header.description          description
/// header.originalLanguage     master_language
/// header.abapLanguageVersion  abap_language_version
/// ```
///
/// Description is limited to 40 Unicode characters. Original language uses
/// BCP47 in AFF and SAP codes in ADT, for example `en` and `EN`.
///
/// ```text
/// AFF language version  Accepted ADT values    ADT value for an edit
/// --------------------  -------------------    ---------------------
/// standard              Absent, "", " ", "X"   "X"
/// keyUser               "2"                    "2"
/// cloudDevelopment      "5"                    "5"
/// ```
///
/// An unchanged Standard retains the original ADT representation. The direct
/// `abap_language_version` field is used, not the parser version inside
/// `syntax_configuration`. That parser metadata is preserved without modification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct FunctionGroupHeader {
    #[garde(length(chars, max = 40))]
    pub description: String,

    #[garde(length(chars, min = 2))]
    pub original_language: String,

    #[serde(
        default,
        skip_serializing_if = "AbapLanguageVersion::is_standard",
        deserialize_with = "string_enum"
    )]
    pub abap_language_version: AbapLanguageVersion,
}

/// AFF Function Group status vocabulary.
///
/// ```text
/// AFF value        ADT source_object_status
/// ---------        ------------------------
/// notClassified    unknown
/// sapProgram       SAPStandardProduction
/// customerProgram  customerProduction
/// systemProgram    system
/// testProgram      test
/// ```
///
/// Absent or empty ADT values also render as `notClassified`, omitted from JSON.
/// Unchanged values preserve their original spelling. Other ADT strings are rejected.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionGroupStatus {
    #[default]
    NotClassified,
    SapProgram,
    CustomerProgram,
    SystemProgram,
    TestProgram,
}

impl FunctionGroupStatus {
    fn from_adt(value: Option<&zadt::SourceObjectStatus>) -> Result<Self, ProjectionError> {
        use zadt::SourceObjectStatus;
        match value {
            None | Some(SourceObjectStatus::Unknown) => Ok(Self::NotClassified),
            Some(SourceObjectStatus::Other(value)) if value.is_empty() => Ok(Self::NotClassified),
            Some(SourceObjectStatus::SapStandardProduction) => Ok(Self::SapProgram),
            Some(SourceObjectStatus::CustomerProduction) => Ok(Self::CustomerProgram),
            Some(SourceObjectStatus::System) => Ok(Self::SystemProgram),
            Some(SourceObjectStatus::Test) => Ok(Self::TestProgram),
            Some(SourceObjectStatus::Other(value)) => Err(ProjectionError::InvalidAffField {
                field: "status",
                message: format!("unsupported ADT source object status `{value}`"),
            }),
        }
    }

    const fn adt_value(self) -> zadt::SourceObjectStatus {
        match self {
            Self::NotClassified => zadt::SourceObjectStatus::Unknown,
            Self::SapProgram => zadt::SourceObjectStatus::SapStandardProduction,
            Self::CustomerProgram => zadt::SourceObjectStatus::CustomerProduction,
            Self::SystemProgram => zadt::SourceObjectStatus::System,
            Self::TestProgram => zadt::SourceObjectStatus::Test,
        }
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::NotClassified)
    }
}

impl ProjectedFunctionGroupProperties {
    fn from_adt(properties: &FunctionGroupProperties) -> Result<Self, ProjectionError> {
        let document = Self {
            format_version: FUNCTION_GROUP_FORMAT.version().to_owned(),
            header: FunctionGroupHeader {
                description: properties.description.clone(),
                original_language: language_from_adt(
                    &properties.master_language,
                    "header.originalLanguage",
                )?,
                abap_language_version: AbapLanguageVersion::from_adt(
                    properties.abap_language_version.as_ref(),
                    "X",
                )
                .map_err(|value| ProjectionError::InvalidAffField {
                    field: "header.abapLanguageVersion",
                    message: format!("unsupported ADT value `{value}`"),
                })?,
            },
            fix_point_arithmetic: properties.fix_point_arithmetic,
            status: FunctionGroupStatus::from_adt(properties.source_object_status.as_ref())?,
        };
        document.validate()?;
        Ok(document)
    }
}

// Includes

pub(crate) static FUNCTION_GROUP_INCLUDE_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "REPS",
    version: "1",
    workbench_types: &[FunctionGroupInclude::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.fugr.<include>.reps.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping {
                render: render_include,
                merge: merge_include,
            }),
        )
        .with_names(&[
            ("name", NameSource::Parent),
            ("include", NameSource::Object),
        ]),
        FileSpec::new(
            "<name>.fugr.<include>.reps.abap",
            Cardinality::One,
            Mapping::Source { component: None },
        )
        .with_names(&[
            ("name", NameSource::Parent),
            ("include", NameSource::Object),
        ]),
    ],
};

/// Renders REPS metadata from a group or include snapshot.
fn render_include(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let document = ProjectedFunctionGroupIncludeProperties::from_adt(obj)?;
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');
    Ok(content)
}

/// Maps REPS description and supported editor-lock settings to their owning ADT properties.
fn merge_include(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let edited: ProjectedFunctionGroupIncludeProperties = parse_object(edited)?;
    edited.validate()?;
    let previous = ProjectedFunctionGroupIncludeProperties::from_adt(obj)?;
    if edited.include_type != previous.include_type {
        return Err(ProjectionError::InvalidAffField {
            field: "includeType",
            message: "the include type must match the owning ADT object".to_owned(),
        });
    }
    if obj.key().workbench_type() == &FunctionGroup::WORKBENCH_TYPE {
        let original = obj.typed_properties::<FunctionGroup>()?;
        // The shared group description is limited to 40 characters in FUGR,
        // even though the general REPS schema permits 70.
        if edited.header.description != previous.header.description
            && edited.header.description.chars().count() > 40
        {
            return Err(ProjectionError::InvalidAffField {
                field: "header.description",
                message: "function group descriptions are limited to 40 characters".to_owned(),
            });
        }
        let mut merged = original.clone();
        merged.description = edited.header.description;
        merged.locked_by_editor = edited.edit_locked;
        if merged == *original {
            return Ok(None);
        }
        serde_json::to_value(merged).map(Some).map_err(Into::into)
    } else {
        let original = obj.typed_properties::<FunctionGroupInclude>()?;
        if edited.edit_locked {
            return Err(ProjectionError::UnsupportedAffProperty {
                object_type: "REPS",
                field: "editLocked",
            });
        }
        let mut merged = original.clone();
        // AFF empty text can mean either ADT None or Some(""). Preserve that
        // distinction unless the user changed the description.
        if edited.header.description != previous.header.description {
            merged.description = Some(edited.header.description);
        }
        if merged == *original {
            return Ok(None);
        }
        serde_json::to_value(merged).map(Some).map_err(Into::into)
    }
}

/// REPS properties for a main Function Group program or a separate group include.
///
/// ```text
/// AFF field           FunctionGroupProperties  FunctionGroupIncludeProperties
/// ---------           -----------------------  ------------------------------
/// formatVersion       No field, fixed to "1"   No field, fixed to "1"
/// header.description  description              description
/// editLocked          locked_by_editor         No implemented backing
/// includeType         Fixed "functionGroup"    Fixed "include"
/// ```
///
/// `includeType` is required even when its value is `include`. It cannot be
/// used to change a main program into an include or vice versa.
///
/// The main-program description is the same ADT field exposed in `.fugr.json`.
/// Group edits are limited to 40 characters. Include descriptions use the REPS
/// limit of 70 characters. An absent include description is represented as empty
/// text and stays absent if the AFF description is not changed.
///
/// The editor-lock flag is an object property, not an ADT session lock. It is
/// mapped for the main group program only. Include edits can only use false.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProjectedFunctionGroupIncludeProperties {
    #[garde(custom(one_of([FUNCTION_GROUP_INCLUDE_FORMAT.version()])))]
    pub format_version: String,

    #[serde(deserialize_with = "object")]
    #[garde(dive)]
    pub header: RepsHeader,

    #[serde(default, skip_serializing_if = "is_false")]
    pub edit_locked: bool,

    #[serde(deserialize_with = "string_enum")]
    pub include_type: FunctionGroupIncludeType,
}

/// REPS description mapped to the owning ADT `description` field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct RepsHeader {
    #[garde(length(chars, max = 70))]
    pub description: String,
}

/// AFF REPS kind determined by the owning ADT object, not an editable conversion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionGroupIncludeType {
    Include,
    FunctionGroup,
}

impl ProjectedFunctionGroupIncludeProperties {
    fn from_adt(obj: &ObjectSnapshot<()>) -> Result<Self, ProjectionError> {
        let (description, edit_locked, include_type) =
            if obj.key().workbench_type() == &FunctionGroup::WORKBENCH_TYPE {
                let properties = obj.typed_properties::<FunctionGroup>()?;
                (
                    properties.description.clone(),
                    properties.locked_by_editor,
                    FunctionGroupIncludeType::FunctionGroup,
                )
            } else {
                let properties = obj.typed_properties::<FunctionGroupInclude>()?;
                (
                    properties.description.clone().unwrap_or_default(),
                    false,
                    FunctionGroupIncludeType::Include,
                )
            };
        let document = Self {
            format_version: FUNCTION_GROUP_INCLUDE_FORMAT.version().to_owned(),
            header: RepsHeader { description },
            edit_locked,
            include_type,
        };
        document.validate()?;
        Ok(document)
    }
}

// Modules

pub(crate) static FUNCTION_MODULE_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "FUNC",
    version: "1",
    workbench_types: &[FunctionModule::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.fugr.<fmname>.func.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping {
                render: render_function_module,
                merge: merge_function_module,
            }),
        )
        .with_names(&[("name", NameSource::Parent), ("fmname", NameSource::Object)]),
        FileSpec::new(
            "<name>.fugr.<fmname>.func.abap",
            Cardinality::One,
            Mapping::Source { component: None },
        )
        .with_names(&[("name", NameSource::Parent), ("fmname", NameSource::Object)]),
    ],
};

/// Renders validated FUNC JSON with a trailing newline.
fn render_function_module(snapshot: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = snapshot.typed_properties::<FunctionModule>()?;
    let document = ProjectedFunctionModuleProperties {
        format_version: FUNCTION_MODULE_FORMAT.version().to_owned(),
        header: FunctionModuleHeader {
            description: properties.description.clone(),
        },
        processing_type: FunctionModuleProcessingType::from_adt(&properties.processing_type)?,
        rfc_properties: None,
        update_properties: None,
        release_state: FunctionModuleReleaseState::from_adt(&properties.release_state)?,
        release_date: None,
        global: false,
        exception_classes: false,
        application: String::new(),
        client: String::new(),
        active_function_exit: false,
        // TEMP placeholder, not an assignment to a function-group include.
        include_number: "00".to_owned(),
        not_executable: false,
        edit_locked: false,
        parameters: Vec::new(),
        exceptions: Vec::new(),
    };
    document.validate()?;
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');
    Ok(content)
}

/// Validates edits and returns changed ADT wire properties, or `None` for a no-op.
fn merge_function_module(
    snapshot: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = snapshot.typed_properties::<FunctionModule>()?;
    let edited: ProjectedFunctionModuleProperties = parse_object(edited)?;
    edited.validate()?;

    let previous_type = FunctionModuleProcessingType::from_adt(&original.processing_type)?;
    FunctionModuleReleaseState::from_adt(&original.release_state)?;

    for (unsupported, field) in [
        (edited.release_date.is_some(), "releaseDate"),
        (edited.global, "global"),
        (edited.exception_classes, "exceptionClasses"),
        (!edited.application.is_empty(), "application"),
        (!edited.client.is_empty(), "client"),
        (edited.active_function_exit, "activeFunctionExit"),
        (edited.include_number != "00", "includeNumber"),
        (edited.not_executable, "notExecutable"),
        (edited.edit_locked, "editLocked"),
        (!edited.parameters.is_empty(), "parameters"),
        (!edited.exceptions.is_empty(), "exceptions"),
    ] {
        if unsupported {
            return Err(ProjectionError::UnsupportedAffProperty {
                object_type: "FUNC",
                field,
            });
        }
    }
    if let Some(rfc) = &edited.rfc_properties {
        for (unsupported, field) in [
            (rfc.basxml_enabled, "rfcProperties.basxmlEnabled"),
            (
                rfc.rfc_scope != RfcScope::NotClassified,
                "rfcProperties.rfcScope",
            ),
            (
                rfc.rfc_version != RfcVersion::Any,
                "rfcProperties.rfcVersion",
            ),
            (rfc.abap_from_java, "rfcProperties.abapFromJava"),
            (rfc.java_from_abap, "rfcProperties.javaFromAbap"),
            (rfc.java_remote, "rfcProperties.javaRemote"),
            (
                previous_type == FunctionModuleProcessingType::Rfc
                    || edited.processing_type == FunctionModuleProcessingType::Rfc,
                "rfcProperties",
            ),
        ] {
            if unsupported {
                return Err(ProjectionError::UnsupportedAffProperty {
                    object_type: "FUNC",
                    field,
                });
            }
        }
    }
    if let Some(update) = &edited.update_properties {
        if update.update_task_kind != UpdateTaskKind::StartImmediately {
            return Err(ProjectionError::UnsupportedAffProperty {
                object_type: "FUNC",
                field: "updateProperties.updateTaskKind",
            });
        }
        if previous_type == FunctionModuleProcessingType::Update
            || edited.processing_type == FunctionModuleProcessingType::Update
        {
            return Err(ProjectionError::UnsupportedAffProperty {
                object_type: "FUNC",
                field: "updateProperties",
            });
        }
    }

    let mut merged = original.clone();
    merged.description = edited.header.description;
    merged.processing_type = edited.processing_type.adt_value().to_owned();
    merged.release_state = edited.release_state.adt_value().to_owned();
    if merged == *original {
        return Ok(None);
    }
    serde_json::to_value(merged).map(Some).map_err(Into::into)
}

/// Function-module metadata represented by the complete FUNC v1 schema.
///
/// # Field Mapping
///
/// ADT field names refer to [`zadt::FunctionModuleProperties`].
///
/// ```text
/// AFF field           ADT field or accepted unbacked value
/// ---------           -----------------------------------
/// formatVersion       Constant "1", required
/// header.description  description, required, at most 74 characters
/// processingType      processing_type, required even for normal
/// rfcProperties       No backing, see RfcProperties for acceptance policy
/// updateProperties    No backing, see UpdateProperties for acceptance policy
/// releaseState        release_state, omission means notReleased
/// releaseDate         No backing, every supplied date is rejected after validation
/// global              No backing, absent or false
/// exceptionClasses    No backing, absent or false
/// application         No backing, absent or empty string, at most 1 character
/// client              No backing, absent or empty string, at most 3 characters
/// activeFunctionExit  No backing, absent or false
/// includeNumber       Required TEMP "00", no include assignment is performed
/// notExecutable       No backing, absent or false
/// editLocked          No backing, absent or false
/// parameters          No backing, absent or empty array
/// exceptions          No backing, absent or empty array
/// ```
///
/// The required include number is a string of one or two ASCII digits in the
/// schema. ZADT has no include-number field, so rendering supplies the TEMP
/// placeholder `"00"`. Even schema-valid alternatives such as `"0"` or `"01"`
/// are rejected with `UnsupportedAffProperty` at `includeNumber`.
///
/// Optional false flags, empty strings, empty arrays, and `notReleased` are
/// omitted on render. Their explicit defaults are accepted on merge. Omitted
/// `releaseState` writes `notReleased`. Other unbacked nondefault edits return
/// `UnsupportedAffProperty` with object type `FUNC` and the AFF field path.
/// Schema validation runs before unsupported-property checks.
///
/// Only description, processing type, and release state are changed in a clone
/// of the typed ADT properties. Identity, container, source URI, links, language,
/// timestamps, version, and description limit retain their original values.
/// The separate `.func.abap` backing retains the advertised source reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProjectedFunctionModuleProperties {
    #[garde(custom(one_of([FUNCTION_MODULE_FORMAT.version()])))]
    pub format_version: String,

    #[serde(deserialize_with = "object")]
    #[garde(dive)]
    pub header: FunctionModuleHeader,

    #[serde(deserialize_with = "string_enum")]
    pub processing_type: FunctionModuleProcessingType,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_object"
    )]
    #[garde(dive)]
    pub rfc_properties: Option<RfcProperties>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_object"
    )]
    #[garde(dive)]
    pub update_properties: Option<UpdateProperties>,

    #[serde(
        default,
        skip_serializing_if = "FunctionModuleReleaseState::is_default",
        deserialize_with = "string_enum"
    )]
    pub release_state: FunctionModuleReleaseState,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[garde(custom(optional_date))]
    pub release_date: Option<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub global: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub exception_classes: bool,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 1))]
    pub application: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 3))]
    pub client: String,

    #[serde(default, skip_serializing_if = "is_false")]
    pub active_function_exit: bool,

    #[garde(length(chars, min = 1, max = 2), custom(numeric_string))]
    pub include_number: String,

    #[serde(default, skip_serializing_if = "is_false")]
    pub not_executable: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub edit_locked: bool,

    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "objects"
    )]
    #[garde(dive, custom(unique_items))]
    pub parameters: Vec<FunctionModuleComponentDescription>,

    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "objects"
    )]
    #[garde(dive, custom(unique_items))]
    pub exceptions: Vec<FunctionModuleComponentDescription>,
}

/// FUNC header with only the schema description field.
///
/// ```text
/// AFF field           ADT field    Constraint
/// ---------           ---------    ----------
/// header.description  description  Required, at most 74 characters
/// ```
///
/// This header has no original-language or ABAP language-version field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct FunctionModuleHeader {
    #[garde(length(chars, max = 74))]
    pub description: String,
}

/// Processing-type vocabulary with exact, bidirectional ADT spellings.
///
/// ```text
/// AFF value  ADT processing_type
/// ---------  -------------------
/// normal     normal
/// rfc        rfc
/// update     update
/// ```
///
/// Unknown ADT strings fail rather than selecting a default. All transitions
/// between these values map directly, without synthesizing RFC or update settings.
/// The required field is always serialized, including `normal`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionModuleProcessingType {
    Normal,

    Rfc,

    Update,
}

impl FunctionModuleProcessingType {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "normal" => Ok(Self::Normal),
            "rfc" => Ok(Self::Rfc),
            "update" => Ok(Self::Update),
            value => Err(ProjectionError::InvalidAffField {
                field: "processingType",
                message: format!("unsupported ADT processing type `{value}`"),
            }),
        }
    }

    const fn adt_value(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Rfc => "rfc",
            Self::Update => "update",
        }
    }
}

/// Release-state vocabulary with exact, bidirectional ADT spellings.
///
/// ```text
/// AFF value            ADT release_state     Render policy
/// ---------            -----------------     -------------
/// notReleased          notReleased           Default, omitted
/// released             released              Included
/// releasedSapInternal  releasedSapInternal   Included
/// obsolete             obsolete              Included
/// releasePlanned       releasePlanned        Included
/// ```
///
/// Unknown ADT values, including empty strings, fail on render and merge.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionModuleReleaseState {
    #[default]
    NotReleased,

    Released,

    ReleasedSapInternal,

    Obsolete,

    ReleasePlanned,
}

impl FunctionModuleReleaseState {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "notReleased" => Ok(Self::NotReleased),
            "released" => Ok(Self::Released),
            "releasedSapInternal" => Ok(Self::ReleasedSapInternal),
            "obsolete" => Ok(Self::Obsolete),
            "releasePlanned" => Ok(Self::ReleasePlanned),
            value => Err(ProjectionError::InvalidAffField {
                field: "releaseState",
                message: format!("unsupported ADT release state `{value}`"),
            }),
        }
    }

    const fn adt_value(self) -> &'static str {
        match self {
            Self::NotReleased => "notReleased",
            Self::Released => "released",
            Self::ReleasedSapInternal => "releasedSapInternal",
            Self::Obsolete => "obsolete",
            Self::ReleasePlanned => "releasePlanned",
        }
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::NotReleased)
    }
}

/// RFC settings with no implemented ADT backing.
///
/// Paths below are relative to `rfcProperties`.
///
/// ```text
/// AFF field      Schema presence  Default accepted only for an inactive block
/// ---------      ---------------  -------------------------------------------
/// basxmlEnabled  Required         false
/// rfcScope       Required         notClassified
/// rfcVersion     Required         any
/// abapFromJava   Optional         Absent or false
/// javaFromAbap   Optional         Absent or false
/// javaRemote     Optional         Absent or false
/// ```
///
/// Rendering omits this block. Absence never writes RFC settings. An explicit
/// block is accepted only when all values above are default and both original
/// and edited processing types are not `rfc`. A block active on either side of
/// a transition is rejected at `rfcProperties`, since its default-looking values
/// cannot be compared with the actual unexposed ADT settings. Nondefault fields
/// are rejected at their precise nested paths, even in an inactive block.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct RfcProperties {
    pub basxml_enabled: bool,

    #[serde(deserialize_with = "string_enum")]
    pub rfc_scope: RfcScope,

    #[serde(deserialize_with = "string_enum")]
    pub rfc_version: RfcVersion,

    #[serde(default, skip_serializing_if = "is_false")]
    pub abap_from_java: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub java_from_abap: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub java_remote: bool,
}

/// RFC call-scope vocabulary, without an ADT mapping.
///
/// ```text
/// AFF rfcProperties.rfcScope  Mapping policy
/// -------------------------  --------------
/// fromSameClientAndUser      Rejected
/// fromSameSystem             Rejected
/// fromAnySystem              Rejected
/// notClassified              Default, subject to inactive-block policy
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RfcScope {
    FromSameClientAndUser,

    FromSameSystem,

    FromAnySystem,

    #[default]
    NotClassified,
}

/// RFC serialization vocabulary, without an ADT mapping.
///
/// ```text
/// AFF rfcProperties.rfcVersion  Mapping policy
/// ---------------------------  --------------
/// fastSerializationRequired    Rejected
/// any                          Default, subject to inactive-block policy
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RfcVersion {
    FastSerializationRequired,

    #[default]
    Any,
}

/// Update settings with no implemented ADT backing.
///
/// ```text
/// AFF field                        Schema presence  Default
/// ---------                        ---------------  -------
/// updateProperties.updateTaskKind  Required         startImmediately
/// ```
///
/// Rendering omits this block. Absence never writes update settings. An explicit
/// default block is accepted only when neither original nor edited processing
/// type is `update`. Otherwise it is rejected at `updateProperties`, since the
/// actual task kind is unexposed. Nondefault kinds fail at the nested field path.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct UpdateProperties {
    #[serde(deserialize_with = "string_enum")]
    pub update_task_kind: UpdateTaskKind,
}

/// Update-task vocabulary, without an ADT mapping.
///
/// ```text
/// AFF updateProperties.updateTaskKind  Mapping policy
/// -----------------------------------  --------------
/// startImmediately                     Default, subject to inactive-block policy
/// startDelayed                         Rejected
/// startImmediatelyNoRestart            Rejected
/// collectiveRun                        Rejected
/// unsupportedKind                      Rejected
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateTaskKind {
    #[default]
    StartImmediately,

    StartDelayed,

    StartImmediatelyNoRestart,

    CollectiveRun,

    UnsupportedKind,
}

/// Parameter or exception description with no implemented ADT backing.
///
/// ```text
/// AFF field                  Schema constraint
/// ---------                  -----------------
/// parameters[].name          Required, at most 30 characters
/// parameters[].description   Required, at most 79 characters
/// exceptions[].name          Required, at most 30 characters
/// exceptions[].description   Required, at most 79 characters
/// ```
///
/// Each array requires whole-item uniqueness, not unique names. Nonempty arrays
/// are schema-validated then rejected at `parameters` or `exceptions`. These
/// entries are metadata descriptions, not source text or source transformations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct FunctionModuleComponentDescription {
    #[garde(length(chars, max = 30))]
    pub name: String,

    #[garde(length(chars, max = 79))]
    pub description: String,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};
    use zadt::{FunctionModuleProperties, ToXml};

    use super::*;

    const MODULE_XML: &[u8] =
        include_bytes!("../../../zadt/tests/fixtures/function-module-zzzzfunc.xml");
    const MODULE_URI: &str = "/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc";
    const PROCESSING_TYPES: &[&str] = &["normal", "rfc", "update"];
    const RELEASE_STATES: &[&str] = &[
        "notReleased",
        "released",
        "releasedSapInternal",
        "obsolete",
        "releasePlanned",
    ];

    fn module_snapshot(xml: &[u8]) -> ObjectSnapshot<FunctionModule> {
        let reference = crate::test_support::reference::<FunctionModule>("ZZZZFUNC", MODULE_URI);
        crate::test_support::properties(
            &reference,
            FunctionModule::MEDIA_TYPES[0],
            "module-etag",
            xml,
        )
    }

    fn snapshot(properties: &FunctionModuleProperties) -> ObjectSnapshot<()> {
        module_snapshot(&properties.to_xml().unwrap()).into_erased()
    }

    #[test]
    fn group_status_round_trips_and_preserves_sparse_values() {
        let xml = include_str!("../../../zadt/tests/fixtures/function-group-z-test-group.xml");
        let reference = crate::test_support::reference::<FunctionGroup>(
            "Z_TEST_GROUP",
            "/sap/bc/adt/functions/groups/z_test_group",
        );
        let loaded = crate::test_support::properties(
            &reference,
            FunctionGroup::MEDIA_TYPES[0],
            "etag",
            xml.as_bytes(),
        );
        let baseline = loaded.properties();
        for (wire, aff) in [
            (None, "notClassified"),
            (Some(""), "notClassified"),
            (Some("unknown"), "notClassified"),
            (Some("SAPStandardProduction"), "sapProgram"),
            (Some("customerProduction"), "customerProgram"),
            (Some("system"), "systemProgram"),
            (Some("test"), "testProgram"),
        ] {
            let mut original = baseline.clone();
            original.source_object_status = wire.map(Into::into);
            let xml = original.to_xml().unwrap();
            let obj = crate::test_support::properties(
                &reference,
                FunctionGroup::MEDIA_TYPES[0],
                "etag",
                &xml,
            )
            .into_erased();
            assert_eq!(obj.typed_properties::<FunctionGroup>().unwrap(), &original);
            let content = render(&obj).unwrap();
            assert_eq!(merge(&obj, &content).unwrap(), None);
            let mut edited: Value = serde_json::from_str(&content).unwrap();
            assert_eq!(edited["status"].as_str().unwrap_or("notClassified"), aff);
            edited["header"]["description"] = json!("Changed description");
            let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
            let preserved: FunctionGroupProperties = serde_json::from_value(payload).unwrap();
            assert_eq!(
                preserved.source_object_status,
                original.source_object_status
            );

            for (status, expected) in [
                ("notClassified", "unknown"),
                ("sapProgram", "SAPStandardProduction"),
                ("customerProgram", "customerProduction"),
                ("systemProgram", "system"),
                ("testProgram", "test"),
            ] {
                edited["status"] = json!(status);
                let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
                let updated: FunctionGroupProperties = serde_json::from_value(payload).unwrap();
                assert_eq!(
                    updated
                        .source_object_status
                        .as_ref()
                        .map(zadt::SourceObjectStatus::as_str),
                    if status == aff { wire } else { Some(expected) }
                );
            }
        }
        assert!(FunctionGroupStatus::from_adt(Some(&"futureStatus".into())).is_err());
    }

    #[test]
    fn renders_required_defaults_and_preserves_typed_properties() {
        let snapshot = module_snapshot(MODULE_XML).into_erased();
        let original = snapshot
            .typed_properties::<FunctionModule>()
            .unwrap()
            .clone();
        let content = render_function_module(&snapshot).unwrap();
        assert!(content.ends_with('\n'));
        let mut document: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            document,
            json!({
                "formatVersion": "1",
                "header": {"description": "ftfrtat"},
                "processingType": "normal",
                "includeNumber": "00"
            })
        );
        assert_eq!(merge_function_module(&snapshot, &content).unwrap(), None);
        assert_eq!(
            merge_function_module(&snapshot, &document.to_string()).unwrap(),
            None
        );

        document["header"]["description"] = json!("Updated function module");
        let payload = merge_function_module(&snapshot, &document.to_string())
            .unwrap()
            .unwrap();
        let mut expected = original.clone();
        expected.description = "Updated function module".to_owned();
        assert_eq!(payload, serde_json::to_value(&expected).unwrap());
        assert_eq!(
            serde_json::from_value::<FunctionModuleProperties>(payload).unwrap(),
            expected
        );
        assert_eq!(
            snapshot.typed_properties::<FunctionModule>().unwrap(),
            &original
        );
        assert_eq!(render_function_module(&snapshot).unwrap(), content);
    }

    #[test]
    fn binds_metadata_and_the_untransformed_advertised_source() {
        let snapshot = Arc::new(module_snapshot(MODULE_XML).into_erased());
        assert_eq!(FUNCTION_MODULE_FORMAT.object_type(), "FUNC");
        assert_eq!(FUNCTION_MODULE_FORMAT.version(), "1");
        assert_eq!(
            FUNCTION_MODULE_FORMAT.workbench_types(),
            &[FunctionModule::WORKBENCH_TYPE]
        );
        assert_eq!(FUNCTION_MODULE_FORMAT.files().len(), 2);
        for (specification, suffix) in FUNCTION_MODULE_FORMAT.files().iter().zip(["json", "abap"]) {
            assert_eq!(specification.cardinality(), Cardinality::One);
            assert_eq!(
                specification.template(),
                format!("<name>.fugr.<fmname>.func.{suffix}")
            );
            let file = specification.bind(&snapshot).unwrap().unwrap();
            assert_eq!(
                file.name(),
                format!("z_test_group.fugr.zzzzfunc.func.{suffix}")
            );
            match file.backing() {
                crate::FileBacking::Properties(properties) => {
                    assert_eq!(suffix, "json");
                    assert!(std::ptr::eq(properties.subject(), snapshot.as_ref()));
                    let rendered = properties.render().unwrap();
                    assert_eq!(properties.merge(&rendered).unwrap(), None);
                }
                crate::FileBacking::Source(source) => {
                    assert_eq!(suffix, "abap");
                    assert_eq!(source, &snapshot.source().unwrap());
                    assert_eq!(source.object.uri().as_str(), MODULE_URI);
                    assert_eq!(source.uri.as_str(), format!("{MODULE_URI}/source/main"));
                    assert_eq!(source.etag.as_deref(), Some("202608051521490001"));
                }
            }
        }
    }

    #[test]
    fn processing_and_release_enums_render_and_all_transitions_preserve_other_fields() {
        let fixture = module_snapshot(MODULE_XML).properties().clone();
        for &previous_type in PROCESSING_TYPES {
            for &previous_release in RELEASE_STATES {
                let mut original = fixture.clone();
                original.processing_type = previous_type.to_owned();
                original.release_state = previous_release.to_owned();
                let baseline = snapshot(&original);
                let content = render_function_module(&baseline).unwrap();
                let document: Value = serde_json::from_str(&content).unwrap();
                assert_eq!(document["processingType"], previous_type);
                if previous_release == "notReleased" {
                    assert!(document.get("releaseState").is_none());
                } else {
                    assert_eq!(document["releaseState"], previous_release);
                }
                assert!(document.get("rfcProperties").is_none());
                assert!(document.get("updateProperties").is_none());
                assert_eq!(merge_function_module(&baseline, &content).unwrap(), None);

                for &processing_type in PROCESSING_TYPES {
                    for &release_state in RELEASE_STATES {
                        let mut edited = document.clone();
                        edited["processingType"] = json!(processing_type);
                        edited["releaseState"] = json!(release_state);
                        let mut expected = original.clone();
                        expected.processing_type = processing_type.to_owned();
                        expected.release_state = release_state.to_owned();
                        let payload =
                            merge_function_module(&baseline, &edited.to_string()).unwrap();
                        assert_eq!(
                            payload,
                            (expected != original)
                                .then(|| serde_json::to_value(&expected).unwrap())
                        );
                        let merged: FunctionModuleProperties = serde_json::from_value(
                            payload.unwrap_or_else(|| serde_json::to_value(&original).unwrap()),
                        )
                        .unwrap();
                        assert_eq!(merged, expected);
                        let rendered: ProjectedFunctionModuleProperties = serde_json::from_str(
                            &render_function_module(&snapshot(&merged)).unwrap(),
                        )
                        .unwrap();
                        assert_eq!(rendered, serde_json::from_value(edited).unwrap());
                    }
                }

                let mut omitted = document;
                omitted.as_object_mut().unwrap().remove("releaseState");
                let mut expected = original.clone();
                expected.release_state = "notReleased".to_owned();
                assert_eq!(
                    merge_function_module(&baseline, &omitted.to_string()).unwrap(),
                    (expected != original).then(|| serde_json::to_value(expected).unwrap())
                );
            }
        }
    }

    #[test]
    fn rejects_unknown_adt_values_on_render_and_merge() {
        let fixture = module_snapshot(MODULE_XML).properties().clone();
        let valid = render_function_module(&snapshot(&fixture)).unwrap();
        for field in ["processingType", "releaseState"] {
            for value in ["", " ", "NORMAL", "unknown", "0"] {
                let mut original = fixture.clone();
                if field == "processingType" {
                    original.processing_type = value.to_owned();
                } else {
                    original.release_state = value.to_owned();
                }
                let baseline = snapshot(&original);
                for result in [
                    render_function_module(&baseline).map(|_| ()),
                    merge_function_module(&baseline, &valid).map(|_| ()),
                ] {
                    assert!(matches!(result,
                        Err(ProjectionError::InvalidAffField { field: actual, .. }) if actual == field));
                }
            }
        }
    }

    #[test]
    fn optional_defaults_are_noops_but_active_unbacked_blocks_are_rejected() {
        let fixture = module_snapshot(MODULE_XML).properties().clone();
        for &previous_type in PROCESSING_TYPES {
            let mut original = fixture.clone();
            original.processing_type = previous_type.to_owned();
            let baseline = snapshot(&original);
            let mut defaults: Value =
                serde_json::from_str(&render_function_module(&baseline).unwrap()).unwrap();
            for field in [
                "global",
                "exceptionClasses",
                "activeFunctionExit",
                "notExecutable",
                "editLocked",
            ] {
                defaults[field] = json!(false);
            }
            defaults["application"] = json!("");
            defaults["client"] = json!("");
            defaults["parameters"] = json!([]);
            defaults["exceptions"] = json!([]);
            defaults["releaseState"] = json!("notReleased");
            assert_eq!(
                merge_function_module(&baseline, &defaults.to_string()).unwrap(),
                None
            );

            for (field, active_type, block) in [
                (
                    "rfcProperties",
                    "rfc",
                    json!({
                        "basxmlEnabled": false, "rfcScope": "notClassified", "rfcVersion": "any",
                        "abapFromJava": false, "javaFromAbap": false, "javaRemote": false
                    }),
                ),
                (
                    "updateProperties",
                    "update",
                    json!({"updateTaskKind": "startImmediately"}),
                ),
            ] {
                for &target_type in PROCESSING_TYPES {
                    let mut edited = defaults.clone();
                    edited["processingType"] = json!(target_type);
                    edited[field] = block.clone();
                    let result = merge_function_module(&baseline, &edited.to_string());
                    if previous_type == active_type || target_type == active_type {
                        assert!(matches!(result,
                            Err(ProjectionError::UnsupportedAffProperty { object_type: "FUNC", field: actual }) if actual == field));
                    } else {
                        let mut expected = original.clone();
                        expected.processing_type = target_type.to_owned();
                        assert_eq!(
                            result.unwrap(),
                            (expected != original).then(|| serde_json::to_value(expected).unwrap())
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn nondefault_unmapped_fields_report_precise_aff_paths() {
        let baseline = module_snapshot(MODULE_XML).into_erased();
        let document: Value =
            serde_json::from_str(&render_function_module(&baseline).unwrap()).unwrap();
        for (field, value) in [
            ("global", json!(true)),
            ("exceptionClasses", json!(true)),
            ("application", json!("A")),
            ("client", json!("100")),
            ("activeFunctionExit", json!(true)),
            ("notExecutable", json!(true)),
            ("editLocked", json!(true)),
            (
                "parameters",
                json!([{"name": "P", "description": "Parameter"}]),
            ),
            (
                "exceptions",
                json!([{"name": "E", "description": "Exception"}]),
            ),
        ] {
            let mut edited = document.clone();
            edited[field] = value;
            assert!(
                matches!(merge_function_module(&baseline, &edited.to_string()),
                Err(ProjectionError::UnsupportedAffProperty { object_type: "FUNC", field: actual }) if actual == field)
            );
        }
        for (field, value, path) in [
            ("basxmlEnabled", json!(true), "rfcProperties.basxmlEnabled"),
            (
                "rfcScope",
                json!("fromSameClientAndUser"),
                "rfcProperties.rfcScope",
            ),
            (
                "rfcScope",
                json!("fromSameSystem"),
                "rfcProperties.rfcScope",
            ),
            ("rfcScope", json!("fromAnySystem"), "rfcProperties.rfcScope"),
            (
                "rfcVersion",
                json!("fastSerializationRequired"),
                "rfcProperties.rfcVersion",
            ),
            ("abapFromJava", json!(true), "rfcProperties.abapFromJava"),
            ("javaFromAbap", json!(true), "rfcProperties.javaFromAbap"),
            ("javaRemote", json!(true), "rfcProperties.javaRemote"),
        ] {
            let mut edited = document.clone();
            edited["rfcProperties"] = serde_json::to_value(RfcProperties::default()).unwrap();
            edited["rfcProperties"][field] = value;
            let typed: ProjectedFunctionModuleProperties =
                serde_json::from_value(edited.clone()).unwrap();
            typed.validate().unwrap();
            assert!(
                matches!(merge_function_module(&baseline, &edited.to_string()),
                Err(ProjectionError::UnsupportedAffProperty { object_type: "FUNC", field }) if field == path)
            );
        }
        for kind in [
            "startDelayed",
            "startImmediatelyNoRestart",
            "collectiveRun",
            "unsupportedKind",
        ] {
            let mut edited = document.clone();
            edited["updateProperties"] = json!({"updateTaskKind": kind});
            let typed: ProjectedFunctionModuleProperties =
                serde_json::from_value(edited.clone()).unwrap();
            typed.validate().unwrap();
            assert!(matches!(
                merge_function_module(&baseline, &edited.to_string()),
                Err(ProjectionError::UnsupportedAffProperty {
                    object_type: "FUNC",
                    field: "updateProperties.updateTaskKind"
                })
            ));
        }
    }

    #[test]
    fn required_fields_unknown_fields_and_json_types_follow_the_schema() {
        let baseline = module_snapshot(MODULE_XML).into_erased();
        let mut document: Value =
            serde_json::from_str(&render_function_module(&baseline).unwrap()).unwrap();
        document["rfcProperties"] = serde_json::to_value(RfcProperties::default()).unwrap();
        document["updateProperties"] = serde_json::to_value(UpdateProperties::default()).unwrap();
        document["parameters"] = json!([{"name": "P", "description": "Parameter"}]);
        document["exceptions"] = json!([{"name": "E", "description": "Exception"}]);
        for (block, fields) in [
            (
                "",
                &["formatVersion", "header", "processingType", "includeNumber"][..],
            ),
            ("/header", &["description"][..]),
            (
                "/rfcProperties",
                &["basxmlEnabled", "rfcScope", "rfcVersion"][..],
            ),
            ("/updateProperties", &["updateTaskKind"][..]),
            ("/parameters/0", &["name", "description"][..]),
            ("/exceptions/0", &["name", "description"][..]),
        ] {
            for &field in fields {
                let mut edited = document.clone();
                edited
                    .pointer_mut(block)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                assert!(
                    matches!(
                        merge_function_module(&baseline, &edited.to_string()),
                        Err(ProjectionError::Json(_))
                    ),
                    "{block}/{field}"
                );
            }
            let mut edited = document.clone();
            edited.pointer_mut(block).unwrap()["unknown"] = json!(true);
            assert!(
                matches!(
                    merge_function_module(&baseline, &edited.to_string()),
                    Err(ProjectionError::Json(_))
                ),
                "{block}"
            );
        }
        for path in [
            "/processingType",
            "/releaseState",
            "/rfcProperties/rfcScope",
            "/rfcProperties/rfcVersion",
            "/updateProperties/updateTaskKind",
        ] {
            let mut edited = document.clone();
            edited["releaseState"] = json!("notReleased");
            *edited.pointer_mut(path).unwrap() = json!("unknown");
            assert!(
                matches!(
                    merge_function_module(&baseline, &edited.to_string()),
                    Err(ProjectionError::Json(_))
                ),
                "{path}"
            );
        }
        for field in [
            "formatVersion",
            "header",
            "processingType",
            "rfcProperties",
            "updateProperties",
            "releaseState",
            "releaseDate",
            "global",
            "exceptionClasses",
            "application",
            "client",
            "activeFunctionExit",
            "includeNumber",
            "notExecutable",
            "editLocked",
            "parameters",
            "exceptions",
        ] {
            let mut edited = document.clone();
            edited[field] = Value::Null;
            assert!(
                matches!(
                    merge_function_module(&baseline, &edited.to_string()),
                    Err(ProjectionError::Json(_))
                ),
                "{field}"
            );
        }
        let mut edited = document;
        edited["header"]["abapLanguageVersion"] = json!("standard");
        assert!(matches!(
            merge_function_module(&baseline, &edited.to_string()),
            Err(ProjectionError::Json(_))
        ));
    }

    #[test]
    fn validates_character_limits_and_whole_item_uniqueness_before_mapping() {
        let baseline = module_snapshot(MODULE_XML).into_erased();
        let mut document: Value =
            serde_json::from_str(&render_function_module(&baseline).unwrap()).unwrap();
        document["application"] = json!("");
        document["client"] = json!("");
        document["parameters"] = json!([{"name": "P", "description": "Parameter"}]);
        document["exceptions"] = json!([{"name": "E", "description": "Exception"}]);
        for (path, limit) in [
            ("/header/description", 74),
            ("/application", 1),
            ("/client", 3),
            ("/parameters/0/name", 30),
            ("/parameters/0/description", 79),
            ("/exceptions/0/name", 30),
            ("/exceptions/0/description", 79),
        ] {
            for length in [0, limit, limit + 1] {
                let mut edited = document.clone();
                *edited.pointer_mut(path).unwrap() = json!("\u{00e9}".repeat(length));
                let typed: ProjectedFunctionModuleProperties =
                    serde_json::from_value(edited.clone()).unwrap();
                assert_eq!(
                    typed.validate().is_ok(),
                    length <= limit,
                    "{path}: {length}"
                );
                if length > limit {
                    assert!(matches!(
                        merge_function_module(&baseline, &edited.to_string()),
                        Err(ProjectionError::Validation(_))
                    ));
                }
            }
        }
        for field in ["parameters", "exceptions"] {
            let mut edited = document.clone();
            edited[field] = json!([
                {"name": "SAME", "description": "First"},
                {"name": "SAME", "description": "First"}
            ]);
            assert!(matches!(
                merge_function_module(&baseline, &edited.to_string()),
                Err(ProjectionError::Validation(_))
            ));
            edited[field][1]["description"] = json!("Second");
            let typed: ProjectedFunctionModuleProperties = serde_json::from_value(edited).unwrap();
            typed.validate().unwrap();
        }
        let mut properties = baseline
            .typed_properties::<FunctionModule>()
            .unwrap()
            .clone();
        properties.description = "\u{00e9}".repeat(74);
        render_function_module(&snapshot(&properties)).unwrap();
        properties.description.push('x');
        assert!(matches!(
            render_function_module(&snapshot(&properties)),
            Err(ProjectionError::Validation(_))
        ));
        document["formatVersion"] = json!("2");
        assert!(matches!(
            merge_function_module(&baseline, &document.to_string()),
            Err(ProjectionError::Validation(_))
        ));
    }

    #[test]
    fn include_number_is_required_numeric_and_only_the_temp_placeholder_merges() {
        let baseline = module_snapshot(MODULE_XML).into_erased();
        let document: Value =
            serde_json::from_str(&render_function_module(&baseline).unwrap()).unwrap();
        for value in [
            "", "000", "-1", "+1", " 0", "1 ", "1\n", "1.0", "\u{0660}", "AB",
        ] {
            let mut edited = document.clone();
            edited["includeNumber"] = json!(value);
            assert!(
                matches!(
                    merge_function_module(&baseline, &edited.to_string()),
                    Err(ProjectionError::Validation(_))
                ),
                "{value:?}"
            );
        }
        for value in ["0", "1", "01", "10", "99"] {
            let mut edited = document.clone();
            edited["includeNumber"] = json!(value);
            assert!(matches!(
                merge_function_module(&baseline, &edited.to_string()),
                Err(ProjectionError::UnsupportedAffProperty {
                    object_type: "FUNC",
                    field: "includeNumber"
                })
            ));
        }
        let mut edited = document.clone();
        edited["includeNumber"] = json!(0);
        assert!(matches!(
            merge_function_module(&baseline, &edited.to_string()),
            Err(ProjectionError::Json(_))
        ));
        assert_eq!(
            merge_function_module(&baseline, &document.to_string()).unwrap(),
            None
        );
    }

    #[test]
    fn release_dates_are_validated_before_rejecting_the_unmapped_property() {
        let baseline = module_snapshot(MODULE_XML).into_erased();
        let document: Value =
            serde_json::from_str(&render_function_module(&baseline).unwrap()).unwrap();
        for value in [
            "",
            "2026-9-08",
            "2026-09-8",
            "2026-00-01",
            "2026-13-01",
            "2026-01-00",
            "2026-04-31",
            "2026-02-29",
            "1900-02-29",
            "2024-02-30",
            "2026-09-08Z",
            "2026-09-08T00:00:00Z",
            "2026/09/08",
            "\u{00e9}026-09-08",
        ] {
            let mut edited = document.clone();
            edited["releaseDate"] = json!(value);
            assert!(
                matches!(
                    merge_function_module(&baseline, &edited.to_string()),
                    Err(ProjectionError::Validation(_))
                ),
                "{value}"
            );
        }
        for value in [
            "2026-09-08",
            "2024-02-29",
            "2000-02-29",
            "0000-01-01",
            "9999-12-31",
        ] {
            let mut edited = document.clone();
            edited["releaseDate"] = json!(value);
            assert!(
                matches!(
                    merge_function_module(&baseline, &edited.to_string()),
                    Err(ProjectionError::UnsupportedAffProperty {
                        object_type: "FUNC",
                        field: "releaseDate"
                    })
                ),
                "{value}"
            );
        }
    }
}
