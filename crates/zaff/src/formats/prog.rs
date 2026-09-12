//! Program and standalone Include mapping to AFF `.prog.json` documents.
//!
//! Both object families use the PROG format, but their ADT property models differ.
//! [`ProgramProperties`] provides a program type and editor-lock flag.
//! [`IncludeProperties`] has no corresponding fields, so the AFF type is fixed
//! to `include` and the editor-lock flag has no implemented mapping.
//!
//! The mapping tables use AFF JSON paths and ADT Rust field names, not XML
//! attribute names. Fields listed as unsupported belong to the AFF model but
//! have no implemented ADT backing here. The `.prog.abap` file is bound
//! separately to the advertised main source.

use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{Include, IncludeProperties, ObjectSnapshot, ObjectType, Program, ProgramProperties};

use crate::{
    Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::is_false,
    models::{language_from_adt, language_to_adt},
    validate::one_of,
};

pub(crate) static PROGRAM_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "PROG",
    version: "1",
    workbench_types: &[Program::WORKBENCH_TYPE, Include::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.prog.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.prog.abap",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
        FileSpec::new(
            "<name>.prog.texts.<lang>.properties",
            Cardinality::ZeroOrMore,
            Mapping::Unavailable,
        ),
        FileSpec::new(
            "<name>.prog.headings.<lang>.properties",
            Cardinality::ZeroOrMore,
            Mapping::Unavailable,
        ),
        FileSpec::new(
            "<name>.prog.selections.<lang>.properties",
            Cardinality::ZeroOrMore,
            Mapping::Unavailable,
        ),
    ],
};

/// Renders ADT [`ProgramProperties`] or [`IncludeProperties`] as pretty-printed
/// AFF JSON with a trailing newline. Both use the PROG format.
fn render(snapshot: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let workbench_type = snapshot.reference().workbench_type();

    let document = if workbench_type == &Program::WORKBENCH_TYPE {
        ProjectedProgramProperties::from_program(snapshot.typed_properties::<Program>()?)?
    } else if workbench_type == &Include::WORKBENCH_TYPE {
        ProjectedProgramProperties::from_include(snapshot.typed_properties::<Include>()?)?
    } else {
        return Err(ProjectionError::UnsupportedRepositoryType {
            workbench_type: workbench_type.clone(),
        });
    };
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');
    Ok(content)
}

/// Validates edited AFF JSON and applies its changes to a copy of the snapshot
/// properties. Returns changed ADT wire JSON, or `None` for a validated no-op.
fn merge(
    snapshot: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let workbench_type = snapshot.reference().workbench_type();

    if workbench_type == &Program::WORKBENCH_TYPE {
        let original = snapshot.typed_properties::<Program>()?;
        let merged = merge_program_properties(original, edited)?;
        if merged == *original {
            return Ok(None);
        }
        serde_json::to_value(merged).map(Some).map_err(Into::into)
    } else if workbench_type == &Include::WORKBENCH_TYPE {
        let original = snapshot.typed_properties::<Include>()?;
        let merged = merge_include_properties(original, edited)?;
        if merged == *original {
            return Ok(None);
        }
        serde_json::to_value(merged).map(Some).map_err(Into::into)
    } else {
        Err(ProjectionError::UnsupportedRepositoryType {
            workbench_type: workbench_type.clone(),
        })
    }
}

/// Program and standalone Include properties represented by the AFF v1 JSON document.
///
/// # Document Layout
///
/// ```text
/// AFF block           Program ADT storage                 Include ADT storage
/// ---------           -------------------                 -------------------
/// formatVersion       No backing field, constant "1"      No backing field, constant "1"
/// header              ProgramProperties                   IncludeProperties
/// generalInformation  ProgramProperties                   IncludeProperties and fixed AFF values
/// logicalDatabase     ProgramProperties.logical_database  No implemented backing
/// ```
///
/// Both families expose description and original language through [`ProgramHeader`].
/// An absent Include description renders as an empty string. Unchanged edits
/// preserve absence, while adding or clearing a description writes an explicit string.
/// The supported general fields differ between them, as documented on
/// [`ProgramGeneralInformation`]. [`LogicalDatabase`] maps the Program database
/// assignment and selection screen.
///
/// The object name, package, links, users, timestamps, and other unrepresented
/// ADT metadata are not supplied by this document. They remain in the original
/// properties when represented fields are edited.
///
/// Source text belongs to the separate `.prog.abap` file. Language-dependent
/// text, heading, and selection `.properties` files are listed in the format
/// declaration but are not implemented.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedProgramProperties {
    #[garde(custom(one_of([PROGRAM_FORMAT.version()])))]
    pub format_version: String,

    #[garde(dive)]
    pub header: ProgramHeader,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub general_information: Option<ProgramGeneralInformation>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub logical_database: Option<LogicalDatabase>,
}

