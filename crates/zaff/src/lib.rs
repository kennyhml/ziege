#![doc = include_str!("../README.md")]

use std::path::Path;

use thiserror::Error;
use zadt::{AdtObject, GlobalWorkbenchType, SourceRef};

mod format;
mod formats;
mod language;
mod registry;

#[doc(hidden)]
pub use format::ProjectionObject;
pub use format::{
    Cardinality, ComponentId, FileBacking, FileSpec, ObjectFormat, ProjectedFile, Projection,
};
pub use formats::{
    AffAbapLanguageVersion, AffBasicDirection, AffBidirectionalOptions, AffClass,
    AffClassAbapLanguageVersion, AffClassCategory, AffClassDescriptions, AffClassHeader,
    AffDataElement, AffDataElementAdditionalProperties, AffDataElementCategory,
    AffDataElementFieldLabels, AffDataElementHeader, AffDataElementTypeInformation,
    AffEventDescription, AffLogicalDatabase, AffMethodDescription, AffNameDescription,
    AffPredefinedType, AffProgram, AffProgramGeneralInformation, AffProgramHeader,
    AffProgramStatus, AffProgramType, AffSearchHelp, CLASS_FORMAT, DATA_ELEMENT_FORMAT,
    PROGRAM_FORMAT,
};
pub use registry::formats;

impl TryFrom<&GlobalWorkbenchType> for ObjectFormat {
    type Error = ProjectionError;

    fn try_from(object_type: &GlobalWorkbenchType) -> Result<Self, Self::Error> {
        Self::for_workbench_type(object_type)
    }
}

impl<P> TryFrom<&AdtObject<P>> for ObjectFormat {
    type Error = ProjectionError;

    fn try_from(object: &AdtObject<P>) -> Result<Self, Self::Error> {
        Self::for_workbench_type(object.reference().object_type())
    }
}

/// Projects a runtime repository object into its currently materializable AFF files.
pub fn project<P>(object: &AdtObject<P>) -> Result<Projection, ProjectionError>
where
    AdtObject<P>: ProjectionObject,
{
    let descriptor = registry::for_workbench_type(object.reference().object_type())?;
    let format = descriptor.format();
    let mut files = Vec::new();
    for specification in descriptor.files() {
        if specification.template.contains("<lang>") {
            continue;
        }
        let Some(backing) = specification.bind(object, None)? else {
            continue;
        };
        files.push(ProjectedFile {
            name: specification.file_name(object.reference().name(), None)?,
            format,
            cardinality: specification.cardinality,
            component: specification.component(),
            backing,
            descriptor: specification.descriptor,
        });
    }
    Ok(Projection { format, files })
}

/// The semantic result of resolving an AFF path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFile {
    pub object_name: String,
    pub format: ObjectFormat,
    pub component: ComponentId,
    pub language: Option<String>,
}

impl ResolvedFile {
    /// Binds this projected path to the runtime repository object that produced it.
    pub fn bind<P>(&self, object: &AdtObject<P>) -> Result<FileBacking, ProjectionError>
    where
        AdtObject<P>: ProjectionObject,
    {
        if !self
            .object_name
            .eq_ignore_ascii_case(object.reference().name())
        {
            return Err(ProjectionError::BindingNameMismatch {
                projected_name: self.object_name.clone(),
                repository_name: object.reference().name().to_owned(),
            });
        }

        let repository_format = ObjectFormat::try_from(object)?;
        if self.format != repository_format {
            return Err(ProjectionError::BindingTypeMismatch {
                projected_type: self.format.object_type(),
                repository_type: object.reference().object_type().clone(),
            });
        }

        let specification = registry::by_format(self.format)
            .files()
            .iter()
            .find(|file| file.component() == self.component)
            .ok_or_else(|| ProjectionError::UnsupportedFileComponent {
                object_type: object.reference().object_type().clone(),
                component: self.component,
            })?;
        specification
            .bind(object, self.language.as_deref())?
            .ok_or_else(|| ProjectionError::UnsupportedFileComponent {
                object_type: object.reference().object_type().clone(),
                component: self.component,
            })
    }

    /// Resolves this projected file to its ADT source resource.
    pub fn source_ref<P>(&self, object: &AdtObject<P>) -> Result<SourceRef, ProjectionError>
    where
        AdtObject<P>: ProjectionObject,
    {
        let source_backed = registry::by_format(self.format)
            .files()
            .iter()
            .find(|file| file.component() == self.component)
            .is_some_and(|file| file.descriptor.is_source());
        if !source_backed {
            return Err(ProjectionError::NotSourceFile {
                component: self.component,
            });
        }
        match self.bind(object)? {
            FileBacking::Source(source) => Ok(source),
            FileBacking::Properties(_) => Err(ProjectionError::NotSourceFile {
                component: self.component,
            }),
        }
    }
}

