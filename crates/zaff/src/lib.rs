#![doc = include_str!("../README.md")]

use std::sync::Arc;

use thiserror::Error;
use zadt::{GlobalWorkbenchType, ObjectSnapshot, SourceRef};

mod filename;
mod formats;
mod helpers;
mod models;
mod registry;
mod validate;

pub use filename::encode_object_name;
pub use formats::*;
pub use models::{AbapLanguageVersion, CdsHeader, CdsSourceOrigin};

/// The available AFF files derived from one immutable, loaded ADT snapshot.
///
/// The snapshot is shared with its properties files. Source text, dirty buffers,
/// cache invalidation, and save orchestration belong to the caller.
#[derive(Clone, Debug)]
pub struct Projection {
    snapshot: Arc<ObjectSnapshot<()>>,
    format: &'static ObjectFormat,
    files: Vec<FileProjection>,
}

impl Projection {
    /// The original loaded ADT object represented by this projection.
    /// This observation is immutable and is not automatically refreshed.
    pub fn subject(&self) -> &ObjectSnapshot<()> {
        &self.snapshot
    }

    pub const fn format(&self) -> &'static ObjectFormat {
        self.format
    }

    pub fn files(&self) -> &[FileProjection] {
        &self.files
    }

    /// Finds an available file by its canonical AFF filename, not a filesystem path.
    pub fn file(&self, name: &str) -> Option<&FileProjection> {
        self.files.iter().find(|file| file.name() == name)
    }
}

/// One concrete AFF filename bound to its ADT source or properties mapping.
///
/// This is not a loaded editor document. Source text remains unfetched, and
/// rendering properties can fail if their values cannot be represented in AFF.
#[derive(Clone, Debug)]
pub struct FileProjection {
    name: String,
    specification: &'static FileSpec,
    backing: FileBacking,
}

impl FileProjection {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn specification(&self) -> &'static FileSpec {
        self.specification
    }

    /// Returns the operations available for this file's kind of backing.
    pub fn backing(&self) -> &FileBacking {
        &self.backing
    }
}

/// An AFF properties mapping bound to the snapshot from which it was projected.
///
/// Rendering and merging always use the same immutable baseline. Cloning this
/// value shares that baseline; it does not clone the complete ADT properties.
#[derive(Clone, Debug)]
pub struct PropertiesProjection {
    snapshot: Arc<ObjectSnapshot<()>>,
    mapping: &'static PropertiesMapping,
}

impl PropertiesProjection {
    /// The original loaded ADT object, including its properties and update validator.
    /// This observation is immutable and is not automatically refreshed.
    pub fn subject(&self) -> &ObjectSnapshot<()> {
        &self.snapshot
    }

    /// Renders the represented AFF fields as canonical JSON with a trailing newline.
    pub fn render(&self) -> Result<String, ProjectionError> {
        (self.mapping.render)(&self.snapshot)
    }

    /// Validates edited AFF JSON and merges its changes into the complete ADT properties.
    ///
    /// Returns `None` if the validated result equals the retained baseline, otherwise
    /// `Some` ADT wire-shaped JSON, not AFF JSON. Submit the changed properties through
    /// `self.subject().update_if_match(...)` or `update_with_lock(...)`.
    /// A no-op does not establish that the backend still matches this observation.
    /// Neither this method nor a successful save advances the retained baseline;
    /// obtain a fresh snapshot and project it again after saving.
    pub fn merge(&self, edited: &str) -> Result<Option<serde_json::Value>, ProjectionError> {
        (self.mapping.merge)(&self.snapshot, edited)
    }
}

/// How to read or edit a projected file.
#[derive(Clone, Debug)]
pub enum FileBacking {
    /// Text is fetched through this advertised reference.
    Source(SourceRef),
    /// AFF JSON is rendered and merged against the retained properties baseline.
    Properties(PropertiesProjection),
}

/// Projects a loaded ADT snapshot into its currently available AFF files without I/O.
///
/// Typed callers use `snapshot.into_erased()`. The projection retains this exact
/// baseline for properties rendering and merging; source text remains unfetched.
/// Missing sources are omitted, while malformed advertised resources return an
/// error. Recognized file specifications without an implemented mapping are omitted.
pub fn project(snapshot: ObjectSnapshot<()>) -> Result<Projection, ProjectionError> {
    let format = registry::for_workbench_type(snapshot.key().workbench_type())?;
    let snapshot = Arc::new(snapshot);

    let mut files = Vec::new();
    for filespec in format.files() {
        if let Some(file) = filespec.bind(&snapshot)? {
            files.push(file);
        }
    }

    Ok(Projection {
        snapshot,
        format,
        files,
    })
}

