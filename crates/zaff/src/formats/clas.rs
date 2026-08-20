use serde::{Deserialize, Serialize};
use zadt::{
    AbapLanguageVersion, AdvertisedObjectReference, AnyObject, Class, ClassCategory,
    ClassProperties, GlobalWorkbenchType, ObjectType,
};

use crate::{
    Cardinality, ComponentId, FileBacking, FileSpec, ObjectFormat, ProjectionError,
    format::{
        FileDescriptor, FormatDescriptor, PropertiesCodec, SourceFileDescriptor,
        UnbackedFileDescriptor, decode_properties, encode_properties,
    },
    language,
};

pub const CLASS_FORMAT: ObjectFormat = ObjectFormat::new("CLAS", "1");

#[derive(Debug)]
pub(crate) struct ClassDescriptor;

#[derive(Debug)]
struct ClassMetadata;

static CLASS_FILES: &[FileSpec] = &[
    FileSpec::new("<name>.clas.json", Cardinality::One, &ClassMetadata),
    FileSpec::new(
        "<name>.clas.abap",
        Cardinality::One,
        &SourceFileDescriptor::main(ComponentId::new("source/main")),
    ),
    FileSpec::new(
        "<name>.clas.definitions.abap",
        Cardinality::ZeroOrOne,
        &SourceFileDescriptor::named(ComponentId::new("source/definitions"), "definitions"),
    ),
    FileSpec::new(
        "<name>.clas.implementations.abap",
        Cardinality::ZeroOrOne,
        &SourceFileDescriptor::named(
            ComponentId::new("source/implementations"),
            "implementations",
        ),
    ),
    FileSpec::new(
        "<name>.clas.macros.abap",
        Cardinality::ZeroOrOne,
        &SourceFileDescriptor::named(ComponentId::new("source/macros"), "macros"),
    ),
    FileSpec::new(
        "<name>.clas.testclasses.abap",
        Cardinality::ZeroOrOne,
        &SourceFileDescriptor::named(ComponentId::new("source/testclasses"), "testclasses"),
    ),
    FileSpec::new(
        "<name>.clas.locals.abap",
        Cardinality::ZeroOrOne,
        &SourceFileDescriptor::named(ComponentId::new("source/localtypes"), "localtypes"),
    ),
    FileSpec::new(
        "<name>.clas.texts.<lang>.properties",
        Cardinality::ZeroOrMore,
        &UnbackedFileDescriptor::new(ComponentId::new("text/texts")),
    ),
];

impl FormatDescriptor for ClassDescriptor {
    fn format(&self) -> ObjectFormat {
        CLASS_FORMAT
    }

    fn repository_types(&self) -> &'static [GlobalWorkbenchType] {
        const TYPES: &[GlobalWorkbenchType] = &[Class::WORKBENCH_TYPE];
        TYPES
    }

    fn files(&self) -> &'static [FileSpec] {
        CLASS_FILES
    }

    fn repository_type_from_metadata(
        &self,
        metadata: &[u8],
    ) -> Result<GlobalWorkbenchType, ProjectionError> {
        let metadata: MetadataDiscriminator = serde_json::from_slice(metadata)?;
        if metadata.format_version != CLASS_FORMAT.version() {
            return Err(ProjectionError::UnsupportedFormatVersion {
                object_type: CLASS_FORMAT.object_type(),
                version: metadata.format_version,
            });
        }
        Ok(Class::WORKBENCH_TYPE)
    }
}

impl FileDescriptor for ClassMetadata {
    fn component(&self) -> ComponentId {
        ComponentId::new("metadata")
    }