/// Common AFF Program header fields.
///
/// # Field Mapping
///
/// Both columns refer to top-level fields on the corresponding ADT model.
///
/// ```text
/// AFF field                ProgramProperties field  IncludeProperties field
/// ---------                -----------------------  -----------------------
/// header.description       description              description
/// header.originalLanguage  master_language          master_language
/// ```
///
/// Description text is copied directly. Original language is converted between
/// SAP codes in ADT and BCP47 tags in AFF, for example `EN` and `en`. Unsupported
/// codes or tags are rejected.
///
/// This AFF header has no `abapLanguageVersion` field. It does not change any
/// ABAP language-version value retained in the ADT properties.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProgramHeader {
    #[garde(length(chars, max = 70))]
    pub description: String,

    pub original_language: String,
}

/// General Program attributes represented by AFF v1.
///
/// # Supported Fields
///
/// All AFF paths below are inside `generalInformation`.
///
/// ```text
/// AFF field           ProgramProperties field              IncludeProperties field
/// ---------           -----------------------              -----------------------
/// programType         program_type                         No field, fixed to "include"
/// fixPointArithmetic  fix_point_arithmetic                 fix_point_arithmetic
/// editLocked          locked_by_editor                     No implemented backing
/// programStatus       source_object_status                 source_object_status
/// startsUsingVariant  start_using_variant                  No implemented backing
/// authorizationGroup  authorization_group.reference.name  No implemented backing
/// application         authorization_group.application     No implemented backing
/// ```
///
/// Program type uses the spellings documented on [`ProgramType`]. A Program
/// cannot be changed into a standalone Include through this field, and an
/// Include must keep `include` as its type.
///
/// The arithmetic flag is copied directly for both families. It does not change
/// the separate ADT `unicode_check_active` field. `editLocked` is copied to
/// `locked_by_editor` for Programs. It is an object property, not an ADT session
/// lock handle. Includes accept only false for this AFF field.
///
/// Missing optional Program values render as AFF defaults. Unchanged defaults
/// preserve absent ADT values. Actual edits write explicit values, including
/// false and empty strings when clearing a setting. Program status spellings
/// are documented on [`ProgramStatus`].
///
/// Authorization group names retain their reference metadata when unchanged.
/// A changed name replaces the reference with a name-only value. Editing the
/// application alone preserves the group reference. Includes accept only the
/// defaults for status, variant startup, authorization group, and application.
///
/// # Omitted Values
///
/// The default program type is `executableProgram`. Boolean fields default to
/// false and strings to empty. A Program with only default values omits the
/// whole block. An Include always includes the block because `include` is not
/// the default type.
///
/// Removing a supported Program field applies its AFF default. For example,
/// removing a true `editLocked` value writes false to ADT `locked_by_editor`.
/// Removing the whole block from an Include is rejected because the resulting
/// default type would be `executableProgram`, not `include`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProgramGeneralInformation {
    #[serde(default, skip_serializing_if = "ProgramType::is_default")]
    pub program_type: ProgramType,

    #[serde(default, skip_serializing_if = "ProgramStatus::is_default")]
    pub program_status: ProgramStatus,

    #[serde(default, skip_serializing_if = "is_false")]
    pub fix_point_arithmetic: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub edit_locked: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub starts_using_variant: bool,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 8))]
    pub authorization_group: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 1))]
    pub application: String,
}

impl ProgramGeneralInformation {
    fn is_empty(&self) -> bool {
        self.program_type.is_default()
            && self.program_status.is_default()
            && !self.fix_point_arithmetic
            && !self.edit_locked
            && !self.starts_using_variant
            && self.authorization_group.is_empty()
            && self.application.is_empty()
    }
}

/// The AFF Program kind, including the standalone Include discriminator.
///
/// # Program Type Mapping
///
/// These spellings map directly between AFF `generalInformation.programType`
/// and ADT `ProgramProperties.program_type`.
///
/// ```text
/// AFF value          ADT program_type
/// ---------          ----------------
/// executableProgram  executableProgram
/// modulePool         modulePool
/// subroutinePool     subroutinePool
/// include            include
/// ```
///
/// The conversion recognizes all four spellings, but writes remain constrained
/// by object family. A Program edit cannot select `include`. A standalone
/// Include uses a fixed AFF `include` value rather than reading an ADT field,
/// and edits must retain that value.
///
/// `executableProgram` is the default and is omitted from AFF JSON.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProgramType {
    #[default]
    #[serde(rename = "executableProgram")]
    ExecutableProgram,
    #[serde(rename = "modulePool")]
    ModulePool,
    #[serde(rename = "subroutinePool")]
    SubroutinePool,
    #[serde(rename = "include")]
    Include,
}

