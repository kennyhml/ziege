#![doc = include_str!("../README.md")]

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;
use zadt::{
    Class, ClassSourceComponent, DataElement, DataElementProperties, GlobalWorkbenchType, Include,
    ObjectRef, Program, RepositoryObject, RepositoryObjectEntry, SourceRef,
};

mod data_element;

pub use data_element::{
    AffAbapLanguageVersion, AffBasicDirection, AffBidirectionalOptions, AffDataElement,
    AffDataElementAdditionalProperties, AffDataElementCategory, AffDataElementFieldLabels,
    AffDataElementHeader, AffDataElementTypeInformation, AffPredefinedType, AffSearchHelp,
};

const PROGRAM_FILES: &[FileSpec] = &[
    FileSpec::new(
        "<name>.prog.json",
        Cardinality::One,
        FileComponent::Metadata,
    ),
    FileSpec::new(
        "<name>.prog.abap",
        Cardinality::One,
        FileComponent::Source(SourceComponent::Main),
    ),
    FileSpec::new(
        "<name>.prog.texts.<lang>.properties",
        Cardinality::ZeroOrMore,
        FileComponent::Text(TextComponent::Texts),
    ),
    FileSpec::new(
        "<name>.prog.headings.<lang>.properties",
        Cardinality::ZeroOrMore,
        FileComponent::Text(TextComponent::ProgramHeadings),
    ),
    FileSpec::new(
        "<name>.prog.selections.<lang>.properties",
        Cardinality::ZeroOrMore,
        FileComponent::Text(TextComponent::ProgramSelections),
    ),
];

const CLASS_FILES: &[FileSpec] = &[
    FileSpec::new(
        "<name>.clas.json",
        Cardinality::One,
        FileComponent::Metadata,
    ),
    FileSpec::new(
        "<name>.clas.abap",
        Cardinality::One,
        FileComponent::Source(SourceComponent::Main),
    ),
    FileSpec::new(
        "<name>.clas.definitions.abap",
        Cardinality::ZeroOrOne,
        FileComponent::Source(SourceComponent::Class(ClassSourceComponent::Definitions)),
    ),
    FileSpec::new(
        "<name>.clas.implementations.abap",
        Cardinality::ZeroOrOne,
        FileComponent::Source(SourceComponent::Class(
            ClassSourceComponent::Implementations,
        )),
    ),
    FileSpec::new(
        "<name>.clas.macros.abap",
        Cardinality::ZeroOrOne,
        FileComponent::Source(SourceComponent::Class(ClassSourceComponent::Macros)),
    ),
    FileSpec::new(
        "<name>.clas.testclasses.abap",
        Cardinality::ZeroOrOne,
        FileComponent::Source(SourceComponent::Class(ClassSourceComponent::TestClasses)),
    ),
    FileSpec::new(
        "<name>.clas.locals.abap",
        Cardinality::ZeroOrOne,
        FileComponent::Source(SourceComponent::Class(ClassSourceComponent::LocalTypes)),
    ),
    FileSpec::new(
        "<name>.clas.texts.<lang>.properties",
        Cardinality::ZeroOrMore,
        FileComponent::Text(TextComponent::Texts),
    ),
];

const DATA_ELEMENT_FILES: &[FileSpec] = &[FileSpec::new(
    "<name>.dtel.json",
    Cardinality::One,
    FileComponent::Metadata,
)];

/// An ABAP File Formats object family supported by this projection layer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ObjectFormat {
    Program,
    Class,
    DataElement,
}