/// Resolves an AFF path into its object family and logical component.
pub fn resolve_path(path: impl AsRef<Path>) -> Result<ResolvedFile, ProjectionError> {
    let file_name = path
        .as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProjectionError::NonUtf8Path)?;
    resolve_file_name(file_name)
}

/// Resolves an AFF file name into its object family and logical component.
pub fn resolve_file_name(file_name: &str) -> Result<ResolvedFile, ProjectionError> {
    for descriptor in registry::descriptors() {
        for specification in descriptor.files() {
            if let Some((object_name, language)) = match_template(specification.template, file_name)
            {
                if let Some(language) = language {
                    validate_language(language)?;
                }
                return Ok(ResolvedFile {
                    object_name: decode_object_name(object_name)?,
                    format: descriptor.format(),
                    component: specification.component(),
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

    #[error("AFF component `{component}` is not an ADT source resource")]
    NotSourceFile { component: ComponentId },

    #[error("AFF component `{component}` is not backed by ADT object properties")]
    NotPropertiesFile { component: ComponentId },

    #[error("repository object type `{object_type}` cannot supply AFF component `{component}`")]
    UnsupportedFileComponent {
        object_type: GlobalWorkbenchType,
        component: ComponentId,
    },

    #[error("properties for object `{actual}` cannot materialize projected object `{expected}`")]
    PropertiesBindingMismatch { expected: String, actual: String },

    #[error("invalid AFF Data Element document: {0}")]
    InvalidDataElementDocument(#[source] serde_json::Error),

    #[error("invalid AFF Class document: {0}")]
    InvalidClassDocument(#[source] serde_json::Error),

    #[error("invalid AFF Program document: {0}")]
    InvalidProgramDocument(#[source] serde_json::Error),

    #[error("invalid AFF field `{field}`: {message}")]
    InvalidAffField {
        field: &'static str,
        message: String,
    },

    #[error("AFF `{object_type}` field `{field}` is not available through ADT object properties")]
    UnsupportedAffProperty {
        object_type: &'static str,
        field: &'static str,
    },

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

    #[error(
        "ADT properties media type `{media_type}` is not supported for AFF object type `{object_type}`"
    )]
    UnsupportedPropertiesMediaType {
        object_type: &'static str,
        media_type: String,
    },

    #[error("invalid ADT properties for AFF object type `{object_type}`: {source}")]
    InvalidAdtProperties {
        object_type: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "ADT properties for AFF object type `{object_type}` identify `{actual}`, expected `{expected}`"
    )]
    InvalidPropertiesIdentity {
        object_type: &'static str,
        expected: String,
        actual: String,
    },
}

#[cfg(test)]
mod test_support {
    use http::{HeaderMap, StatusCode};
    use zadt::{
        AdtObject, AdtResponse, AdtUri, ObjectPropertiesQuery, ObjectRef, ObjectType, Operation,
        OperationResponse, Ready, RepositoryContentQuery, RepositoryObjectEntry,
    };

    pub fn repository_entry(name: &str, object_type: &str, uri: &str) -> RepositoryObjectEntry {
        let body = format!(
            r#"<vfs:virtualFoldersResult xmlns:vfs="http://www.sap.com/adt/ris/virtualFolders" objectCount="1">
                <vfs:object name="{name}" package="$TMP" type="{object_type}"
                    uri="{uri}" expandable="false" />
            </vfs:virtualFoldersResult>"#,
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

    pub fn reference<T: ObjectType>(name: &str, uri: &str) -> ObjectRef<T> {
        repository_entry(name, T::WORKBENCH_TYPE.as_str(), uri)
            .typed_reference::<T>()
            .unwrap()
    }

    pub fn properties<T>(
        reference: &ObjectRef<T>,
        media_type: &'static str,
        etag: &'static str,
        body: &[u8],
    ) -> AdtObject<T::Properties>
    where
        T: ObjectType,
    {
        let mut headers = HeaderMap::new();
        headers.insert(http::header::CONTENT_TYPE, media_type.parse().unwrap());
        headers.insert(http::header::ETAG, etag.parse().unwrap());
        let response = AdtResponse::new(StatusCode::OK, headers, body.to_vec());
        let target = reference.uri().clone();
        let query = reference.query();
        <ObjectPropertiesQuery<T> as Operation<Ready>>::decode(
            &query,
            OperationResponse::new(response, target),
        )
        .unwrap()
    }

    pub fn json_properties<T>(
        reference: &ObjectRef<T>,
        media_type: &'static str,
        etag: &'static str,
        body: &[u8],
    ) -> AdtObject
    where
        T: ObjectType,
    {
        let reference = reference.erase();
        let query = reference.query().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(http::header::CONTENT_TYPE, media_type.parse().unwrap());
        headers.insert(http::header::ETAG, etag.parse().unwrap());
        let response = AdtResponse::new(StatusCode::OK, headers, body.to_vec());
        let target = reference.uri().clone();
        <ObjectPropertiesQuery<()> as Operation<Ready>>::decode(
            &query,
            OperationResponse::new(response, target),
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use zadt::{
        Class, ClassPropertiesVersion, DataElement, EntityTag, Include, ObjectType, Program,
        ProgramPropertiesVersion,
    };

    use super::*;

    const CLASS_XML: &[u8] =
        include_bytes!("../../zadt/tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const PROGRAM_XML: &[u8] = include_bytes!("../../zadt/tests/fixtures/program-z-test.xml");

    #[test]
    fn registry_maps_supported_repository_types() {
        assert_eq!(
            ObjectFormat::for_workbench_type(&Class::WORKBENCH_TYPE).unwrap(),
            CLASS_FORMAT
        );
        assert_eq!(
            ObjectFormat::for_workbench_type(&Program::WORKBENCH_TYPE).unwrap(),
            PROGRAM_FORMAT
        );
        assert_eq!(
            ObjectFormat::for_workbench_type(&Include::WORKBENCH_TYPE).unwrap(),
            PROGRAM_FORMAT
        );
        assert_eq!(
            ObjectFormat::for_workbench_type(&DataElement::WORKBENCH_TYPE).unwrap(),
            DATA_ELEMENT_FORMAT
        );
    }

    #[test]
    fn paths_round_trip_namespaced_objects() {
        let metadata = CLASS_FORMAT.files()[0];
        let file_name = metadata.file_name("/ACME/DEMO", None).unwrap();
        assert_eq!(file_name, "(acme)demo.clas.json");
        assert_eq!(
            resolve_file_name(&file_name).unwrap(),
            ResolvedFile {
                object_name: "/ACME/DEMO".to_owned(),
                format: CLASS_FORMAT,
                component: ComponentId::new("metadata"),
                language: None,
            }
        );
    }

    #[test]
    fn metadata_infers_shared_program_types() {
        assert_eq!(
            PROGRAM_FORMAT
                .repository_type_from_metadata(
                    br#"{"formatVersion":"1","generalInformation":{"programType":"include"}}"#,
                )
                .unwrap(),
            Include::WORKBENCH_TYPE
        );
        assert_eq!(
            PROGRAM_FORMAT
                .repository_type_from_metadata(br#"{"formatVersion":"1"}"#)
                .unwrap(),
            Program::WORKBENCH_TYPE
        );
    }

    #[test]
    fn every_registered_file_specification_round_trips() {
        for format in formats() {
            for specification in format.files() {
                let language =
                    matches!(specification.cardinality, Cardinality::ZeroOrMore).then_some("en-GB");
                let file_name = specification.file_name("Z_EXAMPLE", language).unwrap();
                let resolved = resolve_file_name(&file_name).unwrap();

                assert_eq!(resolved.object_name, "Z_EXAMPLE");
                assert_eq!(resolved.format, format);
                assert_eq!(resolved.component, specification.component());
                assert_eq!(resolved.language.as_deref(), language);
            }
        }
    }

    #[test]
    fn file_specs_validate_language_usage() {
        let text = CLASS_FORMAT
            .files()
            .iter()
            .copied()
            .find(|file| file.component().as_str() == "text/texts")
            .unwrap();
        let source = CLASS_FORMAT
            .files()
            .iter()
            .copied()
            .find(|file| file.component().as_str() == "source/main")
            .unwrap();

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
    fn resolved_class_sources_bind_through_their_descriptors() {
        let reference = test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        let object = test_support::properties(
            &reference,
            ClassPropertiesVersion::V4.media_type(),
            "class-etag",
            CLASS_XML,
        );

        for (file_name, expected_uri) in [
            (
                "cl_adt_uri_mapper.clas.abap",
                "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main",
            ),
            (
                "cl_adt_uri_mapper.clas.testclasses.abap",
                "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/includes/testclasses",
            ),
        ] {
            let backing = resolve_file_name(file_name).unwrap().bind(&object).unwrap();
            let FileBacking::Source(source) = backing else {
                panic!("source file must have a source backing");
            };
            assert_eq!(source.uri.as_str(), expected_uri);
            assert_eq!(source.object.uri(), reference.uri());
        }

        assert!(matches!(
            resolve_file_name("cl_adt_uri_mapper.clas.locals.abap")
                .unwrap()
                .bind(&object),
            Err(ProjectionError::UnsupportedFileComponent { .. })
        ));
    }

    #[test]
    fn language_properties_are_not_reported_as_source_files() {
        let reference = test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        let object = test_support::properties(
            &reference,
            ClassPropertiesVersion::V4.media_type(),
            "class-etag",
            CLASS_XML,
        );
        let resolved = resolve_file_name("cl_adt_uri_mapper.clas.texts.en.properties").unwrap();

        assert!(matches!(
            resolved.source_ref(&object),
            Err(ProjectionError::NotSourceFile { .. })
        ));
    }

    #[test]
    fn bindings_reject_the_wrong_name_and_object_family() {
        let class_reference = test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        let class = test_support::properties(
            &class_reference,
            ClassPropertiesVersion::V4.media_type(),
            "class-etag",
            CLASS_XML,
        );
        let program_reference =
            test_support::reference::<Program>("Z_TEST", "/sap/bc/adt/programs/programs/z_test");
        let program = test_support::properties(
            &program_reference,
            ProgramPropertiesVersion::V3.media_type(),
            "program-etag",
            PROGRAM_XML,
        );

        assert!(matches!(
            resolve_file_name("zcl_other.clas.abap")
                .unwrap()
                .bind(&class),
            Err(ProjectionError::BindingNameMismatch { .. })
        ));
        assert!(matches!(
            resolve_file_name("z_test.clas.abap")
                .unwrap()
                .bind(&program),
            Err(ProjectionError::BindingTypeMismatch { .. })
        ));
    }

    #[test]
    fn metadata_rejects_unknown_versions_and_program_types() {
        assert!(matches!(
            PROGRAM_FORMAT.repository_type_from_metadata(br#"{"formatVersion":"2"}"#),
            Err(ProjectionError::UnsupportedFormatVersion { .. })
        ));
        assert!(matches!(
            PROGRAM_FORMAT.repository_type_from_metadata(
                br#"{"formatVersion":"1","generalInformation":{"programType":"unknown"}}"#,
            ),
            Err(ProjectionError::UnsupportedProgramType { .. })
        ));
    }

    #[test]
    fn projected_metadata_uses_the_runtime_properties_codec() {
        let reference = test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        let properties = test_support::json_properties(
            &reference,
            ClassPropertiesVersion::V4.media_type(),
            "class-etag",
            CLASS_XML,
        );
        let projection = project(&properties).unwrap();
        let metadata = projection
            .files
            .iter()
            .find(|file| file.component().as_str() == "metadata")
            .unwrap();

        assert!(
            matches!(metadata.backing(), FileBacking::Properties(object) if object.uri() == reference.uri())
        );
        let rendered = metadata.render_properties(&properties).unwrap();
        let edited = rendered.replacen("URI Mapper", "Updated class", 1);
        let merged = metadata.merge_properties(&properties, &edited).unwrap();

        assert_eq!(merged.media_type(), properties.media_type());
        assert_eq!(
            merged.etag.as_ref().map(EntityTag::as_str),
            Some("class-etag")
        );
        assert_eq!(merged.properties["@adtcore:description"], "Updated class");
        assert_eq!(
            merged.properties["adtcore:packageRef"],
            properties.properties["adtcore:packageRef"]
        );

        let other = test_support::reference::<Class>("CL_OTHER", "/sap/bc/adt/oo/classes/cl_other");
        let other_xml = String::from_utf8_lossy(CLASS_XML).replacen(
            "adtcore:name=\"CL_ADT_URI_MAPPER\"",
            "adtcore:name=\"CL_OTHER\"",
            1,
        );
        let other_properties = test_support::json_properties(
            &other,
            ClassPropertiesVersion::V4.media_type(),
            "other-etag",
            other_xml.as_bytes(),
        );
        assert!(matches!(
            metadata.render_properties(&other_properties),
            Err(ProjectionError::PropertiesBindingMismatch { .. })
        ));
    }
}