/// An error mapping between ADT repository objects and AFF files.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionError {
    #[error("repository object type `{workbench_type}` has no supported AFF projection")]
    UnsupportedRepositoryType { workbench_type: GlobalWorkbenchType },

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

    #[error("JSON conversion failed: {0}")]
    Json(#[from] serde_json::Error),

    /// Validation failures use Rust field names and nested collection indices.
    #[error("AFF validation failed: {0}")]
    Validation(#[from] garde::Report),
}

#[cfg(test)]
mod test_support {
    use http::{HeaderMap, StatusCode};
    use zadt::{
        AdtResponse, AdtUri, ObjectQuery, ObjectRef, ObjectSnapshot, ObjectType, Operation,
        OperationResponse, RepositoryContentQuery, RepositoryObjectEntry,
    };

    pub fn repository_entry(name: &str, workbench_type: &str, uri: &str) -> RepositoryObjectEntry {
        let body = format!(
            r#"<vfs:virtualFoldersResult xmlns:vfs="http://www.sap.com/adt/ris/virtualFolders" objectCount="1">
                <vfs:object name="{name}" package="$TMP" type="{workbench_type}"
                    uri="{uri}" expandable="false" />
            </vfs:virtualFoldersResult>"#,
        );
        let response = AdtResponse::new(StatusCode::OK, HeaderMap::new(), body.into_bytes());
        let target =
            AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents")
                .unwrap();
        let mut content = <RepositoryContentQuery as Operation>::decode(
            &RepositoryContentQuery::new(),
            OperationResponse::new(response, target),
        )
        .unwrap();
        content.objects.pop().unwrap()
    }

    pub fn reference<T: ObjectType>(name: &str, uri: &str) -> ObjectRef<T> {
        ObjectRef::try_from(&repository_entry(name, T::WORKBENCH_TYPE.as_str(), uri)).unwrap()
    }

    pub fn properties<T>(
        reference: &ObjectRef<T>,
        media_type: &'static str,
        etag: &'static str,
        body: &[u8],
    ) -> ObjectSnapshot<T>
    where
        T: ObjectType,
    {
        let mut headers = HeaderMap::new();
        headers.insert(http::header::CONTENT_TYPE, media_type.parse().unwrap());
        headers.insert(http::header::ETAG, etag.parse().unwrap());
        let response = AdtResponse::new(StatusCode::OK, headers, body.to_vec());
        let target = reference.uri().clone();
        let query = reference.query();
        <ObjectQuery<T> as Operation>::decode(&query, OperationResponse::new(response, target))
            .unwrap()
    }

    pub fn erased_properties<T>(
        reference: &ObjectRef<T>,
        media_type: &'static str,
        etag: &'static str,
        body: &[u8],
    ) -> ObjectSnapshot<()>
    where
        T: ObjectType,
    {
        let reference = reference.erase();
        let query = reference.query();
        let mut headers = HeaderMap::new();
        headers.insert(http::header::CONTENT_TYPE, media_type.parse().unwrap());
        headers.insert(http::header::ETAG, etag.parse().unwrap());
        let response = AdtResponse::new(StatusCode::OK, headers, body.to_vec());
        let target = reference.uri().clone();
        <ObjectQuery<()> as Operation>::decode(&query, OperationResponse::new(response, target))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use zadt::{
        Class, ClassProperties, DataElement, Domain, EntityTag, Include, ObjectType,
        Operation, Program, ToXml, WorkbenchVersion, XmlCodec,
    };

    use super::*;
    use crate::filename::encode_object_name;
    use crate::formats::{clas::CLASS_FORMAT, dtel::DATA_ELEMENT_FORMAT, prog::PROGRAM_FORMAT};
    use crate::prog::{ProgramType, ProjectedProgramProperties};

    const CLASS_XML: &[u8] =
        include_bytes!("../../zadt/tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const PROGRAM_XML: &[u8] = include_bytes!("../../zadt/tests/fixtures/program-z-test.xml");
    const LEGACY_CLASS_XML: &[u8] =
        include_bytes!("../../zadt/tests/fixtures/class-cx-root-v4.xml");
    const INCLUDE_XML: &[u8] = include_bytes!("../../zadt/tests/fixtures/include-ztest.xml");
    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../zadt/tests/fixtures/data-element-ztfrwtfrt-v2.xml");
    const CLASS_URI: &str = "/sap/bc/adt/oo/classes/cl_adt_uri_mapper";

    fn fixture_snapshots<T: ObjectType>(
        name: &str,
        uri: &str,
        xml: &[u8],
    ) -> [ObjectSnapshot<()>; 2] {
        let reference = test_support::reference::<T>(name, uri);
        [
            test_support::properties(&reference, T::MEDIA_TYPES[0], "object-etag", xml)
                .into_erased(),
            test_support::erased_properties(&reference, T::MEDIA_TYPES[0], "object-etag", xml),
        ]
    }

    #[test]
    fn json_conversion_errors_preserve_the_serde_cause() {
        for source in [
            serde_json::to_value(std::collections::BTreeMap::from([((1, 2), 3)])).unwrap_err(),
            serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
        ] {
            let message = source.to_string();
            let error = ProjectionError::from(source);

            assert!(matches!(&error, ProjectionError::Json(_)));
            assert_eq!(
                std::error::Error::source(&error).unwrap().to_string(),
                message
            );
            assert_eq!(
                error.to_string(),
                format!("JSON conversion failed: {message}")
            );
        }
    }

    #[test]
    fn unsupported_snapshot_families_fail_projection() {
        for snapshots in [
            fixture_snapshots::<Domain>(
                "XFELD",
                "/sap/bc/adt/ddic/domains/xfeld",
                include_bytes!("../../zadt/tests/fixtures/domain-xfeld.xml"),
            ),
        ] {
            for snapshot in snapshots {
                let expected = snapshot.reference().workbench_type().clone();
                assert!(matches!(
                    project(snapshot),
                    Err(ProjectionError::UnsupportedRepositoryType { workbench_type })
                        if workbench_type == expected
                ));
            }
        }
    }

    #[test]
    fn object_name_encoding_rejects_invalid_filename_inputs() {
        for name in [
            "",
            " Z_TEST",
            "Z_TEST ",
            "Z\nTEST",
            "Z\\TEST",
            "<name>",
            ".",
            "..",
            "/ACME",
            "//NAME",
            "/ACME/",
            "/ACME/NAME/EXTRA",
            "Z/TEST",
            "(acme)name",
            "/AC(ME/NAME",
            "/ACME/NA)ME",
        ] {
            assert!(matches!(
                encode_object_name(name),
                Err(ProjectionError::InvalidObjectName { object_name }) if object_name == name
            ));
        }
        assert_eq!(encode_object_name("Z_\u{00c4}").unwrap(), "z_\u{00c4}");
    }

    #[test]
    fn file_specifications_generate_plain_and_namespaced_names() {
        for format in [&CLASS_FORMAT, &PROGRAM_FORMAT, &DATA_ELEMENT_FORMAT] {
            for specification in format.files() {
                let language = matches!(specification.cardinality(), Cardinality::ZeroOrMore)
                    .then_some("en-GB");
                assert!(specification.template().starts_with("<name>"));
                for (name, encoded) in [("Z_EXAMPLE", "z_example"), ("/ACME/DEMO", "(acme)demo")] {
                    assert_eq!(
                        specification.filename(name, None, language).unwrap(),
                        specification
                            .template()
                            .replace("<name>", encoded)
                            .replace("<lang>", "en-GB")
                    );
                }
            }
        }
    }

    #[test]
    fn namespaced_snapshot_projects_encoded_file_names() {
        let xml = String::from_utf8_lossy(CLASS_XML).replace("CL_ADT_URI_MAPPER", "/ACME/DEMO");
        for snapshot in fixture_snapshots::<Class>(
            "/ACME/DEMO",
            "/sap/bc/adt/oo/classes/%2Facme%2Fdemo",
            xml.as_bytes(),
        ) {
            let projection = project(snapshot).unwrap();
            assert_eq!(projection.subject().reference().name(), "/ACME/DEMO");
            assert_eq!(projection.format(), &CLASS_FORMAT);
            let file = projection.file("(acme)demo.clas.json").unwrap();
            assert_eq!(file.specification().template(), "<name>.clas.json");
            assert!(matches!(file.backing(), FileBacking::Properties(_)));
            assert!(projection.file("(acme)demo.clas.abap").is_some());
        }
    }

    #[test]
    fn file_specs_validate_language_usage() {
        let text = CLASS_FORMAT
            .files()
            .iter()
            .copied()
            .find(|file| file.template() == "<name>.clas.texts.<lang>.properties")
            .unwrap();
        let source = CLASS_FORMAT
            .files()
            .iter()
            .copied()
            .find(|file| file.template() == "<name>.clas.abap")
            .unwrap();

        assert!(matches!(
            text.filename("ZCL_EXAMPLE", None, None),
            Err(ProjectionError::MissingLanguage { .. })
        ));
        assert!(matches!(
            source.filename("ZCL_EXAMPLE", None, Some("en")),
            Err(ProjectionError::UnexpectedLanguage { .. })
        ));
        assert!(matches!(
            text.filename("ZCL_EXAMPLE", None, Some("en--GB")),
            Err(ProjectionError::InvalidLanguage { .. })
        ));
    }

    #[test]
    fn projected_class_sources_retain_advertised_locations_and_validators() {
        let reference = test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        let object =
            test_support::properties(&reference, Class::MEDIA_TYPES[0], "class-etag", CLASS_XML);
        let projection = project(object.into_erased()).unwrap();

        for (file_name, expected_uri, expected_etag) in [
            (
                "cl_adt_uri_mapper.clas.abap",
                "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main",
                "20210406145501001000181",
            ),
            (
                "cl_adt_uri_mapper.clas.testclasses.abap",
                "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/includes/testclasses",
                "202003161335160011",
            ),
        ] {
            let file = projection.file(file_name).unwrap();
            let FileBacking::Source(source) = file.backing() else {
                panic!("source file must have a source backing");
            };
            assert_eq!(source.uri.as_str(), expected_uri);
            assert_eq!(source.object.uri(), reference.uri());
            assert_eq!(source.etag.as_deref(), Some(expected_etag));
        }

        assert!(
            projection
                .file("cl_adt_uri_mapper.clas.locals.abap")
                .is_none()
        );
    }

    #[test]
    fn language_properties_are_described_but_never_projected() {
        for snapshots in [
            fixture_snapshots::<Class>("CL_ADT_URI_MAPPER", CLASS_URI, CLASS_XML),
            fixture_snapshots::<Program>(
                "Z_TEST",
                "/sap/bc/adt/programs/programs/z_test",
                PROGRAM_XML,
            ),
            fixture_snapshots::<Include>(
                "ZTEST",
                "/sap/bc/adt/programs/includes/ztest",
                INCLUDE_XML,
            ),
        ] {
            for snapshot in snapshots {
                let projection = project(snapshot).unwrap();
                let specifications: Vec<_> = projection
                    .format()
                    .files()
                    .iter()
                    .filter(|specification| specification.template().contains("<lang>"))
                    .collect();
                assert_eq!(
                    specifications.len(),
                    if projection.format() == &CLASS_FORMAT {
                        1
                    } else {
                        3
                    }
                );
                for specification in specifications {
                    assert!(!specification.is_supported());
                    assert_eq!(specification.cardinality(), Cardinality::ZeroOrMore);
                    let name = specification
                        .filename(projection.subject().reference().name(), None, Some("en-GB"))
                        .unwrap();
                    assert!(projection.file(&name).is_none());
                    assert!(
                        !projection
                            .files()
                            .iter()
                            .any(|file| std::ptr::eq(file.specification(), specification))
                    );
                }
            }
        }
    }

    #[test]
    fn bound_metadata_merge_rejects_unknown_versions_and_program_types() {
        for snapshots in [
            fixture_snapshots::<Program>(
                "Z_TEST",
                "/sap/bc/adt/programs/programs/z_test",
                PROGRAM_XML,
            ),
            fixture_snapshots::<Include>(
                "ZTEST",
                "/sap/bc/adt/programs/includes/ztest",
                INCLUDE_XML,
            ),
        ] {
            for snapshot in snapshots {
                let projection = project(snapshot).unwrap();
                let name = format!(
                    "{}.prog.json",
                    projection.subject().reference().name().to_ascii_lowercase()
                );
                let FileBacking::Properties(properties) = projection.file(&name).unwrap().backing()
                else {
                    panic!("metadata must have a properties backing");
                };
                let baseline = properties.render().unwrap();
                let mut document: serde_json::Value = serde_json::from_str(&baseline).unwrap();
                document["formatVersion"] = "2".into();
                assert!(matches!(
                    properties.merge(&document.to_string()),
                    Err(ProjectionError::Validation(_))
                ));
                document["formatVersion"] = "1".into();
                document["generalInformation"]["programType"] = "unknown".into();
                assert!(matches!(
                    properties.merge(&document.to_string()),
                    Err(ProjectionError::Json(_))
                ));
                assert_eq!(properties.render().unwrap(), baseline);
                assert_eq!(properties.merge(&baseline).unwrap(), None);
            }
        }
    }

    #[test]
    fn properties_merge_returns_payload_only_for_validated_changes() {
        use serde_json::{Value, json};

        for snapshots in [
            fixture_snapshots::<Class>("CL_ADT_URI_MAPPER", CLASS_URI, CLASS_XML),
            fixture_snapshots::<Program>(
                "Z_TEST",
                "/sap/bc/adt/programs/programs/z_test",
                PROGRAM_XML,
            ),
            fixture_snapshots::<Include>(
                "ZTEST",
                "/sap/bc/adt/programs/includes/ztest",
                INCLUDE_XML,
            ),
            fixture_snapshots::<DataElement>(
                "ZTFRWTFRT",
                "/sap/bc/adt/ddic/dataelements/ztfrwtfrt",
                DATA_ELEMENT_XML,
            ),
        ] {
            for snapshot in snapshots {
                let original = snapshot.properties().unwrap();
                let projection = project(snapshot).unwrap();
                let FileBacking::Properties(properties) = projection.files()[0].backing() else {
                    panic!("first file must have a properties backing");
                };
                let baseline = properties.render().unwrap();
                let mut document: Value = serde_json::from_str(&baseline).unwrap();
                let compact = document.to_string();
                assert_ne!(compact, baseline);
                assert_eq!(properties.merge(&compact).unwrap(), None);
                let reordered = format!(
                    "{{{}}}",
                    document
                        .as_object()
                        .unwrap()
                        .iter()
                        .rev()
                        .map(|(key, value)| format!(
                            "{}:{value}",
                            serde_json::to_string(key).unwrap()
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                assert_ne!(reordered, compact);
                assert_eq!(properties.merge(&reordered).unwrap(), None);

                match projection.format().object_type() {
                    "CLAS" => {
                        document["header"]["abapLanguageVersion"] = json!("standard");
                        document["category"] = json!("generalObjectType");
                        document["descriptions"] = json!({});
                    }
                    "PROG" => {
                        document["generalInformation"]["programStatus"] = json!("unknown");
                        document["logicalDatabase"] = json!({});
                    }
                    "DTEL" => {
                        document["header"]["abapLanguageVersion"] = json!("standard");
                        let additional = document
                            .as_object_mut()
                            .unwrap()
                            .entry("additionalProperties")
                            .or_insert_with(|| json!({}));
                        additional
                            .as_object_mut()
                            .unwrap()
                            .entry("noInputHistory")
                            .or_insert(json!(false));
                    }
                    _ => unreachable!(),
                }
                assert_eq!(properties.merge(&document.to_string()).unwrap(), None);

                assert!(matches!(
                    properties.merge("{"),
                    Err(ProjectionError::Json(_))
                ));
                let mut invalid = document.clone();
                invalid["formatVersion"] = json!("2");
                assert!(matches!(
                    properties.merge(&invalid.to_string()),
                    Err(ProjectionError::Validation(_))
                ));
                invalid = document.clone();
                invalid["header"]["originalLanguage"] = json!("not-supported");
                assert!(matches!(
                    properties.merge(&invalid.to_string()),
                    Err(ProjectionError::InvalidAffField {
                        field: "header.originalLanguage",
                        ..
                    })
                ));

                invalid = document.clone();
                match projection.format().object_type() {
                    "CLAS" => {
                        invalid["descriptions"] = json!({
                            "methods": [{"name": "RUN", "description": "Unsupported edit"}]
                        });
                        assert!(matches!(
                            properties.merge(&invalid.to_string()),
                            Err(ProjectionError::UnsupportedAffProperty {
                                field: "descriptions",
                                ..
                            })
                        ));
                    }
                    "PROG" => {
                        invalid["generalInformation"]["programStatus"] = json!("testProgram");
                        assert!(matches!(
                            properties.merge(&invalid.to_string()),
                            Err(ProjectionError::UnsupportedAffProperty {
                                field: "generalInformation.programStatus",
                                ..
                            })
                        ));
                    }
                    "DTEL" => {
                        invalid["dataTypeInformation"]["category"] = json!("domain");
                        invalid["dataTypeInformation"]["predefinedType"] =
                            json!({"dataType": "CHAR", "length": 1});
                        assert!(matches!(
                            properties.merge(&invalid.to_string()),
                            Err(ProjectionError::InvalidDataElementField {
                                field: "dataTypeInformation.predefinedType",
                                ..
                            })
                        ));
                    }
                    _ => unreachable!(),
                }

                document["header"]["description"] = json!("Changed properties");
                let mut expected = original.clone();
                expected["@adtcore:description"] = json!("Changed properties");
                assert_eq!(
                    properties.merge(&document.to_string()).unwrap(),
                    Some(expected)
                );
                assert_eq!(properties.subject().properties().unwrap(), original);
                assert_eq!(properties.render().unwrap(), baseline);
            }
        }
    }

    #[test]
    fn projected_metadata_uses_the_runtime_properties_codec() {
        let reference = test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        let properties = test_support::erased_properties(
            &reference,
            Class::MEDIA_TYPES[0],
            "class-etag",
            CLASS_XML,
        );
        let original_properties = properties.properties().unwrap();
        let projection = project(properties).unwrap();
        let file = projection.file("cl_adt_uri_mapper.clas.json").unwrap();
        let FileBacking::Properties(metadata) = file.backing() else {
            panic!("metadata must have a properties backing");
        };
        assert!(std::ptr::eq(metadata.subject(), projection.subject()));
        assert_eq!(metadata.subject().reference().uri(), reference.uri());
        let metadata = metadata.clone();
        drop(projection);
        let rendered = metadata.render().unwrap();
        let edited = rendered.replacen("URI Mapper", "Updated class", 1);
        let merged = metadata.merge(&edited).unwrap().unwrap();

        let mut expected = original_properties.clone();
        expected["@adtcore:description"] = "Updated class".into();
        assert_eq!(merged, expected);
        assert_eq!(original_properties["@adtcore:description"], "URI Mapper");

        for (uri, etag, description) in [
            (
                "/sap/bc/adt/custom/classes/cl_adt_uri_mapper",
                "relocated-etag",
                "Relocated class",
            ),
            (CLASS_URI, "new-etag", "New class version"),
        ] {
            let other = test_support::reference::<Class>(reference.name(), uri);
            assert_eq!(other.key(), reference.key());
            let other_xml = String::from_utf8_lossy(CLASS_XML)
                .replacen("URI Mapper", description, 1)
                .replace("SADT_TOOLS_CORE", "OTHER_PACKAGE");
            let other_projection = project(test_support::erased_properties(
                &other,
                Class::MEDIA_TYPES[0],
                etag,
                other_xml.as_bytes(),
            ))
            .unwrap();
            let FileBacking::Properties(other_metadata) = other_projection
                .file("cl_adt_uri_mapper.clas.json")
                .unwrap()
                .backing()
            else {
                panic!("metadata must have a properties backing");
            };
            assert!(std::ptr::eq(
                other_metadata.subject(),
                other_projection.subject()
            ));
            assert!(!std::ptr::eq(metadata.subject(), other_metadata.subject()));
            assert_eq!(other_metadata.subject().reference().uri(), other.uri());
            assert_eq!(
                other_metadata.subject().etag().map(EntityTag::as_str),
                Some(etag)
            );
            let other_rendered: serde_json::Value =
                serde_json::from_str(&other_metadata.render().unwrap()).unwrap();
            assert_eq!(other_rendered["header"]["description"], description);
            let other_merged = other_metadata.merge(&edited).unwrap().unwrap();
            assert_eq!(other_merged["@adtcore:description"], "Updated class");
            assert_eq!(
                other_merged["adtcore:packageRef"]["@adtcore:name"],
                "OTHER_PACKAGE"
            );
            drop(other_projection);

            assert_eq!(metadata.subject().reference().uri(), reference.uri());
            assert_eq!(
                metadata.subject().etag().map(EntityTag::as_str),
                Some("class-etag")
            );
            assert_eq!(
                metadata.subject().properties().unwrap(),
                original_properties
            );
            assert_eq!(metadata.render().unwrap(), rendered);
            assert_eq!(metadata.merge(&edited).unwrap().unwrap(), expected);
        }
    }

    #[test]
    fn exact_file_inventories_match_for_typed_and_erased_snapshots() {
        for (snapshots, format, expected) in [
            (
                fixture_snapshots::<Class>("CL_ADT_URI_MAPPER", CLASS_URI, CLASS_XML),
                &CLASS_FORMAT,
                &[
                    ("cl_adt_uri_mapper.clas.json", None),
                    ("cl_adt_uri_mapper.clas.abap", Some("main")),
                    (
                        "cl_adt_uri_mapper.clas.definitions.abap",
                        Some("definitions"),
                    ),
                    (
                        "cl_adt_uri_mapper.clas.implementations.abap",
                        Some("implementations"),
                    ),
                    ("cl_adt_uri_mapper.clas.macros.abap", Some("macros")),
                    (
                        "cl_adt_uri_mapper.clas.testclasses.abap",
                        Some("testclasses"),
                    ),
                ][..],
            ),
            (
                fixture_snapshots::<Class>(
                    "CX_ROOT",
                    "/sap/bc/adt/oo/classes/cx_root",
                    LEGACY_CLASS_XML,
                ),
                &CLASS_FORMAT,
                &[
                    ("cx_root.clas.json", None),
                    ("cx_root.clas.abap", Some("main")),
                    ("cx_root.clas.locals.abap", Some("localtypes")),
                ][..],
            ),
            (
                fixture_snapshots::<Program>(
                    "Z_TEST",
                    "/sap/bc/adt/programs/programs/z_test",
                    PROGRAM_XML,
                ),
                &PROGRAM_FORMAT,
                &[
                    ("z_test.prog.json", None),
                    ("z_test.prog.abap", Some("main")),
                ][..],
            ),
            (
                fixture_snapshots::<Include>(
                    "ZTEST",
                    "/sap/bc/adt/programs/includes/ztest",
                    INCLUDE_XML,
                ),
                &PROGRAM_FORMAT,
                &[("ztest.prog.json", None), ("ztest.prog.abap", Some("main"))][..],
            ),
            (
                fixture_snapshots::<DataElement>(
                    "ZTFRWTFRT",
                    "/sap/bc/adt/ddic/dataelements/ztfrwtfrt",
                    DATA_ELEMENT_XML,
                ),
                &DATA_ELEMENT_FORMAT,
                &[("ztfrwtfrt.dtel.json", None)][..],
            ),
        ] {
            let [typed, erased] = snapshots.map(|snapshot| project(snapshot).unwrap());
            assert_eq!(typed.subject().reference(), erased.subject().reference());
            assert_eq!(typed.subject().etag(), erased.subject().etag());
            assert_eq!(
                typed.subject().properties().unwrap(),
                erased.subject().properties().unwrap()
            );
            for projection in [&typed, &erased] {
                assert!(std::ptr::eq(projection.format(), format));
                assert_eq!(
                    projection
                        .files()
                        .iter()
                        .map(FileProjection::name)
                        .collect::<Vec<_>>(),
                    expected.iter().map(|(name, _)| *name).collect::<Vec<_>>()
                );
                assert!(projection.file("not-an-aff-file").is_none());
                for (file, &(_, expected_component)) in projection.files().iter().zip(expected) {
                    let specification = file.specification();
                    assert!(specification.is_supported());
                    assert!(
                        format
                            .files()
                            .iter()
                            .any(|registered| std::ptr::eq(registered, specification))
                    );
                    assert_eq!(
                        specification
                            .filename(projection.subject().reference().name(), None, None)
                            .unwrap(),
                        file.name()
                    );
                    assert!(std::ptr::eq(file, projection.file(file.name()).unwrap()));
                    match file.backing() {
                        FileBacking::Properties(properties) => {
                            assert_eq!(expected_component, None);
                            assert!(std::ptr::eq(properties.subject(), projection.subject()));
                            let rendered = properties.render().unwrap();
                            assert_eq!(properties.merge(&rendered).unwrap(), None);
                            if format == &PROGRAM_FORMAT {
                                let document: ProjectedProgramProperties =
                                    serde_json::from_str(&rendered).unwrap();
                                assert_eq!(
                                    document.general_information.unwrap().program_type,
                                    if projection.subject().reference().workbench_type()
                                        == &Include::WORKBENCH_TYPE
                                    {
                                        ProgramType::Include
                                    } else {
                                        ProgramType::ExecutableProgram
                                    }
                                );
                            }
                        }
                        FileBacking::Source(source) => {
                            assert_eq!(&source.object, projection.subject().reference());
                            let component = expected_component.expect("expected a source file");
                            let expected_source = if component == "main" {
                                projection.subject().source().unwrap()
                            } else {
                                projection
                                    .subject()
                                    .source_component(component)
                                    .unwrap()
                                    .unwrap()
                            };
                            assert_eq!(source, &expected_source);
                        }
                    }
                }
            }
            for (typed_file, erased_file) in typed.files().iter().zip(erased.files()) {
                match (typed_file.backing(), erased_file.backing()) {
                    (FileBacking::Source(typed), FileBacking::Source(erased)) => {
                        assert_eq!(typed, erased)
                    }
                    (FileBacking::Properties(typed), FileBacking::Properties(erased)) => {
                        let rendered = typed.render().unwrap();
                        assert_eq!(rendered, erased.render().unwrap());
                        assert_eq!(typed.merge(&rendered).unwrap(), None);
                        assert_eq!(erased.merge(&rendered).unwrap(), None);
                    }
                    _ => panic!("typed and erased inputs must produce the same backing"),
                }
            }
        }
    }

    #[test]
    fn missing_main_source_is_omitted_consistently() {
        let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
        properties
            .sources
            .retain(|source| source.include_type != "main");
        let xml = properties.to_xml().unwrap();
        for snapshot in fixture_snapshots::<Class>("CL_ADT_URI_MAPPER", CLASS_URI, &xml) {
            assert!(matches!(
                snapshot.source(),
                Err(zadt::ObjectError::MissingRelation { relation: "source" })
            ));
            let projection = project(snapshot).unwrap();
            assert_eq!(
                projection
                    .files()
                    .iter()
                    .map(FileProjection::name)
                    .collect::<Vec<_>>(),
                [
                    "cl_adt_uri_mapper.clas.json",
                    "cl_adt_uri_mapper.clas.definitions.abap",
                    "cl_adt_uri_mapper.clas.implementations.abap",
                    "cl_adt_uri_mapper.clas.macros.abap",
                    "cl_adt_uri_mapper.clas.testclasses.abap",
                ]
            );
            let name = "cl_adt_uri_mapper.clas.abap";
            assert!(projection.file(name).is_none());
            let specification = CLASS_FORMAT
                .files()
                .iter()
                .find(|file| file.template() == "<name>.clas.abap")
                .unwrap();
            assert!(specification.is_supported());
            assert_eq!(specification.cardinality(), Cardinality::One);
        }
    }

    #[test]
    fn invalid_advertised_source_uri_is_an_error_not_an_omitted_file() {
        for component in ["main", "definitions"] {
            for href in ["", "https://example.invalid/source/main", "source\\main"] {
                let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
                properties
                    .sources
                    .iter_mut()
                    .find(|source| source.include_type == component)
                    .unwrap()
                    .source_uri = href.to_owned();
                for snapshot in fixture_snapshots::<Class>(
                    "CL_ADT_URI_MAPPER",
                    CLASS_URI,
                    &properties.to_xml().unwrap(),
                ) {
                    assert!(
                        matches!(project(snapshot), Err(ProjectionError::InvalidObjectReference(zadt::ObjectError::InvalidLink { href: actual, .. })) if actual == href)
                    );
                }
            }
        }
    }

    #[test]
    fn object_names_cannot_inject_template_placeholders() {
        for format in [&CLASS_FORMAT, &PROGRAM_FORMAT, &DATA_ELEMENT_FORMAT] {
            for specification in format.files() {
                let language = specification.template().contains("<lang>").then_some("en");
                for name in [
                    "<name>",
                    "Z<name>",
                    "<lang>",
                    "Z<lang>",
                    "/ACME/Z<name>",
                    "/ACME/Z<lang>",
                ] {
                    assert!(matches!(
                        specification.filename(name, None, language),
                        Err(ProjectionError::InvalidObjectName { .. })
                    ));
                }
            }
        }
    }

    #[test]
    fn queryless_sources_do_not_inherit_the_properties_version() {
        let reference =
            test_support::reference::<Program>("Z_TEST", "/sap/bc/adt/programs/programs/z_test");
        for version in [WorkbenchVersion::Active, WorkbenchVersion::Inactive] {
            let xml = String::from_utf8_lossy(PROGRAM_XML).replace(
                "adtcore:version=\"inactive\"",
                &format!("adtcore:version=\"{}\"", version.as_str()),
            );
            let snapshot = test_support::erased_properties(
                &reference,
                Program::MEDIA_TYPES[0],
                "properties-etag",
                xml.as_bytes(),
            );
            let projection = project(snapshot).unwrap();
            assert_eq!(projection.subject().workbench_version(), version);
            let FileBacking::Source(source) =
                projection.file("z_test.prog.abap").unwrap().backing()
            else {
                panic!("ABAP file must be source-backed");
            };
            // Current ZADT behavior: no source-version selector is synthesized.
            assert!(source.query().encode(&()).unwrap().query().is_empty());
        }
    }

    #[test]
    fn projected_source_location_and_validator_are_distinct_from_properties() {
        let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
        let main = properties
            .sources
            .iter_mut()
            .find(|source| source.include_type == "main")
            .unwrap();
        main.source_uri = "source/main?version=inactive&note=a+b#section".to_owned();
        let mut link = main
            .links
            .iter()
            .find(|link| link.media_type.as_deref() == Some("text/plain"))
            .unwrap()
            .clone();
        link.href = format!("{CLASS_URI}/source/main?version=inactive&note=a%20b#section");
        link.etag = Some("source-etag".to_owned());
        main.links = vec![link];
        for snapshot in fixture_snapshots::<Class>(
            "CL_ADT_URI_MAPPER",
            CLASS_URI,
            &properties.to_xml().unwrap(),
        ) {
            let projection = project(snapshot).unwrap();
            let FileBacking::Source(source) = projection
                .file("cl_adt_uri_mapper.clas.abap")
                .unwrap()
                .backing()
            else {
                panic!("source file must have a source backing");
            };
            assert_eq!(source.uri.as_str(), format!("{CLASS_URI}/source/main"));
            assert_eq!(source.object.uri().as_str(), CLASS_URI);
            assert_eq!(
                source.query,
                [
                    ("version".to_owned(), "inactive".to_owned()),
                    ("note".to_owned(), "a b".to_owned())
                ]
            );
            assert_eq!(source.fragment.as_deref(), Some("section"));
            assert_eq!(source.etag.as_deref(), Some("source-etag"));
            let request = source.query().encode(&()).unwrap();
            assert_eq!(request.target(), &source.uri);
            assert_eq!(request.query(), source.query);
            let FileBacking::Properties(metadata) = projection
                .file("cl_adt_uri_mapper.clas.json")
                .unwrap()
                .backing()
            else {
                panic!("metadata must have a properties backing");
            };
            assert!(std::ptr::eq(metadata.subject(), projection.subject()));
            assert_eq!(metadata.subject().reference().uri().as_str(), CLASS_URI);
            assert_eq!(
                metadata.subject().etag().map(EntityTag::as_str),
                Some("object-etag")
            );
            assert_ne!(
                source.etag.as_deref(),
                metadata.subject().etag().map(EntityTag::as_str)
            );
        }
    }
}
