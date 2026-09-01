use std::fmt;

use serde::{Serialize, de::DeserializeOwned};
use zadt::{
    Class, DataElement, GlobalWorkbenchType, Include, MediaTyped, ObjectRef, ObjectSnapshot,
    Package, Program, SourceRef,
};

use crate::{ProjectionError, registry};

/// Stable identity of one supported ABAP File Formats family and version.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ObjectFormat {
    object_type: &'static str,
    version: &'static str,
}

impl ObjectFormat {
    pub(crate) const fn new(object_type: &'static str, version: &'static str) -> Self {
        Self {
            object_type,
            version,
        }
    }

    /// Returns the R3TR object type used in AFF file names.
    pub const fn object_type(self) -> &'static str {
        self.object_type
    }

    /// Returns the AFF version implemented for this family.
    pub const fn version(self) -> &'static str {
        self.version
    }

    /// Returns every possible file in this AFF family.
    pub fn files(self) -> &'static [FileSpec] {
        registry::by_format(self).files()
    }

    /// Infers an exact ADT Workbench type from AFF metadata.
    pub fn repository_type_from_metadata(
        self,
        metadata: &[u8],
    ) -> Result<GlobalWorkbenchType, ProjectionError> {
        registry::by_format(self).repository_type_from_metadata(metadata)
    }

    /// Resolves an ADT Workbench type into a registered AFF family.
    pub fn for_workbench_type(object_type: &GlobalWorkbenchType) -> Result<Self, ProjectionError> {
        registry::for_workbench_type(object_type).map(FormatDescriptor::format)
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

/// Stable semantic identity of one file within an AFF family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ComponentId(&'static str);

impl ComponentId {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// The number of files permitted for one file specification.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cardinality {
    One,
    ZeroOrOne,
    ZeroOrMore,
}

/// One possible file in an AFF object representation.
#[derive(Clone, Copy)]
pub struct FileSpec {
    pub template: &'static str,
    pub cardinality: Cardinality,
    pub(crate) descriptor: &'static dyn FileDescriptor,
}

impl FileSpec {
    pub(crate) const fn new(
        template: &'static str,
        cardinality: Cardinality,
        descriptor: &'static dyn FileDescriptor,
    ) -> Self {
        Self {
            template,
            cardinality,
            descriptor,
        }
    }

    /// Returns the semantic component represented by this file.
    pub fn component(self) -> ComponentId {
        self.descriptor.component()
    }

    /// Renders this specification for one ABAP object and optional language.
    pub fn file_name(
        self,
        object_name: &str,
        language: Option<&str>,
    ) -> Result<String, ProjectionError> {
        let object_name = crate::encode_object_name(object_name)?;
        let mut file_name = self.template.replacen("<name>", &object_name, 1);

        if file_name.contains("<lang>") {
            let language = language.ok_or(ProjectionError::MissingLanguage {
                template: self.template,
            })?;
            crate::validate_language(language)?;
            file_name = file_name.replacen("<lang>", language, 1);
        } else if let Some(language) = language {
            return Err(ProjectionError::UnexpectedLanguage {
                template: self.template,
                language: language.to_owned(),
            });
        }

        Ok(file_name)
    }

    pub(crate) fn bind(
        self,
        object: &dyn ProjectionObject,
        language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError> {
        self.descriptor.bind(object, language)
    }
}

impl fmt::Debug for FileSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileSpec")
            .field("template", &self.template)
            .field("cardinality", &self.cardinality)
            .field("component", &self.component())
            .finish()
    }
}

/// The concrete ADT resource used to materialize one projected AFF file.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum FileBacking {
    Source(SourceRef),
    Properties(ObjectRef<()>),
}

/// One materializable AFF file bound to its originating repository object.
#[derive(Clone, Debug)]
pub struct ProjectedFile {
    pub(crate) name: String,
    pub(crate) format: ObjectFormat,
    pub(crate) cardinality: Cardinality,
    pub(crate) component: ComponentId,
    pub(crate) backing: FileBacking,
    pub(crate) descriptor: &'static dyn FileDescriptor,
}

impl ProjectedFile {
    /// Returns the projected AFF file name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the AFF family that owns this file.
    pub const fn format(&self) -> ObjectFormat {
        self.format
    }

    /// Returns this file's cardinality within its object projection.
    pub const fn cardinality(&self) -> Cardinality {
        self.cardinality
    }