impl ProgramType {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "executableProgram" => Ok(Self::ExecutableProgram),
            "modulePool" => Ok(Self::ModulePool),
            "subroutinePool" => Ok(Self::SubroutinePool),
            "include" => Ok(Self::Include),
            value => Err(ProjectionError::InvalidAffField {
                field: "generalInformation.programType",
                message: format!("unsupported ADT program type `{value}`"),
            }),
        }
    }

    const fn adt_value(self) -> &'static str {
        match self {
            Self::ExecutableProgram => "executableProgram",
            Self::ModulePool => "modulePool",
            Self::SubroutinePool => "subroutinePool",
            Self::Include => "include",
        }
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::ExecutableProgram)
    }
}

/// Program status vocabulary in AFF.
///
/// Maps `generalInformation.programStatus` to Program `source_object_status`.
///
/// ```text
/// AFF value                  ADT value
/// ---------                  ---------
/// sapProductionProgram       SAPStandardProduction
/// customerProductionProgram  customerProduction
/// systemProgram              system
/// testProgram                test
/// unknown                    Attribute omitted when clearing
/// ```
///
/// Absent, empty, or literal `unknown` ADT values render as `unknown`. Unchanged
/// values retain their original representation. Other ADT strings are rejected. Standalone
/// Includes use the same source status mapping.
/// SADT_ABAP_SOURCE_MAIN_OBJECT has no mapping for the literal `unknown`. Clearing
/// status must omit the attribute rather than sending a word to the internal
/// one-character status field.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProgramStatus {
    #[serde(rename = "sapProductionProgram")]
    SapProductionProgram,
    #[serde(rename = "customerProductionProgram")]
    CustomerProductionProgram,
    #[serde(rename = "systemProgram")]
    SystemProgram,
    #[serde(rename = "testProgram")]
    TestProgram,
    #[default]
    #[serde(rename = "unknown")]
    Unknown,
}

impl ProgramStatus {
    fn from_adt(value: Option<&zadt::SourceObjectStatus>) -> Result<Self, ProjectionError> {
        use zadt::SourceObjectStatus;
        match value {
            Some(SourceObjectStatus::SapStandardProduction) => Ok(Self::SapProductionProgram),
            Some(SourceObjectStatus::CustomerProduction) => Ok(Self::CustomerProductionProgram),
            Some(SourceObjectStatus::System) => Ok(Self::SystemProgram),
            Some(SourceObjectStatus::Test) => Ok(Self::TestProgram),
            None => Ok(Self::Unknown),
            Some(SourceObjectStatus::Other(value)) if value.is_empty() || value == "unknown" => {
                Ok(Self::Unknown)
            }
            Some(SourceObjectStatus::Other(value)) => Err(ProjectionError::InvalidAffField {
                field: "generalInformation.programStatus",
                message: format!("unsupported ADT source object status `{value}`"),
            }),
        }
    }

    const fn adt_value(self) -> Option<zadt::SourceObjectStatus> {
        Some(match self {
            Self::SapProductionProgram => zadt::SourceObjectStatus::SapStandardProduction,
            Self::CustomerProductionProgram => zadt::SourceObjectStatus::CustomerProduction,
            Self::SystemProgram => zadt::SourceObjectStatus::System,
            Self::TestProgram => zadt::SourceObjectStatus::Test,
            Self::Unknown => return None,
        })
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

/// AFF logical-database assignment for an executable Program.
///
/// # Field Mapping
///
/// ```text
/// AFF field                        ProgramProperties field
/// ---------                        -----------------------
/// logicalDatabase.name             logical_database.reference.name
/// logicalDatabase.selectionScreen  logical_database.selection_screen
/// ```
///
/// Missing values render as empty strings, and an empty block is omitted.
/// Unchanged fields preserve absent values, explicit empty strings, and reference
/// metadata. Renaming the database replaces its reference with a name-only value.
/// Removing a populated block clears the ADT assignment. Standalone Includes
/// accept only an absent or empty block.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalDatabase {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 20))]
    pub name: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 3))]
    pub selection_screen: String,
}

impl LogicalDatabase {
    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.selection_screen.is_empty()
    }
}