impl ObjectFormat {
    /// Returns the R3TR object type used in ABAP file names.
    pub const fn object_type(self) -> &'static str {
        match self {
            Self::Program => "PROG",
            Self::Class => "CLAS",
            Self::DataElement => "DTEL",
        }
    }

    /// Returns the currently supported AFF version for this object family.
    pub const fn version(self) -> &'static str {
        "1"
    }

    /// Returns all possible files in this object family's AFF representation.
    pub const fn files(self) -> &'static [FileSpec] {
        match self {
            Self::Program => PROGRAM_FILES,
            Self::Class => CLASS_FILES,
            Self::DataElement => DATA_ELEMENT_FILES,
        }
    }

    /// Infers the exact global Workbench type from this family's AFF metadata.
    ///
    /// AFF uses one `PROG` layout for programs and standalone includes. Its
    /// `programType` property supplies the distinction that is absent from the
    /// file name. An omitted program type has AFF's `executableProgram` default.
    pub fn repository_type_from_metadata(
        self,
        metadata: &[u8],
    ) -> Result<GlobalWorkbenchType, ProjectionError> {
        let metadata: MetadataDiscriminator = serde_json::from_slice(metadata)?;
        if metadata.format_version != self.version() {
            return Err(ProjectionError::UnsupportedFormatVersion {
                object_type: self.object_type(),
                version: metadata.format_version,
            });
        }

        match self {
            Self::Class => Ok(GlobalWorkbenchType::new("CLAS/OC")),
            Self::DataElement => Ok(GlobalWorkbenchType::new("DTEL/DE")),
            Self::Program => match metadata
                .general_information
                .and_then(|information| information.program_type)
                .as_deref()
            {
                None | Some("executableProgram" | "modulePool" | "subroutinePool") => {
                    Ok(GlobalWorkbenchType::new("PROG/P"))
                }
                Some("include") => Ok(GlobalWorkbenchType::new("PROG/I")),
                Some(program_type) => Err(ProjectionError::UnsupportedProgramType {
                    program_type: program_type.to_owned(),
                }),
            },
        }
    }

    /// Resolves a global Workbench type into its AFF object family.
    pub fn for_workbench_type(object_type: &GlobalWorkbenchType) -> Result<Self, ProjectionError> {
        match object_type.as_str() {
            "PROG/P" | "PROG/I" => Ok(Self::Program),
            "CLAS/OC" => Ok(Self::Class),
            "DTEL/DE" => Ok(Self::DataElement),
            _ => Err(ProjectionError::UnsupportedRepositoryType {
                object_type: object_type.clone(),
            }),
        }
    }
}

impl TryFrom<&RepositoryObjectEntry> for ObjectFormat {
    type Error = ProjectionError;

    fn try_from(entry: &RepositoryObjectEntry) -> Result<Self, Self::Error> {
        Self::for_workbench_type(&entry.object_type)
    }
}

impl TryFrom<&RepositoryObject> for ObjectFormat {
    type Error = ProjectionError;

    fn try_from(object: &RepositoryObject) -> Result<Self, Self::Error> {
        Self::for_workbench_type(object.object_type())
    }
}

/// The number of files permitted for one file specification.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cardinality {
    One,
    ZeroOrOne,
    ZeroOrMore,
}

/// The logical content represented by an AFF file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileComponent {
    Metadata,
    Source(SourceComponent),
    Text(TextComponent),
}

/// An ADT source resource represented by an AFF source file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SourceComponent {
    /// The primary source of an object.
    Main,
    /// One secondary source component owned by a `CLAS/OC` object.
    Class(ClassSourceComponent),
}

/// A language-dependent text resource represented by an AFF properties file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextComponent {
    Texts,
    ProgramHeadings,
    ProgramSelections,
}

/// One possible file in an AFF object representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileSpec {
    pub template: &'static str,
    pub cardinality: Cardinality,
    pub component: FileComponent,
}

/// The ADT resource used to materialize one projected AFF file.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum FileBacking {
    /// A plain-text source resource.
    Source(SourceRef),
    /// Object properties that require an AFF schema transformation.
    Properties(ObjectRef<DataElement>),
}

/// One materializable AFF file bound to its originating repository object.
#[derive(Clone, Debug)]
pub struct ProjectedFile {
    pub name: String,
    pub format: ObjectFormat,
    pub cardinality: Cardinality,
    pub component: FileComponent,
    pub backing: FileBacking,
}

