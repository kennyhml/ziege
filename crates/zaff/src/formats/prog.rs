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
/// AFF block           Program ADT storage             Include ADT storage
/// ---------           -------------------             -------------------
/// formatVersion       No backing field, constant "1"  No backing field, constant "1"
/// header              ProgramProperties               IncludeProperties
/// generalInformation  ProgramProperties               IncludeProperties and fixed AFF values
/// logicalDatabase     No implemented backing          No implemented backing
/// ```
///
/// Both families expose description and original language through [`ProgramHeader`].
/// The supported general fields differ between them, as documented on
/// [`ProgramGeneralInformation`]. [`LogicalDatabase`] describes valid AFF fields
/// with no implemented ADT mapping.
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
/// AFF field           ProgramProperties field  IncludeProperties field
/// ---------           -----------------------  -----------------------
/// programType         program_type             No field, fixed to "include"
/// fixPointArithmetic  fix_point_arithmetic     fix_point_arithmetic
/// editLocked          locked_by_editor         No implemented backing
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
/// # Fields Without An Implemented Mapping
///
/// ```text
/// AFF field           Accepted value  ADT backing
/// ---------           --------------  -----------
/// programStatus       unknown         Not implemented
/// startsUsingVariant  false           Not implemented
/// authorizationGroup  Empty string    Not implemented
/// application         Empty string    Not implemented
/// ```
///
/// These values are omitted from generated JSON. Supplying their defaults
/// explicitly is accepted, but nondefault edits are rejected rather than ignored.
/// This does not imply that every SAP endpoint lacks these fields, only that
/// this projection does not map them.
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
/// `generalInformation.programStatus` has no implemented ADT backing for either
/// family. The model represents the AFF vocabulary, but only `unknown` is
/// accepted by the mapping.
///
/// ```text
/// AFF value                  Mapping support
/// ---------                  ---------------
/// sapProductionProgram       Unsupported
/// customerProductionProgram  Unsupported
/// systemProgram              Unsupported
/// testProgram                Unsupported
/// unknown                    Accepted default, no ADT field is changed
/// ```
///
/// Generated documents omit the default `unknown` value.
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
    const fn is_default(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

/// AFF logical-database assignment for an executable Program.
///
/// # Field Mapping
///
/// ```text
/// AFF field                        ADT backing
/// ---------                        -----------
/// logicalDatabase.name             Not implemented
/// logicalDatabase.selectionScreen  Not implemented
/// ```
///
/// Neither Program nor Include projections currently supply these values.
/// The block is omitted from generated JSON. An absent block or a block with
/// both strings empty is accepted. A nonempty name or selection screen is
/// rejected, even if it passes AFF schema validation.
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
    validate_program_fields(&edited_general, edited.logical_database.as_ref())?;
    if edited_general.program_type == ProgramType::Include {
        return Err(ProjectionError::InvalidAffField {
            field: "generalInformation.programType",
            message: "a Program cannot be changed into a standalone Include".to_owned(),
        });
    }

    // Reject unprojectable baselines before replacing their represented fields.
    ProjectedProgramProperties::from_program(original)?;
    let mut merged = original.clone();
    // AFF header.description      -> ADT description
    // AFF header.originalLanguage -> ADT master_language, with language-code conversion
    merged.description = edited.header.description;
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
    Ok(merged)
}

/// Maps AFF description, original language, and arithmetic to [`IncludeProperties`].
pub(crate) fn merge_include_properties(
    original: &IncludeProperties,
    edited: &str,
) -> Result<IncludeProperties, ProjectionError> {
    let edited = parse(edited)?;
    let edited_general = edited.general_information.unwrap_or_default();
    validate_program_fields(&edited_general, edited.logical_database.as_ref())?;
    if edited_general.program_type != ProgramType::Include {
        return Err(ProjectionError::InvalidAffField {
            field: "generalInformation.programType",
            message: "a standalone Include must use program type `include`".to_owned(),
        });
    }
    if edited_general.edit_locked {
        return Err(unsupported("generalInformation.editLocked"));
    }

    ProjectedProgramProperties::from_include(original)?;
    let mut merged = original.clone();
    // AFF header.description      -> ADT description
    // AFF header.originalLanguage -> ADT master_language, with language-code conversion
    merged.description = edited.header.description;
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    // AFF generalInformation.fixPointArithmetic -> ADT fix_point_arithmetic.
    // Program type is fixed to include and editLocked has no backing here.
    merged.fix_point_arithmetic = edited_general.fix_point_arithmetic;
    Ok(merged)
}

impl ProjectedProgramProperties {
    fn from_program(properties: &ProgramProperties) -> Result<Self, ProjectionError> {
        // Only these three general-information fields have Program ADT backings.
        // The other AFF fields retain defaults, not values inferred from source text.
        let general = ProgramGeneralInformation {
            program_type: ProgramType::from_adt(&properties.program_type)?,
            fix_point_arithmetic: properties.fix_point_arithmetic,
            edit_locked: properties.locked_by_editor,
            ..Default::default()
        };
        let document = Self {
            format_version: PROGRAM_FORMAT.version().to_owned(),
            header: ProgramHeader {
                description: properties.description.clone(),
                original_language: language_from_adt(
                    &properties.master_language,
                    "header.originalLanguage",
                )?,
            },
            general_information: (!general.is_empty()).then_some(general),
            logical_database: None,
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
            ..Default::default()
        };
        let document = Self {
            format_version: PROGRAM_FORMAT.version().to_owned(),
            header: ProgramHeader {
                description: properties.description.clone(),
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

fn validate_program_fields(
    general: &ProgramGeneralInformation,
    database: Option<&LogicalDatabase>,
) -> Result<(), ProjectionError> {
    if !general.program_status.is_default() {
        return Err(unsupported("generalInformation.programStatus"));
    }
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

        assert_eq!(merged.description, "Updated program");
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
        assert_eq!(merged.description, "Updated include");
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
    fn rejects_program_fields_not_available_from_adt_properties() {
        let original = program();
        let mut edited: ProjectedProgramProperties =
            serde_json::from_str(&render(&original).unwrap()).unwrap();
        edited
            .general_information
            .get_or_insert_default()
            .program_status = ProgramStatus::CustomerProductionProgram;

        assert!(matches!(
            merge(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "generalInformation.programStatus",
                ..
            })
        ));
    }
}
