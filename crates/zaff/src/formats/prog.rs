use serde::{Deserialize, Serialize};
use zadt::{
    GlobalWorkbenchType, Include, IncludeProperties, MediaTyped, ObjectSnapshot, ObjectType,
    Program, ProgramProperties,
};

use crate::{
    Cardinality, ComponentId, FileBacking, FileSpec, ObjectFormat, ProjectionError,
    format::{
        FileDescriptor, FormatDescriptor, PropertiesCodec, PropertyProjection,
        SourceFileDescriptor, UnbackedFileDescriptor, decode_properties, encode_properties,
    },
    language,
};

pub const PROGRAM_FORMAT: ObjectFormat = ObjectFormat::new("PROG", "1");

#[derive(Debug)]
pub(crate) struct ProgramDescriptor;

#[derive(Debug)]
struct ProgramMetadata;

static PROGRAM_FILES: &[FileSpec] = &[
    FileSpec::new("<name>.prog.json", Cardinality::One, &ProgramMetadata),
    FileSpec::new(
        "<name>.prog.abap",
        Cardinality::One,
        &SourceFileDescriptor::main(ComponentId::new("source/main")),
    ),
    FileSpec::new(
        "<name>.prog.texts.<lang>.properties",
        Cardinality::ZeroOrMore,
        &UnbackedFileDescriptor::new(ComponentId::new("text/texts")),
    ),
    FileSpec::new(
        "<name>.prog.headings.<lang>.properties",
        Cardinality::ZeroOrMore,
        &UnbackedFileDescriptor::new(ComponentId::new("text/headings")),
    ),
    FileSpec::new(
        "<name>.prog.selections.<lang>.properties",
        Cardinality::ZeroOrMore,
        &UnbackedFileDescriptor::new(ComponentId::new("text/selections")),
    ),
];

impl FormatDescriptor for ProgramDescriptor {
    fn format(&self) -> ObjectFormat {
        PROGRAM_FORMAT
    }

    fn repository_types(&self) -> &'static [GlobalWorkbenchType] {
        const TYPES: &[GlobalWorkbenchType] = &[Program::WORKBENCH_TYPE, Include::WORKBENCH_TYPE];
        TYPES
    }

    fn files(&self) -> &'static [FileSpec] {
        PROGRAM_FILES
    }

    fn repository_type_from_metadata(
        &self,
        metadata: &[u8],
    ) -> Result<GlobalWorkbenchType, ProjectionError> {
        let metadata: MetadataDiscriminator = serde_json::from_slice(metadata)?;
        if metadata.format_version != PROGRAM_FORMAT.version() {
            return Err(ProjectionError::UnsupportedFormatVersion {
                object_type: PROGRAM_FORMAT.object_type(),
                version: metadata.format_version,
            });
        }
        match metadata
            .general_information
            .and_then(|information| information.program_type)
            .as_deref()
        {
            None | Some("executableProgram" | "modulePool" | "subroutinePool") => {
                Ok(Program::WORKBENCH_TYPE)
            }
            Some("include") => Ok(Include::WORKBENCH_TYPE),
            Some(program_type) => Err(ProjectionError::UnsupportedProgramType {
                program_type: program_type.to_owned(),
            }),
        }
    }
}

impl FileDescriptor for ProgramMetadata {
    fn component(&self) -> ComponentId {
        ComponentId::new("metadata")
    }

    fn bind(
        &self,
        object: &dyn crate::format::ProjectionObject,
        _language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError> {
        if object.object_type() != &Program::WORKBENCH_TYPE
            && object.object_type() != &Include::WORKBENCH_TYPE
        {
            return Err(ProjectionError::UnsupportedFileComponent {
                object_type: object.object_type().clone(),
                component: self.component(),
            });
        }
        Ok(Some(FileBacking::Properties(object.reference())))
    }

    fn properties_codec(&self) -> Option<&dyn PropertiesCodec> {
        Some(self)
    }
}

impl PropertiesCodec for ProgramMetadata {
    fn render(&self, properties: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
        if ProgramProperties::MEDIA_TYPES.contains(properties.media_type()) {
            return render_program_properties(&decode_properties::<ProgramProperties>(
                properties, "PROG",
            )?);
        }
        render_include_properties(&decode_properties::<IncludeProperties>(properties, "PROG")?)
    }