impl ProjectedFile {
    /// Renders properties returned by ZADT as this file's AFF JSON content.
    pub fn render_properties(
        &self,
        properties: &DataElementProperties,
    ) -> Result<String, ProjectionError> {
        match (&self.backing, self.format, self.component) {
            (FileBacking::Properties(_), ObjectFormat::DataElement, FileComponent::Metadata) => {
                data_element::render_data_element_properties(properties)
            }
            _ => Err(ProjectionError::NotPropertiesFile {
                component: self.component,
            }),
        }
    }

    /// Merges edited AFF JSON into the original ZADT properties representation.
    pub fn merge_properties(
        &self,
        original: &DataElementProperties,
        edited: &str,
    ) -> Result<DataElementProperties, ProjectionError> {
        match (&self.backing, self.format, self.component) {
            (FileBacking::Properties(_), ObjectFormat::DataElement, FileComponent::Metadata) => {
                data_element::merge_data_element_properties(original, edited)
            }
            _ => Err(ProjectionError::NotPropertiesFile {
                component: self.component,
            }),
        }
    }
}

/// A repository object's currently materializable editor-facing AFF files.
#[derive(Clone, Debug)]
pub struct Projection {
    pub format: ObjectFormat,
    pub files: Vec<ProjectedFile>,
}

/// Projects a runtime repository object into its currently materializable AFF files.
///
/// Optional source specifications remain possible files; the consumer decides
/// whether the corresponding backend resource exists. Specifications without
/// an implemented content backing are omitted.
pub fn project(object: &RepositoryObject) -> Result<Projection, ProjectionError> {
    let format = ObjectFormat::try_from(object)?;
    let object_name = object.reference().name().to_owned();
    let files = format
        .files()
        .iter()
        .filter(|specification| match format {
            ObjectFormat::DataElement => specification.component == FileComponent::Metadata,
            ObjectFormat::Program | ObjectFormat::Class => {
                matches!(specification.component, FileComponent::Source(_))
            }
        })
        .map(|specification| {
            let name = specification.file_name(&object_name, None)?;
            let resolved = resolve_file_name(&name)?;
            Ok(ProjectedFile {
                name,
                format,
                cardinality: specification.cardinality,
                component: specification.component,
                backing: resolved.bind(object)?,
            })
        })
        .collect::<Result<_, ProjectionError>>()?;
    Ok(Projection { format, files })
}

impl FileSpec {
    const fn new(
        template: &'static str,
        cardinality: Cardinality,
        component: FileComponent,
    ) -> Self {
        Self {
            template,
            cardinality,
            component,
        }
    }

    /// Renders this specification for one ABAP object and optional language.
    pub fn file_name(
        self,
        object_name: &str,
        language: Option<&str>,
    ) -> Result<String, ProjectionError> {
        let object_name = encode_object_name(object_name)?;
        let mut file_name = self.template.replacen("<name>", &object_name, 1);

        if file_name.contains("<lang>") {
            let language = language.ok_or(ProjectionError::MissingLanguage {
                template: self.template,
            })?;
            validate_language(language)?;
            file_name = file_name.replacen("<lang>", language, 1);
        } else if let Some(language) = language {
            return Err(ProjectionError::UnexpectedLanguage {
                template: self.template,
                language: language.to_owned(),
            });
        }

        Ok(file_name)
    }
}

/// The semantic result of resolving an AFF path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFile {
    pub object_name: String,
    pub format: ObjectFormat,
    pub component: FileComponent,
    pub language: Option<String>,
}