/// Maps AFF header and supported general-information fields to [`ProgramProperties`].
pub(crate) fn merge_program_properties(
    original: &ProgramProperties,
    edited: &str,
) -> Result<ProgramProperties, ProjectionError> {
    let edited = parse(edited)?;
    let edited_general = edited.general_information.unwrap_or_default();
    if edited_general.program_type == ProgramType::Include {
        return Err(ProjectionError::InvalidAffField {
            field: "generalInformation.programType",
            message: "a Program cannot be changed into a standalone Include".to_owned(),
        });
    }

    // Reject unprojectable baselines before replacing their represented fields.
    let baseline = ProjectedProgramProperties::from_program(original)?;
    let baseline_general = baseline.general_information.unwrap_or_default();
    let mut merged = original.clone();
    // AFF header.description      -> ADT description
    // AFF header.originalLanguage -> ADT master_language, with language-code conversion
    if edited.header.description != original.description.as_deref().unwrap_or_default() {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    // Within AFF generalInformation:
    //
    //   programType         -> ADT program_type
    //   fixPointArithmetic  -> ADT fix_point_arithmetic
    //   editLocked          -> ADT locked_by_editor
    merged.program_type = edited_general.program_type.adt_value().to_owned();
    merged.fix_point_arithmetic = edited_general.fix_point_arithmetic;
    merged.locked_by_editor = edited_general.edit_locked;
    if edited_general.program_status != baseline_general.program_status {
        merged.source_object_status = edited_general.program_status.adt_value();
    }
    if edited_general.starts_using_variant != baseline_general.starts_using_variant {
        merged.start_using_variant = Some(edited_general.starts_using_variant);
    }
    if edited_general.authorization_group != baseline_general.authorization_group
        || edited_general.application != baseline_general.application
    {
        let group = merged
            .authorization_group
            .get_or_insert_with(|| zadt::AuthorizationGroup {
                application: None,
                reference: Default::default(),
            });
        if edited_general.authorization_group != baseline_general.authorization_group {
            group.reference = zadt::AdvertisedObjectReference {
                name: Some(edited_general.authorization_group),
                ..Default::default()
            };
        }
        if edited_general.application != baseline_general.application {
            group.application = Some(edited_general.application);
        }
    }

    let database = edited.logical_database.unwrap_or_default();
    let baseline_database = baseline.logical_database.unwrap_or_default();
    if database.name != baseline_database.name
        || database.selection_screen != baseline_database.selection_screen
    {
        if database.is_empty() {
            merged.logical_database = None;
        } else {
            let target = merged
                .logical_database
                .get_or_insert_with(|| zadt::LogicalDatabase {
                    selection_screen: None,
                    reference: Default::default(),
                });
            if database.name != baseline_database.name {
                target.reference = zadt::AdvertisedObjectReference {
                    name: Some(database.name),
                    ..Default::default()
                };
            }
            if database.selection_screen != baseline_database.selection_screen {
                target.selection_screen = Some(database.selection_screen);
            }
        }
    }
    Ok(merged)
}

/// Maps AFF description, original language, and arithmetic to [`IncludeProperties`].
pub(crate) fn merge_include_properties(
    original: &IncludeProperties,
    edited: &str,
) -> Result<IncludeProperties, ProjectionError> {
    let edited = parse(edited)?;
    let edited_general = edited.general_information.unwrap_or_default();
    validate_include_fields(&edited_general, edited.logical_database.as_ref())?;
    if edited_general.program_type != ProgramType::Include {
        return Err(ProjectionError::InvalidAffField {
            field: "generalInformation.programType",
            message: "a standalone Include must use program type `include`".to_owned(),
        });
    }
    if edited_general.edit_locked {
        return Err(unsupported("generalInformation.editLocked"));
    }

    let previous = ProjectedProgramProperties::from_include(original)?;
    let mut merged = original.clone();
    if edited_general.program_status
        != previous
            .general_information
            .unwrap_or_default()
            .program_status
    {
        merged.source_object_status = edited_general.program_status.adt_value();
    }
    // Preserve absent versus explicitly empty include descriptions on a no-op.
    if edited.header.description != previous.header.description {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    // AFF generalInformation.fixPointArithmetic -> ADT fix_point_arithmetic.
    // Program type is fixed to include and editLocked has no backing here.
    merged.fix_point_arithmetic = edited_general.fix_point_arithmetic;
    Ok(merged)
}

impl ProjectedProgramProperties {
    fn from_program(properties: &ProgramProperties) -> Result<Self, ProjectionError> {
        let group = properties.authorization_group.as_ref();
        let general = ProgramGeneralInformation {
            program_type: ProgramType::from_adt(&properties.program_type)?,
            fix_point_arithmetic: properties.fix_point_arithmetic,
            edit_locked: properties.locked_by_editor,
            program_status: ProgramStatus::from_adt(properties.source_object_status.as_ref())?,
            starts_using_variant: properties.start_using_variant.unwrap_or_default(),
            authorization_group: group
                .and_then(|group| group.reference.name.clone())
                .unwrap_or_default(),
            application: group
                .and_then(|group| group.application.clone())
                .unwrap_or_default(),
        };
        let document = Self {
            format_version: PROGRAM_FORMAT.version().to_owned(),
            header: ProgramHeader {
                description: properties.description.clone().unwrap_or_default(),
                original_language: language_from_adt(
                    &properties.master_language,
                    "header.originalLanguage",
                )?,
            },
            general_information: (!general.is_empty()).then_some(general),
            logical_database: properties
                .logical_database
                .as_ref()
                .map(|database| LogicalDatabase {
                    name: database.reference.name.clone().unwrap_or_default(),
                    selection_screen: database.selection_screen.clone().unwrap_or_default(),
                })
                .filter(|database| !database.is_empty()),
        };
        document.validate()?;
        Ok(document)
    }

    fn from_include(properties: &IncludeProperties) -> Result<Self, ProjectionError> {
        // IncludeProperties supplies arithmetic but no program-type field.
        // The fixed include value identifies the family in the shared AFF format.
        let general = ProgramGeneralInformation {
            program_type: ProgramType::Include,
            fix_point_arithmetic: properties.fix_point_arithmetic,
            program_status: ProgramStatus::from_adt(properties.source_object_status.as_ref())?,
            ..Default::default()
        };
        let document = Self {
            format_version: PROGRAM_FORMAT.version().to_owned(),
            header: ProgramHeader {
                description: properties.description.clone().unwrap_or_default(),
                original_language: language_from_adt(
                    &properties.master_language,
                    "header.originalLanguage",
                )?,
            },
            general_information: Some(general),
            logical_database: None,
        };
        document.validate()?;
        Ok(document)
    }
}

fn parse(content: &str) -> Result<ProjectedProgramProperties, ProjectionError> {
    let document: ProjectedProgramProperties = serde_json::from_str(content)?;
    document.validate()?;
    Ok(document)
}

fn validate_include_fields(
    general: &ProgramGeneralInformation,
    database: Option<&LogicalDatabase>,
) -> Result<(), ProjectionError> {
    if general.starts_using_variant {
        return Err(unsupported("generalInformation.startsUsingVariant"));
    }
    if !general.authorization_group.is_empty() {
        return Err(unsupported("generalInformation.authorizationGroup"));
    }
    if !general.application.is_empty() {
        return Err(unsupported("generalInformation.application"));
    }
    if database.is_some_and(|database| !database.is_empty()) {
        return Err(unsupported("logicalDatabase"));
    }
    Ok(())
}

fn unsupported(field: &'static str) -> ProjectionError {
    ProjectionError::UnsupportedAffProperty {
        object_type: "PROG",
        field,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use zadt::{Include, IncludeProperties, Program, ProgramProperties};

    use super::*;

    const PROGRAM_XML: &[u8] = include_bytes!("../../../zadt/tests/fixtures/program-z-test.xml");
    const INCLUDE_XML: &[u8] = include_bytes!("../../../zadt/tests/fixtures/include-ztest.xml");

    fn program() -> ObjectSnapshot<()> {
        let reference = crate::test_support::reference::<Program>(
            "Z_TEST",
            "/sap/bc/adt/programs/programs/z_test",
        );
        crate::test_support::properties(
            &reference,
            Program::MEDIA_TYPES[0],
            "program-etag",
            PROGRAM_XML,
        )
        .into_erased()
    }

    fn include() -> ObjectSnapshot<()> {
        let reference = crate::test_support::reference::<Include>(
            "ZTEST",
            "/sap/bc/adt/programs/includes/ztest",
        );
        crate::test_support::properties(
            &reference,
            Include::MEDIA_TYPES[0],
            "include-etag",
            INCLUDE_XML,
        )
        .into_erased()
    }

    #[test]
    fn live_include_status_and_missing_description_project_and_merge() {
        let reference = crate::test_support::reference::<Include>(
            "/LIME/COLLECTION_DELETE_I01",
            "/sap/bc/adt/programs/includes/%2flime%2fcollection_delete_i01",
        );
        let obj = crate::test_support::properties(
            &reference,
            Include::MEDIA_TYPES[0],
            "etag",
            include_bytes!("../../../zadt/tests/fixtures/include-lime-collection-delete.xml"),
        )
        .into_erased();
        let content = render(&obj).unwrap();
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(edited["header"]["description"], "");
        assert_eq!(
            edited["generalInformation"]["programStatus"],
            "sapProductionProgram"
        );
        assert_eq!(merge(&obj, &content).unwrap(), None);
        edited["generalInformation"]["programStatus"] = "testProgram".into();
        let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
        assert_eq!(payload["@abapsource:sourceObjectStatus"], "test");
        assert!(payload.get("@adtcore:description").is_none());
        edited["generalInformation"]["programStatus"] = "unknown".into();
        let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
        assert!(payload.get("@abapsource:sourceObjectStatus").is_none());
    }

    #[test]
    fn optional_include_descriptions_preserve_noops_and_support_edits() {
        use zadt::ToXml;

        let baseline = include();
        let reference = crate::test_support::reference::<Include>(
            "ZTEST",
            "/sap/bc/adt/programs/includes/ztest",
        );
        for description in [None, Some(""), Some("Existing description")] {
            let mut original = baseline.typed_properties::<Include>().unwrap().clone();
            original.description = description.map(str::to_owned);
            let obj = crate::test_support::properties(
                &reference,
                Include::MEDIA_TYPES[0],
                "etag",
                &original.to_xml().unwrap(),
            )
            .into_erased();
            let content = render(&obj).unwrap();
            let mut edited: Value = serde_json::from_str(&content).unwrap();
            assert_eq!(
                edited["header"]["description"],
                description.unwrap_or_default()
            );
            assert_eq!(merge(&obj, &content).unwrap(), None);
            edited["header"]["originalLanguage"] = "de".into();
            let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
            let mut expected = original.clone();
            expected.master_language = "DE".into();
            assert_eq!(payload, serde_json::to_value(&expected).unwrap());
            for new_description in ["New description", ""] {
                edited["header"]["description"] = new_description.into();
                let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
                expected.description = if new_description == description.unwrap_or_default() {
                    original.description.clone()
                } else {
                    Some(new_description.into())
                };
                assert_eq!(payload, serde_json::to_value(&expected).unwrap());
            }
        }
    }

    #[test]
    fn renders_program_and_include_metadata_as_aff_v1() {
        let program = program();
        let program_json = render(&program).unwrap();
        let program_document: Value = serde_json::from_str(&program_json).unwrap();

        assert_eq!(program_document["formatVersion"], "1");
        assert_eq!(program_document["header"]["description"], "dwadwad");
        assert_eq!(program_document["header"]["originalLanguage"], "en");
        assert_eq!(
            program_document["generalInformation"]["fixPointArithmetic"],
            true
        );
        assert!(
            program_document["generalInformation"]
                .get("programType")
                .is_none()
        );
        assert!(program_document.get("logicalDatabase").is_none());

        let include = include();
        let include_json = render(&include).unwrap();
        let include_document: Value = serde_json::from_str(&include_json).unwrap();
        assert_eq!(
            include_document["generalInformation"]["programType"],
            "include"
        );
    }

    #[test]
    fn no_op_program_merge_returns_no_payload() {
        let original = program();
        let content = render(&original).unwrap();
        assert_eq!(merge(&original, &content).unwrap(), None);
    }

    #[test]
    fn no_op_include_merge_returns_no_payload() {
        let original = include();
        let content = render(&original).unwrap();
        assert_eq!(merge(&original, &content).unwrap(), None);
    }

    #[test]
    fn merges_program_edits_without_losing_the_adt_envelope() {
        let original = program();
        let mut edited: ProjectedProgramProperties =
            serde_json::from_str(&render(&original).unwrap()).unwrap();
        edited.header.description = "Updated program".to_owned();
        edited.header.original_language = "de-CH".to_owned();
        let general = edited.general_information.get_or_insert_default();
        general.program_type = ProgramType::ModulePool;
        general.fix_point_arithmetic = false;
        general.edit_locked = true;

        let merged: ProgramProperties = serde_json::from_value(
            merge(&original, &serde_json::to_string(&edited).unwrap())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let original = original.typed_properties::<Program>().unwrap();

        assert_eq!(merged.description.as_deref(), Some("Updated program"));
        assert_eq!(merged.master_language, "4G");
        assert_eq!(merged.program_type, "modulePool");
        assert!(!merged.fix_point_arithmetic);
        assert!(merged.locked_by_editor);
        assert_eq!(merged.package, original.package);
        assert_eq!(merged.links, original.links);
        assert_eq!(merged.syntax_configuration, original.syntax_configuration);
    }

    #[test]
    fn rejects_changing_program_into_include() {
        let original = program();
        let mut edited: ProjectedProgramProperties =
            serde_json::from_str(&render(&original).unwrap()).unwrap();
        edited
            .general_information
            .get_or_insert_default()
            .program_type = ProgramType::Include;

        assert!(matches!(
            merge(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::InvalidAffField {
                field: "generalInformation.programType",
                ..
            })
        ));
    }

    #[test]
    fn program_description_edits_preserve_optional_adt_settings() {
        let reference = crate::test_support::reference::<Program>(
            "ZTFTFRT",
            "/sap/bc/adt/programs/programs/ztftfrt",
        );
        let original = crate::test_support::properties(
            &reference,
            Program::MEDIA_TYPES[0],
            "program-etag",
            include_bytes!("../../../zadt/tests/fixtures/program-ztftfrt.xml"),
        )
        .into_erased();
        let content = render(&original).unwrap();
        assert_eq!(merge(&original, &content).unwrap(), None);
        let mut edited: ProjectedProgramProperties = serde_json::from_str(&content).unwrap();
        edited.header.description = "Updated description".to_owned();
        let merged: ProgramProperties = serde_json::from_value(
            merge(&original, &serde_json::to_string(&edited).unwrap())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let mut expected = original.typed_properties::<Program>().unwrap().clone();
        expected.description = Some(edited.header.description);
        assert_eq!(merged, expected);
    }

    #[test]
    fn merges_include_edits_and_requires_the_include_discriminator() {
        let original = include();
        let mut edited: ProjectedProgramProperties =
            serde_json::from_str(&render(&original).unwrap()).unwrap();
        edited.header.description = "Updated include".to_owned();
        edited.header.original_language = "zh-Hant".to_owned();
        edited
            .general_information
            .as_mut()
            .unwrap()
            .fix_point_arithmetic = true;

        let merged: IncludeProperties = serde_json::from_value(
            merge(&original, &serde_json::to_string(&edited).unwrap())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(merged.description.as_deref(), Some("Updated include"));
        assert_eq!(merged.master_language, "ZF");
        assert!(merged.fix_point_arithmetic);
        assert_eq!(
            merged.links,
            original.typed_properties::<Include>().unwrap().links
        );

        edited.general_information.as_mut().unwrap().program_type = ProgramType::ExecutableProgram;
        assert!(matches!(
            merge(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::InvalidAffField {
                field: "generalInformation.programType",
                ..
            })
        ));
    }

    #[test]
    fn rejects_program_fields_not_available_from_include_properties() {
        let original = include();
        let document: Value = serde_json::from_str(&render(&original).unwrap()).unwrap();
        for (field, value) in [
            ("startsUsingVariant", serde_json::json!(true)),
            ("authorizationGroup", serde_json::json!("ZGROUP")),
            ("application", serde_json::json!("*")),
        ] {
            let mut edited = document.clone();
            edited["generalInformation"][field] = value;
            assert!(matches!(
                merge(&original, &edited.to_string()),
                Err(ProjectionError::UnsupportedAffProperty { .. })
            ));
        }
        let mut edited = document;
        edited["logicalDatabase"] = serde_json::json!({"name": "ZDB"});
        assert!(matches!(
            merge(&original, &edited.to_string()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "logicalDatabase",
                ..
            })
        ));
    }

    #[test]
    fn maps_program_settings_and_clears_them_through_aff_defaults() {
        let original = <ProgramProperties as zadt::XmlCodec>::from_xml(include_bytes!(
            "../../../zadt/tests/fixtures/program-ztftfrt.xml"
        ))
        .unwrap();
        let mut document = ProjectedProgramProperties::from_program(&original).unwrap();
        let general = document.general_information.as_mut().unwrap();
        assert_eq!(
            general.program_status,
            ProgramStatus::CustomerProductionProgram
        );
        assert!(general.starts_using_variant);
        assert_eq!(general.authorization_group, "BCVADMIN");
        assert_eq!(general.application, "*");
        assert_eq!(document.logical_database.as_ref().unwrap().name, "D$S");
        assert_eq!(
            document.logical_database.as_ref().unwrap().selection_screen,
            ""
        );
        general.program_status = ProgramStatus::TestProgram;
        general.starts_using_variant = false;
        general.authorization_group = "ZGROUP".into();
        general.application = "S".into();
        document.logical_database = Some(LogicalDatabase {
            name: "ZDB".into(),
            selection_screen: "100".into(),
        });
        let merged =
            merge_program_properties(&original, &serde_json::to_string(&document).unwrap())
                .unwrap();
        assert_eq!(
            merged.source_object_status,
            Some(zadt::SourceObjectStatus::Test)
        );
        assert_eq!(merged.start_using_variant, Some(false));
        let group = merged.authorization_group.as_ref().unwrap();
        assert_eq!(group.reference.name.as_deref(), Some("ZGROUP"));
        assert_eq!(group.application.as_deref(), Some("S"));
        let database = merged.logical_database.as_ref().unwrap();
        assert_eq!(database.reference.name.as_deref(), Some("ZDB"));
        assert_eq!(database.selection_screen.as_deref(), Some("100"));
        assert_eq!(
            serde_json::to_value(ProjectedProgramProperties::from_program(&merged).unwrap())
                .unwrap(),
            serde_json::to_value(&document).unwrap()
        );

        let mut cleared = serde_json::to_value(&document).unwrap();
        cleared.as_object_mut().unwrap().remove("logicalDatabase");
        for field in [
            "programStatus",
            "startsUsingVariant",
            "authorizationGroup",
            "application",
        ] {
            cleared["generalInformation"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        let cleared = merge_program_properties(&merged, &cleared.to_string()).unwrap();
        assert_eq!(cleared.source_object_status, None);
        assert_eq!(cleared.start_using_variant, Some(false));
        assert_eq!(cleared.logical_database, None);
        let group = cleared.authorization_group.unwrap();
        assert_eq!(group.reference.name.as_deref(), Some(""));
        assert_eq!(group.application.as_deref(), Some(""));
    }

    #[test]
    fn program_settings_preserve_sparse_values_and_reference_metadata() {
        let mut original = program().typed_properties::<Program>().unwrap().clone();
        let reference = zadt::AdvertisedObjectReference {
            name: Some("ZREF".into()),
            uri: Some("reference/location".into()),
            description: Some("Reference description".into()),
            ..Default::default()
        };
        original.authorization_group = Some(zadt::AuthorizationGroup {
            application: None,
            reference: reference.clone(),
        });
        original.logical_database = Some(zadt::LogicalDatabase {
            selection_screen: Some("".into()),
            reference: reference.clone(),
        });
        let mut document = ProjectedProgramProperties::from_program(&original).unwrap();
        assert_eq!(
            merge_program_properties(&original, &serde_json::to_string(&document).unwrap())
                .unwrap(),
            original
        );
        document.general_information.as_mut().unwrap().application = "*".into();
        document.logical_database.as_mut().unwrap().selection_screen = "900".into();
        let merged =
            merge_program_properties(&original, &serde_json::to_string(&document).unwrap())
                .unwrap();
        assert_eq!(
            merged.authorization_group.as_ref().unwrap().reference,
            reference
        );
        assert_eq!(
            merged.logical_database.as_ref().unwrap().reference,
            reference
        );
        assert_eq!(merged.start_using_variant, None);
        assert_eq!(merged.source_object_status, None);
        document
            .general_information
            .as_mut()
            .unwrap()
            .authorization_group = "ZOTHER".into();
        document.logical_database.as_mut().unwrap().name = "ZOTHER".into();
        let renamed =
            merge_program_properties(&merged, &serde_json::to_string(&document).unwrap()).unwrap();
        let expected = zadt::AdvertisedObjectReference {
            name: Some("ZOTHER".into()),
            ..Default::default()
        };
        assert_eq!(renamed.authorization_group.unwrap().reference, expected);
        assert_eq!(renamed.logical_database.unwrap().reference, expected);

        let absent = program().typed_properties::<Program>().unwrap().clone();
        let added =
            merge_program_properties(&absent, &serde_json::to_string(&document).unwrap()).unwrap();
        assert_eq!(added.authorization_group.unwrap().reference, expected);
        assert_eq!(added.logical_database.unwrap().reference, expected);
    }

    #[test]
    fn program_status_mappings_preserve_baselines_and_reject_unknown_values() {
        let mut original = program().typed_properties::<Program>().unwrap().clone();
        for (adt, aff) in [
            ("SAPStandardProduction", ProgramStatus::SapProductionProgram),
            (
                "customerProduction",
                ProgramStatus::CustomerProductionProgram,
            ),
            ("system", ProgramStatus::SystemProgram),
            ("test", ProgramStatus::TestProgram),
            ("unknown", ProgramStatus::Unknown),
            ("", ProgramStatus::Unknown),
        ] {
            original.source_object_status = Some(adt.into());
            let document = ProjectedProgramProperties::from_program(&original).unwrap();
            assert_eq!(
                document
                    .general_information
                    .as_ref()
                    .unwrap()
                    .program_status,
                aff
            );
            assert_eq!(
                merge_program_properties(&original, &serde_json::to_string(&document).unwrap())
                    .unwrap(),
                original
            );
            let mut changed = document;
            changed.general_information.as_mut().unwrap().program_status =
                ProgramStatus::CustomerProductionProgram;
            if aff == ProgramStatus::CustomerProductionProgram {
                changed.general_information.as_mut().unwrap().program_status =
                    ProgramStatus::SapProductionProgram;
            }
            let merged =
                merge_program_properties(&original, &serde_json::to_string(&changed).unwrap())
                    .unwrap();
            assert_eq!(
                merged.source_object_status,
                changed
                    .general_information
                    .unwrap()
                    .program_status
                    .adt_value()
            );
        }
        original.source_object_status = Some("futureStatus".into());
        assert!(ProjectedProgramProperties::from_program(&original).is_err());
    }
}