    fn merge(
        &self,
        original: &ObjectSnapshot<()>,
        edited: &str,
    ) -> Result<serde_json::Value, ProjectionError> {
        if ProgramProperties::MEDIA_TYPES.contains(original.media_type()) {
            let properties = decode_properties::<ProgramProperties>(original, "PROG")?;
            return encode_properties(merge_program_properties(&properties, edited)?, "PROG");
        }
        let properties = decode_properties::<IncludeProperties>(original, "PROG")?;
        encode_properties(merge_include_properties(&properties, edited)?, "PROG")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDiscriminator {
    format_version: String,
    #[serde(default)]
    general_information: Option<ProgramInformationDiscriminator>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgramInformationDiscriminator {
    #[serde(default)]
    program_type: Option<String>,
}

/// The AFF v1 metadata shared by programs and standalone includes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffProgram {
    pub format_version: String,
    pub header: AffProgramHeader,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub general_information: Option<AffProgramGeneralInformation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_database: Option<AffLogicalDatabase>,
}

/// Common AFF Program header fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffProgramHeader {
    pub description: String,
    pub original_language: String,
}

/// General Program attributes represented by AFF v1.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffProgramGeneralInformation {
    #[serde(default, skip_serializing_if = "AffProgramType::is_default")]
    pub program_type: AffProgramType,
    #[serde(default, skip_serializing_if = "AffProgramStatus::is_default")]
    pub program_status: AffProgramStatus,
    #[serde(default, skip_serializing_if = "is_false")]
    pub fix_point_arithmetic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub edit_locked: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub starts_using_variant: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub authorization_group: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub application: String,
}

impl AffProgramGeneralInformation {
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
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffProgramType {
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

impl AffProgramType {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "executableProgram" => Ok(Self::ExecutableProgram),
            "modulePool" => Ok(Self::ModulePool),
            "subroutinePool" => Ok(Self::SubroutinePool),
            "include" => Ok(Self::Include),
            value => Err(invalid(
                "generalInformation.programType",
                format!("unsupported ADT program type `{value}`"),
            )),
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

/// AFF's Program status vocabulary.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffProgramStatus {
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

impl AffProgramStatus {
    const fn is_default(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

/// AFF logical-database assignment for an executable Program.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffLogicalDatabase {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub selection_screen: String,
}

impl AffLogicalDatabase {
    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.selection_screen.is_empty()
    }
}

pub(crate) fn render_program_properties(
    properties: &ProgramProperties,
) -> Result<String, ProjectionError> {
    render(&<AffProgram as PropertyProjection<ProgramProperties>>::project(properties)?)
}

pub(crate) fn merge_program_properties(
    original: &ProgramProperties,
    edited: &str,
) -> Result<ProgramProperties, ProjectionError> {
    let edited = parse(edited)?;
    let edited_general = edited.general_information.clone().unwrap_or_default();
    validate_program_fields(&edited_general, edited.logical_database.as_ref())?;
    if edited_general.program_type == AffProgramType::Include {
        return Err(invalid(
            "generalInformation.programType",
            "a Program cannot be changed into a standalone Include",
        ));
    }

    let original_document =
        <AffProgram as PropertyProjection<ProgramProperties>>::project(original)?;
    let original_general = original_document.general_information.unwrap_or_default();
    let mut merged = original.clone();
    let properties = &mut merged;
    if edited.header.description != original_document.header.description {
        properties.description = edited.header.description;
    }
    if edited.header.original_language != original_document.header.original_language {
        properties.master_language =
            language::to_adt(&edited.header.original_language, "header.originalLanguage")?;
    }
    if edited_general.program_type != original_general.program_type {
        properties.program_type = edited_general.program_type.adt_value().to_owned();
    }
    if edited_general.fix_point_arithmetic != original_general.fix_point_arithmetic {
        properties.fix_point_arithmetic = edited_general.fix_point_arithmetic;
    }
    if edited_general.edit_locked != original_general.edit_locked {
        properties.locked_by_editor = edited_general.edit_locked;
    }
    Ok(merged)
}

pub(crate) fn render_include_properties(
    properties: &IncludeProperties,
) -> Result<String, ProjectionError> {
    render(&<AffProgram as PropertyProjection<IncludeProperties>>::project(properties)?)
}

pub(crate) fn merge_include_properties(
    original: &IncludeProperties,
    edited: &str,
) -> Result<IncludeProperties, ProjectionError> {
    let edited = parse(edited)?;
    let edited_general = edited.general_information.clone().unwrap_or_default();
    validate_program_fields(&edited_general, edited.logical_database.as_ref())?;
    if edited_general.program_type != AffProgramType::Include {
        return Err(invalid(
            "generalInformation.programType",
            "a standalone Include must use program type `include`",
        ));
    }
    if edited_general.edit_locked {
        return Err(unsupported("PROG", "generalInformation.editLocked"));
    }

    let original_document =
        <AffProgram as PropertyProjection<IncludeProperties>>::project(original)?;
    let original_general = original_document.general_information.unwrap_or_default();
    let mut merged = original.clone();
    let properties = &mut merged;
    if edited.header.description != original_document.header.description {
        properties.description = edited.header.description;
    }
    if edited.header.original_language != original_document.header.original_language {
        properties.master_language =
            language::to_adt(&edited.header.original_language, "header.originalLanguage")?;
    }
    if edited_general.fix_point_arithmetic != original_general.fix_point_arithmetic {
        properties.fix_point_arithmetic = edited_general.fix_point_arithmetic;
    }
    Ok(merged)
}

impl PropertyProjection<ProgramProperties> for AffProgram {
    fn project(properties: &ProgramProperties) -> Result<Self, ProjectionError> {
        let general = AffProgramGeneralInformation {
            program_type: AffProgramType::from_adt(&properties.program_type)?,
            fix_point_arithmetic: properties.fix_point_arithmetic,
            edit_locked: properties.locked_by_editor,
            ..Default::default()
        };
        let document = Self {
            format_version: PROGRAM_FORMAT.version().to_owned(),
            header: AffProgramHeader {
                description: properties.description.clone(),
                original_language: language::from_adt(
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
}

impl PropertyProjection<IncludeProperties> for AffProgram {
    fn project(properties: &IncludeProperties) -> Result<Self, ProjectionError> {
        let general = AffProgramGeneralInformation {
            program_type: AffProgramType::Include,
            fix_point_arithmetic: properties.fix_point_arithmetic,
            ..Default::default()
        };
        let document = Self {
            format_version: PROGRAM_FORMAT.version().to_owned(),
            header: AffProgramHeader {
                description: properties.description.clone(),
                original_language: language::from_adt(
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

fn parse(content: &str) -> Result<AffProgram, ProjectionError> {
    let document: AffProgram =
        serde_json::from_str(content).map_err(ProjectionError::InvalidProgramDocument)?;
    document.validate()?;
    Ok(document)
}

fn render(document: &AffProgram) -> Result<String, ProjectionError> {
    let mut content =
        serde_json::to_string_pretty(document).map_err(ProjectionError::InvalidProgramDocument)?;
    content.push('\n');
    Ok(content)
}

impl AffProgram {
    fn validate(&self) -> Result<(), ProjectionError> {
        if self.format_version != PROGRAM_FORMAT.version() {
            return Err(invalid(
                "formatVersion",
                format!("expected `{}`", PROGRAM_FORMAT.version()),
            ));
        }
        max_length("header.description", &self.header.description, 70)?;
        language::to_adt(&self.header.original_language, "header.originalLanguage")?;
        if let Some(general) = &self.general_information {
            max_length(
                "generalInformation.authorizationGroup",
                &general.authorization_group,
                8,
            )?;
            max_length("generalInformation.application", &general.application, 1)?;
        }
        if let Some(database) = &self.logical_database {
            max_length("logicalDatabase.name", &database.name, 20)?;
            max_length(
                "logicalDatabase.selectionScreen",
                &database.selection_screen,
                3,
            )?;
        }
        Ok(())
    }
}

fn validate_program_fields(
    general: &AffProgramGeneralInformation,
    database: Option<&AffLogicalDatabase>,
) -> Result<(), ProjectionError> {
    if !general.program_status.is_default() {
        return Err(unsupported("PROG", "generalInformation.programStatus"));
    }
    if general.starts_using_variant {
        return Err(unsupported("PROG", "generalInformation.startsUsingVariant"));
    }
    if !general.authorization_group.is_empty() {
        return Err(unsupported("PROG", "generalInformation.authorizationGroup"));
    }
    if !general.application.is_empty() {
        return Err(unsupported("PROG", "generalInformation.application"));
    }
    if database.is_some_and(|database| !database.is_empty()) {
        return Err(unsupported("PROG", "logicalDatabase"));
    }
    Ok(())
}

fn max_length(field: &'static str, value: &str, maximum: usize) -> Result<(), ProjectionError> {
    let length = value.chars().count();
    if length > maximum {
        return Err(invalid(
            field,
            format!("length {length} exceeds maximum {maximum}"),
        ));
    }
    Ok(())
}

fn invalid(field: &'static str, message: impl Into<String>) -> ProjectionError {
    ProjectionError::InvalidAffField {
        field,
        message: message.into(),
    }
}

fn unsupported(object_type: &'static str, field: &'static str) -> ProjectionError {
    ProjectionError::UnsupportedAffProperty { object_type, field }
}

fn is_false(value: &bool) -> bool {
    !value
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use zadt::{Include, IncludeProperties, MediaTyped, Program, ProgramProperties};

    use super::*;

    const PROGRAM_XML: &[u8] = include_bytes!("../../../zadt/tests/fixtures/program-z-test.xml");
    const INCLUDE_XML: &[u8] = include_bytes!("../../../zadt/tests/fixtures/include-ztest.xml");

    fn program() -> ProgramProperties {
        let reference = crate::test_support::reference::<Program>(
            "Z_TEST",
            "/sap/bc/adt/programs/programs/z_test",
        );
        crate::test_support::properties(
            &reference,
            ProgramProperties::MEDIA_TYPES[0],
            "program-etag",
            PROGRAM_XML,
        )
        .properties()
        .clone()
    }

    fn include() -> IncludeProperties {
        let reference = crate::test_support::reference::<Include>(
            "ZTEST",
            "/sap/bc/adt/programs/includes/ztest",
        );
        crate::test_support::properties(
            &reference,
            IncludeProperties::MEDIA_TYPES[0],
            "include-etag",
            INCLUDE_XML,
        )
        .properties()
        .clone()
    }

    #[test]
    fn renders_program_and_include_metadata_as_aff_v1() {
        let program = program();
        let program_json = render_program_properties(&program).unwrap();
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
        let include_json = render_include_properties(&include).unwrap();
        let include_document: Value = serde_json::from_str(&include_json).unwrap();
        assert_eq!(
            include_document["generalInformation"]["programType"],
            "include"
        );
    }

    #[test]
    fn merges_program_edits_without_losing_the_adt_envelope() {
        let original = program();
        let mut edited: AffProgram =
            serde_json::from_str(&render_program_properties(&original).unwrap()).unwrap();
        edited.header.description = "Updated program".to_owned();
        edited.header.original_language = "de-CH".to_owned();
        let general = edited.general_information.get_or_insert_default();
        general.program_type = AffProgramType::ModulePool;
        general.fix_point_arithmetic = false;
        general.edit_locked = true;

        let merged =
            merge_program_properties(&original, &serde_json::to_string(&edited).unwrap()).unwrap();

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
    fn merges_include_edits_and_requires_the_include_discriminator() {
        let original = include();
        let mut edited: AffProgram =
            serde_json::from_str(&render_include_properties(&original).unwrap()).unwrap();
        edited.header.description = "Updated include".to_owned();
        edited.header.original_language = "zh-Hant".to_owned();
        edited
            .general_information
            .as_mut()
            .unwrap()
            .fix_point_arithmetic = true;

        let merged =
            merge_include_properties(&original, &serde_json::to_string(&edited).unwrap()).unwrap();
        assert_eq!(merged.description, "Updated include");
        assert_eq!(merged.master_language, "ZF");
        assert!(merged.fix_point_arithmetic);
        assert_eq!(merged.links, original.links);

        edited.general_information.as_mut().unwrap().program_type =
            AffProgramType::ExecutableProgram;
        assert!(matches!(
            merge_include_properties(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::InvalidAffField {
                field: "generalInformation.programType",
                ..
            })
        ));
    }

    #[test]
    fn rejects_program_fields_not_available_from_adt_properties() {
        let original = program();
        let mut edited: AffProgram =
            serde_json::from_str(&render_program_properties(&original).unwrap()).unwrap();
        edited
            .general_information
            .get_or_insert_default()
            .program_status = AffProgramStatus::CustomerProductionProgram;

        assert!(matches!(
            merge_program_properties(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "generalInformation.programStatus",
                ..
            })
        ));
    }
}