impl ResolvedFile {
    /// Binds this projected path to the runtime repository object that produced it.
    pub fn bind(&self, object: &RepositoryObject) -> Result<FileBacking, ProjectionError> {
        let reference = object.reference();
        if !self.object_name.eq_ignore_ascii_case(reference.name()) {
            return Err(ProjectionError::BindingNameMismatch {
                projected_name: self.object_name.clone(),
                repository_name: reference.name().to_owned(),
            });
        }

        let repository_format = ObjectFormat::try_from(object)?;
        if self.format != repository_format {
            return Err(ProjectionError::BindingTypeMismatch {
                projected_type: self.format.object_type(),
                repository_type: object.object_type().clone(),
            });
        }

        match self.component {
            FileComponent::Source(SourceComponent::Main) => object
                .source()
                .map(FileBacking::Source)
                .ok_or_else(|| ProjectionError::UnsupportedFileComponent {
                    object_type: object.object_type().clone(),
                    component: self.component,
                }),
            FileComponent::Source(SourceComponent::Class(component)) => object
                .source_component(component.as_str())
                .map(FileBacking::Source)
                .ok_or_else(|| ProjectionError::UnsupportedFileComponent {
                    object_type: object.object_type().clone(),
                    component: self.component,
                }),
            FileComponent::Metadata if self.format == ObjectFormat::DataElement => object
                .typed::<DataElement>()
                .map(FileBacking::Properties)
                .ok_or_else(|| ProjectionError::UnsupportedFileComponent {
                    object_type: object.object_type().clone(),
                    component: self.component,
                }),
            _ => Err(ProjectionError::UnsupportedFileComponent {
                object_type: object.object_type().clone(),
                component: self.component,
            }),
        }
    }

    /// Resolves this projected file to its ADT source resource.
    ///
    /// The repository entry is the authoritative remote identity retained when
    /// the path was projected. Its name and object family must agree with this
    /// file before the source reference is returned.
    pub fn source_ref(&self, entry: &RepositoryObjectEntry) -> Result<SourceRef, ProjectionError> {
        if !self.object_name.eq_ignore_ascii_case(&entry.name) {
            return Err(ProjectionError::BindingNameMismatch {
                projected_name: self.object_name.clone(),
                repository_name: entry.name.clone(),
            });
        }

        let repository_format = ObjectFormat::try_from(entry)?;
        if self.format != repository_format {
            return Err(ProjectionError::BindingTypeMismatch {
                projected_type: self.format.object_type(),
                repository_type: entry.object_type.clone(),
            });
        }

        match self.component {
            FileComponent::Source(SourceComponent::Main) => match entry.object_type.as_str() {
                "PROG/P" => Ok(entry.typed_reference::<Program>()?.source()),
                "PROG/I" => Ok(entry.typed_reference::<Include>()?.source()),
                "CLAS/OC" => Ok(entry.typed_reference::<Class>()?.source()),
                _ => Err(ProjectionError::UnsupportedRepositoryType {
                    object_type: entry.object_type.clone(),
                }),
            },
            FileComponent::Source(SourceComponent::Class(component)) => Ok(entry
                .typed_reference::<Class>()?
                .component_source(component)),
            component => Err(ProjectionError::NotSourceFile { component }),
        }
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

/// Resolves an AFF path into its object family and logical component.
///
/// This does not recover a unique remote identity. The language server should
/// bind the result to the `RepositoryObjectEntry` from which it projected the
/// path.
pub fn resolve_path(path: impl AsRef<Path>) -> Result<ResolvedFile, ProjectionError> {
    let path = path.as_ref();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProjectionError::NonUtf8Path)?;
    resolve_file_name(file_name)
}

/// Resolves an AFF file name into its object family and logical component.
pub fn resolve_file_name(file_name: &str) -> Result<ResolvedFile, ProjectionError> {
    for format in [
        ObjectFormat::Class,
        ObjectFormat::Program,
        ObjectFormat::DataElement,
    ] {
        for specification in format.files() {
            if let Some((object_name, language)) = match_template(specification.template, file_name)
            {
                if let Some(language) = language {
                    validate_language(language)?;
                }
                return Ok(ResolvedFile {
                    object_name: decode_object_name(object_name)?,
                    format,
                    component: specification.component,
                    language: language.map(str::to_owned),
                });
            }
        }
    }

    Err(ProjectionError::UnsupportedFileName {
        file_name: file_name.to_owned(),
    })
}

fn match_template<'a>(
    template: &'static str,
    file_name: &'a str,
) -> Option<(&'a str, Option<&'a str>)> {
    let remainder = template.strip_prefix("<name>")?;
    if let Some((before_language, after_language)) = remainder.split_once("<lang>") {
        let without_suffix = file_name.strip_suffix(after_language)?;
        let (object_name, language) = without_suffix.rsplit_once(before_language)?;
        if object_name.is_empty() || language.is_empty() {
            return None;
        }
        return Some((object_name, Some(language)));
    }

    let object_name = file_name.strip_suffix(remainder)?;
    (!object_name.is_empty()).then_some((object_name, None))
}