    fn bind(
        &self,
        object: &dyn crate::format::ProjectionObject,
        _language: Option<&str>,
    ) -> Result<Option<FileBacking>, ProjectionError> {
        if object.object_type() != &Class::WORKBENCH_TYPE {
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

impl PropertiesCodec for ClassMetadata {
    fn render(&self, properties: &AnyObject) -> Result<String, ProjectionError> {
        render_class_properties(&decode_properties::<ClassProperties>(properties, "CLAS")?)
    }

    fn merge(&self, original: &AnyObject, edited: &str) -> Result<AnyObject, ProjectionError> {
        let properties = decode_properties::<ClassProperties>(original, "CLAS")?;
        encode_properties(
            original,
            merge_class_properties(&properties, edited)?,
            "CLAS",
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDiscriminator {
    format_version: String,
}

/// The AFF v1 metadata representation of an ABAP Class.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffClass {
    pub format_version: String,
    pub header: AffClassHeader,
    #[serde(default, skip_serializing_if = "AffClassCategory::is_default")]
    pub category: AffClassCategory,
    #[serde(default, skip_serializing_if = "is_false")]
    pub fix_point_arithmetic: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message_class: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptions: Option<AffClassDescriptions>,
}

/// Common AFF Class header fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffClassHeader {
    pub description: String,
    pub original_language: String,
    #[serde(
        default,
        skip_serializing_if = "AffClassAbapLanguageVersion::is_default"
    )]
    pub abap_language_version: AffClassAbapLanguageVersion,
}

/// AFF's ABAP language-version vocabulary for Classes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffClassAbapLanguageVersion {
    #[default]
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "keyUser")]
    KeyUser,
    #[serde(rename = "cloudDevelopment")]
    CloudDevelopment,
}

impl AffClassAbapLanguageVersion {
    fn from_adt(value: Option<&AbapLanguageVersion>) -> Result<Self, ProjectionError> {
        match value.map(AbapLanguageVersion::as_str) {
            None | Some("" | " " | "X") => Ok(Self::Standard),
            Some("2") => Ok(Self::KeyUser),
            Some("5") => Ok(Self::CloudDevelopment),
            Some(value) => Err(invalid(
                "header.abapLanguageVersion",
                format!("unsupported ADT value `{value}`"),
            )),
        }
    }

    fn adt_value(self, original: Option<&AbapLanguageVersion>) -> Option<AbapLanguageVersion> {
        match self {
            Self::Standard if original.is_none() => None,
            Self::Standard => Some(AbapLanguageVersion::StandardX),
            Self::KeyUser => Some(AbapLanguageVersion::KeyUser),
            Self::CloudDevelopment => Some(AbapLanguageVersion::CloudDevelopment),
        }
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::Standard)
    }
}

/// AFF's semantic Class categories.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AffClassCategory {
    #[default]
    #[serde(rename = "generalObjectType")]
    GeneralObjectType,
    #[serde(rename = "exitClass")]
    ExitClass,
    #[serde(rename = "testclassAbapUnit")]
    TestclassAbapUnit,
    #[serde(rename = "behaviorClass")]
    BehaviorClass,
    #[serde(rename = "entityEventHandler")]
    EntityEventHandler,
    #[serde(rename = "persistentClass")]
    PersistentClass,
    #[serde(rename = "factoryForPersistentClass")]
    FactoryForPersistentClass,
    #[serde(rename = "statusClassForPersistClass")]
    StatusClassForPersistClass,
    #[serde(rename = "rfcProxyClass")]
    RfcProxyClass,
    #[serde(rename = "communicationConnectionClass")]
    CommunicationConnectionClass,
    #[serde(rename = "exceptionClass")]
    ExceptionClass,
    #[serde(rename = "areaClassSharedObjects")]
    AreaClassSharedObjects,
    #[serde(rename = "businessClass")]
    BusinessClass,
    #[serde(rename = "bspApplicationClass")]
    BspApplicationClass,
    #[serde(rename = "basisClassBspElementHdlr")]
    BasisClassBspElementHandler,
    #[serde(rename = "webDynproRuntimeObject")]
    WebDynproRuntimeObject,
}

