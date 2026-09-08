use std::{fmt, hash::Hash, sync::Arc};

use zadt::{GlobalWorkbenchType, ObjectSnapshot, SourceRef};

use crate::ProjectionError;

pub(crate) mod clas;
pub(crate) mod dtel;
pub(crate) mod prog;

pub use clas::{
    ClassCategory, ClassDescriptions, ClassHeader, EventDescription, MethodDescription,
    NameDescription, ProjectedClassProperties,
};
pub use dtel::{
    BasicDirection, BidirectionalOptions, DataElementAdditionalProperties, DataElementCategory,
    DataElementFieldLabels, DataElementHeader, DataElementTypeInformation, PredefinedType,
    ProjectedDataElementProperties, SearchHelp,
};
pub use prog::{
    LogicalDatabase, ProgramGeneralInformation, ProgramHeader, ProgramStatus, ProgramType,
    ProjectedProgramProperties,
};

/// A registered AFF format, including its supported objects and file mappings.
///
/// Equality and hashing use only the AFF object type and version, not the
/// definition's address, supported Workbench types, or file mappings.
pub struct ObjectFormat {
    /// The object type (not the workbench type) of an object - e.g `CLAS`
    pub(crate) object_type: &'static str,

    /// The version of the format
    pub(crate) version: &'static str,

    /// The workbench types this format is used for. Usually its just one, but
    /// includes (PROG/I) and programs (PROG/P) share the same format.
    pub(crate) workbench_types: &'static [GlobalWorkbenchType],

    /// A set of static file specifications for the object that provide binding.
    ///
    /// See [ABAP File Formats][abap-file-formats] for the file specifications
    ///
    /// [abap-file-formats]: https://github.com/SAP/abap-file-formats
    pub(crate) files: &'static [FileSpec],
}

impl ObjectFormat {
    /// Returns the R3TR object type used in AFF file names.
    pub const fn object_type(&self) -> &'static str {
        self.object_type
    }

    /// Returns the AFF version implemented for this family.
    pub const fn version(&self) -> &'static str {
        self.version
    }

    /// Returns all possible files, including recognized but unsupported mappings.
    pub const fn files(&self) -> &'static [FileSpec] {
        self.files
    }

    /// Returns the ADT Workbench types represented by this AFF format.
    pub const fn workbench_types(&self) -> &'static [GlobalWorkbenchType] {
        self.workbench_types
    }
}

impl PartialEq for ObjectFormat {
    fn eq(&self, other: &Self) -> bool {
        self.object_type == other.object_type && self.version == other.version
    }
}

impl Eq for ObjectFormat {}

impl Hash for ObjectFormat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.object_type.hash(state);
        self.version.hash(state);
    }
}

impl fmt::Debug for ObjectFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectFormat")
            .field("object_type", &self.object_type)
            .field("version", &self.version)
            .finish()
    }
}

/// The number of files permitted by the AFF format, not a backend availability guarantee.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cardinality {
    One,
    ZeroOrOne,
    ZeroOrMore,
}

/// One possible file and its mapping, declared by an AFF family.
#[derive(Clone, Copy, Debug)]
pub struct FileSpec {
    template: &'static str,
    cardinality: Cardinality,
    mapping: Mapping,
}

impl FileSpec {
    pub(crate) const fn new(
        template: &'static str,
        cardinality: Cardinality,
        mapping: Mapping,
    ) -> Self {
        Self {
            template,
            cardinality,
            mapping,
        }
    }

