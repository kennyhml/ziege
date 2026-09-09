use std::{fmt, sync::Arc};

use zadt::{GlobalWorkbenchType, ObjectError, ObjectSnapshot};

use crate::{
    FileBacking, FileProjection, ProjectionError, PropertiesProjection,
    filename::{FilenameTemplate, NameSource},
    validate,
};

pub mod clas;
pub mod ddlx;
pub mod dtel;
pub mod fugr;
pub mod intf;
pub mod prog;

/// A registered AFF format, including its supported objects and file mappings.
///
/// This is the core descriptor of the crate that routes [`FileSpec`] bindings.
/// The object type specific logic happens in the respective file specifications
/// as they, among other things, provide the functionality to project the object
/// properties or bind to owned resource references.
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
    /// Returns the AFF object type, including subobject types such as FUNC and REPS.
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

impl fmt::Debug for ObjectFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectFormat")
            .field("object_type", &self.object_type)
            .field("version", &self.version)
            .finish()
    }
}

/// The number of files permitted by the AFF format.
///
/// This is currently only descriptive metadata and has no implications
/// on validation or other kinds of processing logic.
///
/// TODO: Use this data to validate the projection
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cardinality {
    One,
    ZeroOrOne,
    ZeroOrMore,
}

/// One possible file and its mapping, declared by an AFF family.
#[derive(Clone, Copy, Debug)]
pub struct FileSpec {
    filename: FilenameTemplate,
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
            filename: FilenameTemplate::new(template),
            cardinality,
            mapping,
        }
    }

    /// Declares which object supplies each name placeholder, without changing the backing.
    pub(crate) const fn with_names(mut self, names: &'static [(&'static str, NameSource)]) -> Self {
        self.filename = self.filename.with_names(names);
        self
    }

    pub const fn template(&self) -> &'static str {
        self.filename.template()
    }

    pub const fn cardinality(&self) -> Cardinality {
        self.cardinality
    }

    /// Whether ZAFF implements this mapping. Source availability still depends on the snapshot.
    pub const fn is_supported(&self) -> bool {
        !matches!(self.mapping, Mapping::Unavailable)
    }

    /// Renders the filename of the file specification.
    ///
    /// There are generally three components to a filename, the object name which
    /// is always required, an optional parent for objects that belong to some
    /// overarching container (such as function modules) and a language usually
    /// associated with some object containing localized labels.
    ///
    /// For each section of the template path, such as `<name>` or `<fname>`, the
    /// file spec can provide a [`NameSource`] that disambiguates what value belongs
    /// into which section.
    pub(crate) fn filename(
        &self,
        object_name: &str,
        parent: Option<&str>,
        language: Option<&str>,
    ) -> Result<String, ProjectionError> {
        // Converting the objects to ADT objects also validates the names.
        let object = validate::validate_object_name(object_name)?;
        let parent = if self.filename.requires_parent() {
            parent.map(validate::validate_object_name).transpose()?
        } else {
            None
        };
        self.filename.substitute(object, parent, language)
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

        // A handful of objects have concrete parent objects reflected in the filename.
        // The filename template having a section backed by a [`NameSource::Parent`]
        // means the snapshot must have some associated parent - otherwise rendering
        // will fail later on during substitution.
        let parent = if self.filename.requires_parent() {
            let Some(name) = snapshot.parent_name()? else {
                return Err(ObjectError::ParentObjectRequired {
                    workbench_type: snapshot.key().workbench_type().clone(),
                }
                .into());
            };
            Some(name)
        } else {
            None
        };

        Ok(Some(FileProjection {
            name: self.filename(snapshot.key().name(), parent, None)?,
            specification: self,
            backing,
        }))
    }
}

/// Defines how a [`FileSpec`] maps its contents. During binding,
/// this mapping is turned into a [`FileBacking`] using, for example,
/// the [`SourceRef`] resolved from the source component or the
/// [`PropertiesMapping`] for its associated `render` and `merge` functions.
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

    #[test]
    fn file_specs_validate_raw_names_before_substitution() {
        let child = FileSpec::new("<name>.<fmname>", Cardinality::One, Mapping::Unavailable)
            .with_names(&[("name", NameSource::Parent), ("fmname", NameSource::Object)]);
        for name in ["", "/ACME/", "../GROUP", "<name>"] {
            assert!(matches!(
                child.filename("Z_MODULE", Some(name), None),
                Err(ProjectionError::InvalidObjectName { object_name }) if object_name == name
            ));
            assert!(matches!(
                child.filename(name, Some("Z_GROUP"), None),
                Err(ProjectionError::InvalidObjectName { object_name }) if object_name == name
            ));
        }
        let plain = FileSpec::new("<name>.abap", Cardinality::One, Mapping::Unavailable);
        assert_eq!(
            plain.filename("Z_OBJECT", Some("<unused>"), None).unwrap(),
            "z_object.abap"
        );
    }

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
        assert_ne!(alternate, another_version);
        assert_ne!(alternate, PROGRAM_FORMAT);
    }
}