impl AffClassCategory {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "generalObjectType" => Ok(Self::GeneralObjectType),
            "exitClass" => Ok(Self::ExitClass),
            "testclassAbapUnit" => Ok(Self::TestclassAbapUnit),
            "behaviorClass" => Ok(Self::BehaviorClass),
            "entityEventHandler" => Ok(Self::EntityEventHandler),
            "persistentClass" => Ok(Self::PersistentClass),
            "factoryForPersistentClass" => Ok(Self::FactoryForPersistentClass),
            "statusClassForPersistClass" => Ok(Self::StatusClassForPersistClass),
            "rfcProxyClass" => Ok(Self::RfcProxyClass),
            "communicationConnectionClass" => Ok(Self::CommunicationConnectionClass),
            "exceptionClass" => Ok(Self::ExceptionClass),
            "areaClassSharedObjects" => Ok(Self::AreaClassSharedObjects),
            "businessClass" => Ok(Self::BusinessClass),
            "bspApplicationClass" => Ok(Self::BspApplicationClass),
            "basisClassBspElementHdlr" => Ok(Self::BasisClassBspElementHandler),
            "webDynproRuntimeObject" => Ok(Self::WebDynproRuntimeObject),
            value => Err(invalid(
                "category",
                format!("unsupported ADT Class category `{value}`"),
            )),
        }
    }

    const fn adt_value(self) -> &'static str {
        match self {
            Self::GeneralObjectType => "generalObjectType",
            Self::ExitClass => "exitClass",
            Self::TestclassAbapUnit => "testclassAbapUnit",
            Self::BehaviorClass => "behaviorClass",
            Self::EntityEventHandler => "entityEventHandler",
            Self::PersistentClass => "persistentClass",
            Self::FactoryForPersistentClass => "factoryForPersistentClass",
            Self::StatusClassForPersistClass => "statusClassForPersistClass",
            Self::RfcProxyClass => "rfcProxyClass",
            Self::CommunicationConnectionClass => "communicationConnectionClass",
            Self::ExceptionClass => "exceptionClass",
            Self::AreaClassSharedObjects => "areaClassSharedObjects",
            Self::BusinessClass => "businessClass",
            Self::BspApplicationClass => "bspApplicationClass",
            Self::BasisClassBspElementHandler => "basisClassBspElementHdlr",
            Self::WebDynproRuntimeObject => "webDynproRuntimeObject",
        }
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::GeneralObjectType)
    }
}

/// Optional SE80 descriptions represented by the Class AFF schema.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffClassDescriptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<AffNameDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AffNameDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AffEventDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<AffMethodDescription>,
}

impl AffClassDescriptions {
    fn is_empty(&self) -> bool {
        self.types.is_empty()
            && self.attributes.is_empty()
            && self.events.is_empty()
            && self.methods.is_empty()
    }
}

/// A named Class component description.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffNameDescription {
    pub name: String,
    pub description: String,
}

/// A Class event description and its parameter descriptions.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffEventDescription {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<AffNameDescription>,
}

/// A Class method description and its parameter and exception descriptions.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffMethodDescription {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<AffNameDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exceptions: Vec<AffNameDescription>,
}

pub(crate) fn render_class_properties(
    properties: &ClassProperties,
) -> Result<String, ProjectionError> {
    let document = document_from_properties(properties)?;
    let mut content =
        serde_json::to_string_pretty(&document).map_err(ProjectionError::InvalidClassDocument)?;
    content.push('\n');
    Ok(content)
}

pub(crate) fn merge_class_properties(
    original: &ClassProperties,
    edited: &str,
) -> Result<ClassProperties, ProjectionError> {
    let edited: AffClass =
        serde_json::from_str(edited).map_err(ProjectionError::InvalidClassDocument)?;
    edited.validate()?;
    if edited
        .descriptions
        .as_ref()
        .is_some_and(|descriptions| !descriptions.is_empty())
    {
        return Err(ProjectionError::UnsupportedAffProperty {
            object_type: "CLAS",
            field: "descriptions",
        });
    }

    let original_document = document_from_properties(original)?;
    let original_language_version = original.abap_language_version.clone();
    let mut merged = original.clone();
    let properties = &mut merged;
    if edited.header.description != original_document.header.description {
        properties.description = edited.header.description;
    }
    if edited.header.original_language != original_document.header.original_language {
        properties.master_language =
            language::to_adt(&edited.header.original_language, "header.originalLanguage")?;
    }
    if edited.header.abap_language_version != original_document.header.abap_language_version {
        properties.abap_language_version = edited
            .header
            .abap_language_version
            .adt_value(original_language_version.as_ref());
    }
    if edited.category != original_document.category {
        properties.category = ClassCategory::from(edited.category.adt_value());
    }
    if edited.fix_point_arithmetic != original_document.fix_point_arithmetic {
        properties.fix_point_arithmetic = edited.fix_point_arithmetic;
    }
    if edited.message_class != original_document.message_class {
        properties.message_class =
            (!edited.message_class.is_empty()).then(|| AdvertisedObjectReference {
                name: Some(edited.message_class),
                ..Default::default()
            });
    }
    Ok(merged)
}