    pub const fn template(&self) -> &'static str {
        self.template
    }

    pub const fn cardinality(&self) -> Cardinality {
        self.cardinality
    }

    /// Whether ZAFF implements this mapping. Source availability still depends on the snapshot.
    pub const fn is_supported(&self) -> bool {
        !matches!(self.mapping, Mapping::Unavailable)
    }

    /// wenders the name of specification for one ABAP object and optional language.
    ///
    /// For example, the file specification `<name>.clas.definitions.abap` projects
    /// renders `name` into its specification to produce `zmyclass.clas.definitions.abap`.
    ///
    /// Some objects, like program texts, have an associated language in their name.
    pub(crate) fn filename(
        &self,
        object_name: &str,
        language: Option<&str>,
    ) -> Result<String, ProjectionError> {
        let object_name = crate::encode_object_name(object_name)?;

        // If we are given a language, we expect the object to have a
        // placeholder for it and vice versa.
        let template = if self.template.contains("<lang>") {
            let language = language.ok_or(ProjectionError::MissingLanguage {
                template: self.template,
            })?;
            crate::validate::validate_language(language)?;
            self.template.replacen("<lang>", language, 1)
        } else {
            if let Some(language) = language {
                return Err(ProjectionError::UnexpectedLanguage {
                    template: self.template,
                    language: language.to_owned(),
                });
            }
            self.template.to_owned()
        };

        // The name should always exist.
        Ok(template.replacen("<name>", &object_name, 1))
    }

    /// Binds the file to resources on the given snapshot based on the
    /// [`Mapping`] defined for the file.
    ///
    /// For example, if a [`Mapping::Properties`] mapping is defined, it is
    /// stored as a [`FileBacking::Properties`] internally, holding a reference
    /// to the snapshot and the associated functions to merge and render the
    /// resources properties.
    ///
    /// In the case of a source mapping, the [`SourceRef`] of the referenced
    /// source component serves as the file backing. This way, orchestrators
    /// of the propjection can use the file backings in a meaningful way to
    /// apply updates.
    pub(crate) fn bind(
        &'static self,
        snapshot: &Arc<ObjectSnapshot<()>>,
    ) -> Result<Option<FileProjection>, ProjectionError> {
        let backing = match &self.mapping {
            Mapping::Properties(mapping) => FileBacking::Properties(PropertiesProjection {
                snapshot: Arc::clone(snapshot),
                mapping,
            }),
            Mapping::Source { component } => {
                let source = match component {
                    Some(component) => snapshot.source_component(component)?,
                    None => match snapshot.source() {
                        Ok(source) => Some(source),
                        Err(zadt::ObjectError::MissingRelation { relation: "source" }) => None,
                        Err(error) => return Err(error.into()),
                    },
                };
                let Some(source) = source else {
                    return Ok(None);
                };
                FileBacking::Source(source)
            }
            Mapping::Unavailable => return Ok(None),
        };

        Ok(Some(FileProjection {
            name: self.filename(snapshot.reference().name(), None)?,
            specification: self,
            backing,
        }))
    }
}

/// How to read or edit a projected file. Neither variant performs network I/O.
#[derive(Clone, Debug)]
pub enum FileBacking {
    /// Text is fetched through this advertised reference; saves use ZADT's source-update contract.
    Source(SourceRef),
    /// AFF JSON is rendered and merged against the retained properties baseline.
    Properties(PropertiesProjection),
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

/// The available AFF files derived from one immutable, loaded ADT snapshot.
///
/// The snapshot is shared with its properties files. Source text, dirty buffers,
/// cache invalidation, and save orchestration belong to the caller.
#[derive(Clone, Debug)]
pub struct Projection {
    pub(crate) snapshot: Arc<ObjectSnapshot<()>>,
    pub(crate) format: &'static ObjectFormat,
    pub(crate) files: Vec<FileProjection>,
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum Mapping {
    Source { component: Option<&'static str> },
    Properties(PropertiesMapping),
    Unavailable,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PropertiesMapping {
    pub render: fn(&ObjectSnapshot<()>) -> Result<String, ProjectionError>,
    pub merge: fn(&ObjectSnapshot<()>, &str) -> Result<Option<serde_json::Value>, ProjectionError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{clas::CLASS_FORMAT, prog::PROGRAM_FORMAT};
    use std::collections::HashSet;

    #[test]
    fn format_identity_uses_only_object_type_and_version() {
        let alternate = ObjectFormat {
            object_type: "CLAS",
            version: "1",
            workbench_types: PROGRAM_FORMAT.workbench_types(),
            files: PROGRAM_FORMAT.files(),
        };
        assert!(!std::ptr::eq(&alternate, &CLASS_FORMAT));
        assert_eq!(alternate, CLASS_FORMAT);
        assert!(std::ptr::eq(alternate.files(), PROGRAM_FORMAT.files()));

        let another_version = ObjectFormat {
            version: "2",
            ..alternate
        };
        let mut formats = HashSet::from([&CLASS_FORMAT]);
        assert!(!formats.insert(&alternate));
        assert!(formats.insert(&another_version));
        assert!(formats.insert(&PROGRAM_FORMAT));
    }
}