    /// Returns this file's stable semantic component.
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// Returns the concrete ADT resource used to materialize this file.
    pub fn backing(&self) -> &FileBacking {
        &self.backing
    }

    /// Renders runtime ADT properties through this file's AFF codec.
    pub fn render_properties(
        &self,
        properties: &ObjectSnapshot<()>,
    ) -> Result<String, ProjectionError> {
        let FileBacking::Properties(object) = &self.backing else {
            return Err(ProjectionError::NotPropertiesFile {
                component: self.component,
            });
        };
        ensure_properties_owner(object, properties)?;
        self.descriptor
            .properties_codec()
            .ok_or(ProjectionError::NotPropertiesFile {
                component: self.component,
            })?
            .render(properties)
    }

    /// Merges edited AFF content into the original runtime ADT properties.
    pub fn merge_properties(
        &self,
        original: &ObjectSnapshot<()>,
        edited: &str,
    ) -> Result<serde_json::Value, ProjectionError> {
        let FileBacking::Properties(object) = &self.backing else {
            return Err(ProjectionError::NotPropertiesFile {
                component: self.component,
            });
        };
        ensure_properties_owner(object, original)?;
        self.descriptor
            .properties_codec()
            .ok_or(ProjectionError::NotPropertiesFile {
                component: self.component,
            })?
            .merge(original, edited)
    }
}

fn ensure_properties_owner(
    expected: &ObjectRef<()>,
    properties: &ObjectSnapshot<()>,
) -> Result<(), ProjectionError> {
    if properties.reference().uri() != expected.uri()
        || properties.reference().name() != expected.name()
        || properties.reference().object_type() != expected.object_type()
    {
        return Err(ProjectionError::PropertiesBindingMismatch {
            expected: expected.uri().to_string(),
            actual: properties.reference().uri().to_string(),
        });
    }
    Ok(())
}

/// A repository object's currently materializable editor-facing AFF files.
#[derive(Clone, Debug)]
pub struct Projection {
    pub format: ObjectFormat,
    pub files: Vec<ProjectedFile>,
}

pub(crate) trait FormatDescriptor: fmt::Debug + Sync {
    fn format(&self) -> ObjectFormat;
    fn repository_types(&self) -> &'static [GlobalWorkbenchType];
    fn files(&self) -> &'static [FileSpec];
    fn repository_type_from_metadata(
        &self,
        metadata: &[u8],
    ) -> Result<GlobalWorkbenchType, ProjectionError>;
}

pub(crate) trait FileDescriptor: fmt::Debug + Sync {
    fn component(&self) -> ComponentId;
    fn bind(
        &self,
        object: &dyn ProjectionObject,
        language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError>;

    fn is_source(&self) -> bool {
        false
    }

    fn properties_codec(&self) -> Option<&dyn PropertiesCodec> {
        None
    }
}

/// Adapter used to project typed and runtime-loaded ADT objects.
pub trait ProjectionObject {
    fn reference(&self) -> ObjectRef<()>;
    fn object_type(&self) -> &GlobalWorkbenchType;
    fn source(&self) -> Result<Option<SourceRef>, zadt::ObjectError>;
    fn source_component(&self, name: &str) -> Result<Option<SourceRef>, zadt::ObjectError>;
}

macro_rules! impl_source_projection_object {
    ($($object:ty),+ $(,)?) => {
        $(
            impl ProjectionObject for $object {
                fn reference(&self) -> ObjectRef<()> {
                    self.reference().erase()
                }

                fn object_type(&self) -> &GlobalWorkbenchType {
                    self.reference().object_type()
                }

                fn source(&self) -> Result<Option<SourceRef>, zadt::ObjectError> {
                    <$object>::source(self).map(Some)
                }

                fn source_component(
                    &self,
                    _name: &str,
                ) -> Result<Option<SourceRef>, zadt::ObjectError> {
                    Ok(None)
                }
            }
        )+
    };
}

macro_rules! impl_sourceless_projection_object {
    ($($object:ty),+ $(,)?) => {
        $(
            impl ProjectionObject for $object {
                fn reference(&self) -> ObjectRef<()> {
                    self.reference().erase()
                }

                fn object_type(&self) -> &GlobalWorkbenchType {
                    self.reference().object_type()
                }

                fn source(&self) -> Result<Option<SourceRef>, zadt::ObjectError> {
                    Ok(None)
                }

                fn source_component(
                    &self,
                    _name: &str,
                ) -> Result<Option<SourceRef>, zadt::ObjectError> {
                    Ok(None)
                }
            }
        )+
    };
}

impl ProjectionObject for ObjectSnapshot<()> {
    fn reference(&self) -> ObjectRef<()> {
        self.reference().clone()
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        self.reference().object_type()
    }