fn encode_object_name(object_name: &str) -> Result<String, ProjectionError> {
    validate_object_name(object_name)?;
    if let Some(namespaced) = object_name.strip_prefix('/') {
        let (namespace, local_name) =
            namespaced
                .split_once('/')
                .ok_or_else(|| ProjectionError::InvalidObjectName {
                    object_name: object_name.to_owned(),
                })?;
        if namespace.is_empty()
            || local_name.is_empty()
            || local_name.contains(['/', '(', ')'])
            || namespace.contains(['(', ')'])
        {
            return Err(ProjectionError::InvalidObjectName {
                object_name: object_name.to_owned(),
            });
        }
        Ok(format!(
            "({}){}",
            namespace.to_ascii_lowercase(),
            local_name.to_ascii_lowercase()
        ))
    } else if object_name.contains(['/', '(', ')']) {
        Err(ProjectionError::InvalidObjectName {
            object_name: object_name.to_owned(),
        })
    } else {
        Ok(object_name.to_ascii_lowercase())
    }
}

fn decode_object_name(file_name: &str) -> Result<String, ProjectionError> {
    validate_object_name(file_name)?;
    if let Some(namespaced) = file_name.strip_prefix('(') {
        let (namespace, local_name) =
            namespaced
                .split_once(')')
                .ok_or_else(|| ProjectionError::InvalidObjectName {
                    object_name: file_name.to_owned(),
                })?;
        if namespace.is_empty()
            || local_name.is_empty()
            || namespace.contains(['(', ')', '/'])
            || local_name.contains(['(', ')', '/'])
        {
            return Err(ProjectionError::InvalidObjectName {
                object_name: file_name.to_owned(),
            });
        }
        Ok(format!(
            "/{}/{}",
            namespace.to_ascii_uppercase(),
            local_name.to_ascii_uppercase()
        ))
    } else if file_name.contains(['(', ')', '/']) {
        Err(ProjectionError::InvalidObjectName {
            object_name: file_name.to_owned(),
        })
    } else {
        Ok(file_name.to_ascii_uppercase())
    }
}

fn validate_object_name(object_name: &str) -> Result<(), ProjectionError> {
    if object_name.is_empty()
        || object_name.trim() != object_name
        || object_name.chars().any(char::is_control)
        || object_name.contains('\\')
        || matches!(object_name, "." | "..")
    {
        return Err(ProjectionError::InvalidObjectName {
            object_name: object_name.to_owned(),
        });
    }
    Ok(())
}

fn validate_language(language: &str) -> Result<(), ProjectionError> {
    if language.is_empty()
        || language
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    {
        return Err(ProjectionError::InvalidLanguage {
            language: language.to_owned(),
        });
    }
    Ok(())
}