fn document_from_properties(properties: &ClassProperties) -> Result<AffClass, ProjectionError> {
    let document = AffClass {
        format_version: CLASS_FORMAT.version().to_owned(),
        header: AffClassHeader {
            description: properties.description.clone(),
            original_language: language::from_adt(
                &properties.master_language,
                "header.originalLanguage",
            )?,
            abap_language_version: AffClassAbapLanguageVersion::from_adt(
                properties.abap_language_version.as_ref(),
            )?,
        },
        category: AffClassCategory::from_adt(properties.category.as_str())?,
        fix_point_arithmetic: properties.fix_point_arithmetic,
        message_class: properties
            .message_class
            .as_ref()
            .and_then(|reference| reference.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_default(),
        descriptions: None,
    };
    document.validate()?;
    Ok(document)
}

impl AffClass {
    fn validate(&self) -> Result<(), ProjectionError> {
        if self.format_version != CLASS_FORMAT.version() {
            return Err(invalid(
                "formatVersion",
                format!("expected `{}`", CLASS_FORMAT.version()),
            ));
        }
        max_length("header.description", &self.header.description, 60)?;
        language::to_adt(&self.header.original_language, "header.originalLanguage")?;
        max_length("messageClass", &self.message_class, 20)?;
        if let Some(descriptions) = &self.descriptions {
            validate_named("descriptions.types", &descriptions.types)?;
            validate_named("descriptions.attributes", &descriptions.attributes)?;
            validate_events("descriptions.events", &descriptions.events)?;
            validate_methods("descriptions.methods", &descriptions.methods)?;
        }
        Ok(())
    }
}

fn validate_named(
    field: &'static str,
    descriptions: &[AffNameDescription],
) -> Result<(), ProjectionError> {
    for (index, description) in descriptions.iter().enumerate() {
        max_length(field, &description.name, 30)?;
        max_length(field, &description.description, 60)?;
        if descriptions[..index].contains(description) {
            return Err(invalid(
                field,
                format!("duplicate description for `{}`", description.name),
            ));
        }
    }
    Ok(())
}

fn validate_events(
    field: &'static str,
    descriptions: &[AffEventDescription],
) -> Result<(), ProjectionError> {
    for (index, description) in descriptions.iter().enumerate() {
        max_length(field, &description.name, 30)?;
        max_length(field, &description.description, 60)?;
        validate_named("descriptions.events.parameters", &description.parameters)?;
        if descriptions[..index].contains(description) {
            return Err(invalid(
                field,
                format!("duplicate description for `{}`", description.name),
            ));
        }
    }
    Ok(())
}

fn validate_methods(
    field: &'static str,
    descriptions: &[AffMethodDescription],
) -> Result<(), ProjectionError> {
    for (index, description) in descriptions.iter().enumerate() {
        max_length(field, &description.name, 30)?;
        max_length(field, &description.description, 60)?;
        validate_named("descriptions.methods.parameters", &description.parameters)?;
        validate_named("descriptions.methods.exceptions", &description.exceptions)?;
        if descriptions[..index].contains(description) {
            return Err(invalid(
                field,
                format!("duplicate description for `{}`", description.name),
            ));
        }
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

fn is_false(value: &bool) -> bool {
    !value
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use zadt::{Class, ClassPropertiesVersion};

    use super::*;

    const CLASS_XML: &[u8] =
        include_bytes!("../../../zadt/tests/fixtures/class-cl-adt-uri-mapper-v4.xml");

    fn class() -> ClassProperties {
        let reference = crate::test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        crate::test_support::properties(
            &reference,
            ClassPropertiesVersion::V4.media_type(),
            "class-etag",
            CLASS_XML,
        )
        .properties
    }

    #[test]
    fn renders_class_properties_as_canonical_aff_v1() {
        let properties = class();
        let content = render_class_properties(&properties).unwrap();
        let document: Value = serde_json::from_str(&content).unwrap();

        assert!(content.ends_with('\n'));
        assert_eq!(document["formatVersion"], "1");
        assert_eq!(document["header"]["description"], "URI Mapper");
        assert_eq!(document["header"]["originalLanguage"], "en");
        assert!(document["header"].get("abapLanguageVersion").is_none());
        assert!(document.get("category").is_none());
        assert_eq!(document["fixPointArithmetic"], true);
        assert!(document.get("messageClass").is_none());
        assert!(document.get("descriptions").is_none());
    }

    #[test]
    fn merges_class_edits_without_losing_adt_only_properties() {
        let original = class();
        let mut edited: AffClass =
            serde_json::from_str(&render_class_properties(&original).unwrap()).unwrap();
        edited.header.description = "Updated class".to_owned();
        edited.header.original_language = "en-GB".to_owned();
        edited.header.abap_language_version = AffClassAbapLanguageVersion::KeyUser;
        edited.category = AffClassCategory::BusinessClass;
        edited.fix_point_arithmetic = false;
        edited.message_class = "Z_MESSAGES".to_owned();

        let merged =
            merge_class_properties(&original, &serde_json::to_string(&edited).unwrap()).unwrap();

        assert_eq!(merged.description, "Updated class");
        assert_eq!(merged.master_language, "6N");
        assert_eq!(
            merged.abap_language_version,
            Some(AbapLanguageVersion::KeyUser)
        );
        assert_eq!(merged.category.as_str(), "businessClass");
        assert!(!merged.fix_point_arithmetic);
        assert_eq!(
            merged
                .message_class
                .as_ref()
                .and_then(|reference| reference.name.as_deref()),
            Some("Z_MESSAGES")
        );
        assert_eq!(merged.package, original.package);
        assert_eq!(merged.links, original.links);
        assert_eq!(merged.sources, original.sources);
        assert_eq!(merged.super_class, original.super_class);
    }

    #[test]
    fn rejects_class_descriptions_until_an_adt_backing_is_available() {
        let original = class();
        let mut edited: AffClass =
            serde_json::from_str(&render_class_properties(&original).unwrap()).unwrap();
        edited.descriptions = Some(AffClassDescriptions {
            methods: vec![AffMethodDescription {
                name: "RUN".to_owned(),
                description: "Runs the class".to_owned(),
                parameters: Vec::new(),
                exceptions: Vec::new(),
            }],
            ..Default::default()
        });

        assert!(matches!(
            merge_class_properties(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "descriptions",
                ..
            })
        ));
    }

    #[test]
    fn validates_class_schema_fields_and_unique_description_names() {
        let original = class();
        let content = render_class_properties(&original).unwrap();
        let unknown = content.replacen('{', "{\n  \"unknown\": true,", 1);
        assert!(matches!(
            merge_class_properties(&original, &unknown),
            Err(ProjectionError::InvalidClassDocument(_))
        ));

        let mut duplicate: AffClass = serde_json::from_str(&content).unwrap();
        duplicate.descriptions = Some(AffClassDescriptions {
            types: vec![
                AffNameDescription {
                    name: "TYPE".to_owned(),
                    description: "First".to_owned(),
                },
                AffNameDescription {
                    name: "TYPE".to_owned(),
                    description: "First".to_owned(),
                },
            ],
            ..Default::default()
        });
        let duplicate = serde_json::to_string(&duplicate).unwrap();
        assert!(matches!(
            merge_class_properties(&original, &duplicate),
            Err(ProjectionError::InvalidAffField {
                field: "descriptions.types",
                ..
            })
        ));

        let mut same_name: AffClass = serde_json::from_str(&content).unwrap();
        same_name.descriptions = Some(AffClassDescriptions {
            types: vec![
                AffNameDescription {
                    name: "TYPE".to_owned(),
                    description: "First".to_owned(),
                },
                AffNameDescription {
                    name: "TYPE".to_owned(),
                    description: "Second".to_owned(),
                },
            ],
            ..Default::default()
        });
        assert!(matches!(
            merge_class_properties(&original, &serde_json::to_string(&same_name).unwrap()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "descriptions",
                ..
            })
        ));

        let mut invalid_language: Value = serde_json::from_str(&content).unwrap();
        invalid_language["header"]["originalLanguage"] = json!("not-supported");
        assert!(matches!(
            merge_class_properties(&original, &invalid_language.to_string()),
            Err(ProjectionError::InvalidAffField {
                field: "header.originalLanguage",
                ..
            })
        ));
    }
}