    fn source(&self) -> Result<Option<SourceRef>, zadt::ObjectError> {
        match ObjectSnapshot::<()>::source(self) {
            Ok(source) => Ok(Some(source)),
            Err(
                zadt::ObjectError::MissingRelation { relation: "source" }
                | zadt::ObjectError::UnsupportedCapability {
                    capability: "source",
                    ..
                },
            ) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn source_component(&self, name: &str) -> Result<Option<SourceRef>, zadt::ObjectError> {
        match ObjectSnapshot::<()>::source_component(self, name) {
            Err(zadt::ObjectError::UnsupportedCapability {
                capability: "source components",
                ..
            }) => Ok(None),
            result => result,
        }
    }
}

impl ProjectionObject for ObjectSnapshot<Class> {
    fn reference(&self) -> ObjectRef<()> {
        self.reference().erase()
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        self.reference().object_type()
    }

    fn source(&self) -> Result<Option<SourceRef>, zadt::ObjectError> {
        ObjectSnapshot::<Class>::source(self).map(Some)
    }

    fn source_component(&self, name: &str) -> Result<Option<SourceRef>, zadt::ObjectError> {
        ObjectSnapshot::<Class>::source_component(self, name)
    }
}

impl_source_projection_object!(ObjectSnapshot<Include>, ObjectSnapshot<Program>);
impl_sourceless_projection_object!(ObjectSnapshot<DataElement>, ObjectSnapshot<Package>);

/// Projects one ZADT property type into an AFF document.
pub(crate) trait PropertyProjection<P>: Sized {
    fn project(properties: &P) -> Result<Self, ProjectionError>;
}

pub(crate) trait PropertiesCodec: fmt::Debug + Sync {
    fn render(&self, properties: &ObjectSnapshot<()>) -> Result<String, ProjectionError>;
    fn merge(
        &self,
        original: &ObjectSnapshot<()>,
        edited: &str,
    ) -> Result<serde_json::Value, ProjectionError>;
}

#[derive(Debug)]
pub(crate) struct SourceFileDescriptor {
    component: ComponentId,
    source_component: Option<&'static str>,
}

impl SourceFileDescriptor {
    pub(crate) const fn main(component: ComponentId) -> Self {
        Self {
            component,
            source_component: None,
        }
    }

    pub(crate) const fn named(component: ComponentId, source_component: &'static str) -> Self {
        Self {
            component,
            source_component: Some(source_component),
        }
    }
}

impl FileDescriptor for SourceFileDescriptor {
    fn component(&self) -> ComponentId {
        self.component
    }

    fn bind(
        &self,
        object: &dyn ProjectionObject,
        _language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError> {
        let source = match self.source_component {
            Some(component) => object.source_component(component),
            None => object.source(),
        }?;
        Ok(source.map(FileBacking::Source))
    }

    fn is_source(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub(crate) struct UnbackedFileDescriptor {
    component: ComponentId,
}

impl UnbackedFileDescriptor {
    pub(crate) const fn new(component: ComponentId) -> Self {
        Self { component }
    }
}

impl FileDescriptor for UnbackedFileDescriptor {
    fn component(&self) -> ComponentId {
        self.component
    }

    fn bind(
        &self,
        _object: &dyn ProjectionObject,
        _language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError> {
        Ok(None)
    }
}

pub(crate) fn decode_properties<P>(
    properties: &ObjectSnapshot<()>,
    object_type: &'static str,
) -> Result<P, ProjectionError>
where
    P: MediaTyped + DeserializeOwned,
{
    if !P::MEDIA_TYPES.contains(&properties.media_type()) {
        return Err(ProjectionError::UnsupportedPropertiesMediaType {
            object_type,
            media_type: properties.media_type().to_owned(),
        });
    }
    let payload: P = serde_json::from_value(properties.properties()?).map_err(|source| {
        ProjectionError::InvalidAdtProperties {
            object_type,
            source,
        }
    })?;
    Ok(payload)
}

pub(crate) fn encode_properties<P>(
    payload: P,
    object_type: &'static str,
) -> Result<serde_json::Value, ProjectionError>
where
    P: Serialize,
{
    serde_json::to_value(payload).map_err(|source| ProjectionError::InvalidAdtProperties {
        object_type,
        source,
    })
}