/// An error mapping between ADT repository objects and AFF files.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionError {
    #[error("repository object type `{object_type}` has no supported AFF projection")]
    UnsupportedRepositoryType { object_type: GlobalWorkbenchType },

    #[error("file name `{file_name}` does not match a supported AFF projection")]
    UnsupportedFileName { file_name: String },

    #[error(
        "projected object `{projected_name}` cannot bind to repository object `{repository_name}`"
    )]
    BindingNameMismatch {
        projected_name: String,
        repository_name: String,
    },

    #[error(
        "projected `{projected_type}` file cannot bind to repository object type `{repository_type}`"
    )]
    BindingTypeMismatch {
        projected_type: &'static str,
        repository_type: GlobalWorkbenchType,
    },

    #[error("AFF component `{component:?}` is not an ADT source resource")]
    NotSourceFile { component: FileComponent },

    #[error("AFF component `{component:?}` is not backed by ADT object properties")]
    NotPropertiesFile { component: FileComponent },

    #[error("repository object type `{object_type}` cannot supply AFF component `{component:?}`")]
    UnsupportedFileComponent {
        object_type: GlobalWorkbenchType,
        component: FileComponent,
    },

    #[error("invalid AFF Data Element document: {0}")]
    InvalidDataElementDocument(#[source] serde_json::Error),

    #[error("invalid AFF Data Element field `{field}`: {message}")]
    InvalidDataElementField {
        field: &'static str,
        message: String,
    },

    #[error(transparent)]
    InvalidObjectReference(#[from] zadt::ObjectError),

    #[error("invalid AFF metadata: {0}")]
    InvalidMetadata(#[from] serde_json::Error),

    #[error("AFF object type `{object_type}` does not support format version `{version}`")]
    UnsupportedFormatVersion {
        object_type: &'static str,
        version: String,
    },

    #[error("AFF program type `{program_type}` cannot be mapped to an ADT Workbench type")]
    UnsupportedProgramType { program_type: String },

    #[error("`{object_name}` is not a valid projectable ABAP object name")]
    InvalidObjectName { object_name: String },

    #[error("file template `{template}` requires a language")]
    MissingLanguage { template: &'static str },

    #[error("file template `{template}` does not accept language `{language}`")]
    UnexpectedLanguage {
        template: &'static str,
        language: String,
    },

    #[error("`{language}` is not a valid AFF language tag")]
    InvalidLanguage { language: String },

    #[error("AFF paths must be valid UTF-8")]
    NonUtf8Path,
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, StatusCode};
    use zadt::{AdtResponse, AdtUri, Operation, OperationResponse, Ready, RepositoryContentQuery};

    use super::*;

    fn repository_entry(name: &str, object_type: &str, uri: &str) -> RepositoryObjectEntry {
        let body = format!(
            r#"<vfs:virtualFoldersResult xmlns:vfs="http://www.sap.com/adt/ris/virtualFolders" objectCount="1">
                <vfs:object name="{name}" package="$TMP" type="{object_type}"
                    uri="{uri}" expandable="false" />
            </vfs:virtualFoldersResult>"#
        );
        let response = AdtResponse::new(StatusCode::OK, HeaderMap::new(), body.into_bytes());
        let target =
            AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents")
                .unwrap();
        let mut content = <RepositoryContentQuery as Operation<Ready>>::decode(
            &RepositoryContentQuery::new(),
            OperationResponse::new(response, target),
        )
        .unwrap();
        content.objects.pop().unwrap()
    }

    #[test]
    fn maps_supported_repository_types_to_aff_families() {
        for (repository_type, expected) in [
            ("PROG/P", ObjectFormat::Program),
            ("PROG/I", ObjectFormat::Program),
            ("CLAS/OC", ObjectFormat::Class),
            ("DTEL/DE", ObjectFormat::DataElement),
        ] {
            let repository_type: GlobalWorkbenchType = repository_type.parse().unwrap();
            assert_eq!(
                ObjectFormat::for_workbench_type(&repository_type).unwrap(),
                expected
            );
        }

        for unsupported in ["CLAS/OM", "AUTH", "CLAS/OCN/definitions", "clas/oc"] {
            let repository_type: GlobalWorkbenchType = unsupported.parse().unwrap();
            assert!(ObjectFormat::for_workbench_type(&repository_type).is_err());
        }
    }

    #[test]
    fn recovers_exact_repository_types_from_aff_metadata() {
        for (metadata, expected) in [
            (
                br#"{"formatVersion":"1","generalInformation":{"programType":"include"}}"#
                    .as_slice(),
                "PROG/I",
            ),
            (br#"{"formatVersion":"1"}"#.as_slice(), "PROG/P"),
        ] {
            assert_eq!(
                ObjectFormat::Program
                    .repository_type_from_metadata(metadata)
                    .unwrap()
                    .to_string(),
                expected
            );
        }

        assert_eq!(
            ObjectFormat::Class
                .repository_type_from_metadata(br#"{"formatVersion":"1","header":{}}"#)
                .unwrap()
                .to_string(),
            "CLAS/OC"
        );
        assert_eq!(
            ObjectFormat::DataElement
                .repository_type_from_metadata(br#"{"formatVersion":"1","header":{}}"#)
                .unwrap()
                .to_string(),
            "DTEL/DE"
        );
    }

    #[test]
    fn rejects_unknown_metadata_versions_and_program_types() {
        assert!(matches!(
            ObjectFormat::Program.repository_type_from_metadata(br#"{"formatVersion":"2"}"#),
            Err(ProjectionError::UnsupportedFormatVersion { .. })
        ));
        assert!(matches!(
            ObjectFormat::Program.repository_type_from_metadata(
                br#"{"formatVersion":"1","generalInformation":{"programType":"unknown"}}"#,
            ),
            Err(ProjectionError::UnsupportedProgramType { .. })
        ));
    }

    #[test]
    fn resolves_program_and_class_sources() {
        assert_eq!(
            resolve_file_name("zexample.prog.abap").unwrap(),
            ResolvedFile {
                object_name: "ZEXAMPLE".to_owned(),
                format: ObjectFormat::Program,
                component: FileComponent::Source(SourceComponent::Main),
                language: None,
            }
        );
        assert_eq!(
            resolve_path("src/zcl_myclass.clas.testclasses.abap").unwrap(),
            ResolvedFile {
                object_name: "ZCL_MYCLASS".to_owned(),
                format: ObjectFormat::Class,
                component: FileComponent::Source(SourceComponent::Class(
                    ClassSourceComponent::TestClasses,
                )),
                language: None,
            }
        );
        assert_eq!(
            resolve_path("src/cx_root.clas.locals.abap").unwrap(),
            ResolvedFile {
                object_name: "CX_ROOT".to_owned(),
                format: ObjectFormat::Class,
                component: FileComponent::Source(SourceComponent::Class(
                    ClassSourceComponent::LocalTypes,
                )),
                language: None,
            }
        );
        assert_eq!(
            resolve_path("src/zexample.dtel.json").unwrap(),
            ResolvedFile {
                object_name: "ZEXAMPLE".to_owned(),
                format: ObjectFormat::DataElement,
                component: FileComponent::Metadata,
                language: None,
            }
        );
    }

    #[test]
    fn projects_data_elements_as_one_properties_backed_file() {
        let entry = repository_entry(
            "ZEXAMPLE",
            "DTEL/DE",
            "/sap/bc/adt/ddic/dataelements/zexample",
        );
        let object = entry.repository_object().unwrap();

        let projection = project(&object).unwrap();

        assert_eq!(projection.format, ObjectFormat::DataElement);
        assert_eq!(projection.files.len(), 1);
        let file = &projection.files[0];
        assert_eq!(file.name, "zexample.dtel.json");
        assert_eq!(file.cardinality, Cardinality::One);
        assert_eq!(file.component, FileComponent::Metadata);
        let FileBacking::Properties(bound) = &file.backing else {
            panic!("Data Element metadata must be properties-backed");
        };
        assert_eq!(bound.name(), "ZEXAMPLE");
        assert_eq!(bound.uri(), entry.reference.uri());
    }

    #[test]
    fn binds_class_files_to_their_owned_source_components() {
        let entry = repository_entry(
            "ZCL_MYCLASS",
            "CLAS/OC",
            "/sap/bc/adt/oo/classes/zcl_myclass",
        );
        let resolved = resolve_file_name("zcl_myclass.clas.testclasses.abap").unwrap();

        let source = resolved.source_ref(&entry).unwrap();

        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/oo/classes/zcl_myclass/includes/testclasses"
        );
        assert_eq!(source.object.uri(), entry.reference.uri());

        let resolved = resolve_file_name("zcl_myclass.clas.abap").unwrap();
        let source = resolved.source_ref(&entry).unwrap();
        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/oo/classes/zcl_myclass/source/main"
        );

        let resolved = resolve_file_name("zcl_myclass.clas.locals.abap").unwrap();
        let source = resolved.source_ref(&entry).unwrap();
        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/oo/classes/zcl_myclass/includes/localtypes"
        );
    }

    #[test]
    fn binds_shared_program_layout_to_the_exact_repository_type() {
        for (object_type, uri) in [
            ("PROG/P", "/sap/bc/adt/programs/programs/zshared_projection"),
            ("PROG/I", "/sap/bc/adt/programs/includes/zshared_projection"),
        ] {
            let entry = repository_entry("ZSHARED_PROJECTION", object_type, uri);
            let resolved = resolve_file_name("zshared_projection.prog.abap").unwrap();

            let source = resolved.source_ref(&entry).unwrap();

            assert_eq!(source.uri.as_str(), format!("{uri}/source/main"));
            assert_eq!(source.object.uri(), entry.reference.uri());
        }
    }

    #[test]
    fn rejects_an_inconsistent_repository_binding() {
        let class = repository_entry("ZCL_BOUND", "CLAS/OC", "/sap/bc/adt/oo/classes/zcl_bound");
        let program = repository_entry(
            "ZCL_BOUND",
            "PROG/P",
            "/sap/bc/adt/programs/programs/zcl_bound",
        );

        let wrong_name = resolve_file_name("zcl_other.clas.abap").unwrap();
        assert!(matches!(
            wrong_name.source_ref(&class),
            Err(ProjectionError::BindingNameMismatch { .. })
        ));

        let wrong_type = resolve_file_name("zcl_bound.clas.abap").unwrap();
        assert!(matches!(
            wrong_type.source_ref(&program),
            Err(ProjectionError::BindingTypeMismatch { .. })
        ));

        let metadata = resolve_file_name("zcl_bound.clas.json").unwrap();
        assert!(matches!(
            metadata.source_ref(&class),
            Err(ProjectionError::NotSourceFile {
                component: FileComponent::Metadata
            })
        ));
    }

    #[test]
    fn renders_and_resolves_every_supported_file_specification() {
        for format in [
            ObjectFormat::Program,
            ObjectFormat::Class,
            ObjectFormat::DataElement,
        ] {
            for specification in format.files() {
                let language =
                    matches!(specification.cardinality, Cardinality::ZeroOrMore).then_some("en-GB");
                let file_name = specification.file_name("Z_EXAMPLE", language).unwrap();
                let resolved = resolve_file_name(&file_name).unwrap();

                assert_eq!(resolved.object_name, "Z_EXAMPLE");
                assert_eq!(resolved.format, format);
                assert_eq!(resolved.component, specification.component);
                assert_eq!(resolved.language.as_deref(), language);
            }
        }
    }

    #[test]
    fn converts_namespaced_object_names_reversibly() {
        let specification = ObjectFormat::Class.files()[1];
        let file_name = specification.file_name("/DMO/ZCL_FLIGHT", None).unwrap();

        assert_eq!(file_name, "(dmo)zcl_flight.clas.abap");
        assert_eq!(
            resolve_file_name(&file_name).unwrap().object_name,
            "/DMO/ZCL_FLIGHT"
        );
    }

    #[test]
    fn language_is_required_only_for_language_dependent_files() {
        let text = ObjectFormat::Class
            .files()
            .iter()
            .copied()
            .find(|file| matches!(file.component, FileComponent::Text(TextComponent::Texts)))
            .unwrap();
        let source = ObjectFormat::Class.files()[1];

        assert!(matches!(
            text.file_name("ZCL_EXAMPLE", None),
            Err(ProjectionError::MissingLanguage { .. })
        ));
        assert!(matches!(
            source.file_name("ZCL_EXAMPLE", Some("en")),
            Err(ProjectionError::UnexpectedLanguage { .. })
        ));
        assert!(matches!(
            resolve_file_name("zcl_example.clas.texts.en--GB.properties"),
            Err(ProjectionError::InvalidLanguage { .. })
        ));
    }

    #[test]
    fn rejects_file_names_that_cannot_encode_one_object_name() {
        assert!(matches!(
            resolve_file_name("bad/name.clas.abap"),
            Err(ProjectionError::InvalidObjectName { .. })
        ));
        assert!(matches!(
            ObjectFormat::Class.files()[1].file_name("BAD/NAME", None),
            Err(ProjectionError::InvalidObjectName { .. })
        ));
    }
}
