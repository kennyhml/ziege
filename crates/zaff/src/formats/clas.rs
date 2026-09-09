//! Class mapping between ADT properties and an AFF `.clas.json` document.
//!
//! General metadata comes from [`ClassProperties`]. AFF groups the description
//! and language settings under `header`, while category, arithmetic, and message
//! class remain at the document root. Component descriptions are part of the
//! AFF model but have no implemented ADT backing here.
//!
//! The mapping tables use AFF JSON paths and ADT Rust field names, not XML
//! attribute names. ABAP source files are bound separately through the advertised
//! main source and named Class components in the format declaration below.
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{
    AdvertisedObjectReference, Class, ClassCategory as AdtClassCategory, ClassProperties,
    ObjectSnapshot, ObjectType,
};

use crate::{
    AbapLanguageVersion, Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::is_false,
    models::{language_from_adt, language_to_adt},
    validate::{one_of, unique_items},
};

pub(crate) static CLASS_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "CLAS",
    version: "1",
    workbench_types: &[Class::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.clas.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.clas.abap",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
        FileSpec::new(
            "<name>.clas.definitions.abap",
            Cardinality::ZeroOrOne,
            Mapping::Source {
                component: Some("definitions"),
            },
        ),
        FileSpec::new(
            "<name>.clas.implementations.abap",
            Cardinality::ZeroOrOne,
            Mapping::Source {
                component: Some("implementations"),
            },
        ),
        FileSpec::new(
            "<name>.clas.macros.abap",
            Cardinality::ZeroOrOne,
            Mapping::Source {
                component: Some("macros"),
            },
        ),
        FileSpec::new(
            "<name>.clas.testclasses.abap",
            Cardinality::ZeroOrOne,
            Mapping::Source {
                component: Some("testclasses"),
            },
        ),
        FileSpec::new(
            "<name>.clas.locals.abap",
            Cardinality::ZeroOrOne,
            Mapping::Source {
                component: Some("localtypes"),
            },
        ),
        FileSpec::new(
            "<name>.clas.texts.<lang>.properties",
            Cardinality::ZeroOrMore,
            Mapping::Unavailable,
        ),
    ],
};

/// Renders ADT [`ClassProperties`] as pretty-printed AFF JSON with a trailing newline.
fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<Class>()?;

    let document = ProjectedClassProperties::from_adt(properties)?;
    let mut content = serde_json::to_string_pretty(&document)?;
    content.push('\n');

    Ok(content)
}

/// Validates edited AFF JSON and applies its changes to a copy of the original
/// [`ClassProperties`], preserving unaffected ADT fields and their wire representations.
/// Returns ADT wire-shaped JSON only when the merged properties differ.
fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<Class>()?;
    let edited: ProjectedClassProperties = serde_json::from_str(edited)?;
    edited.validate()?;

    // AFF header.originalLanguage uses BCP47. ADT master_language uses SAP
    // language codes, for example "en" maps to "EN".
    let language = language_to_adt(&edited.header.original_language, "header.originalLanguage")?;

    // AFF descriptions contains SE80 component descriptions, not ABAP source
    // or ABAP Doc. This mapping has no ADT backing for those entries, so a
    // nonempty block is rejected rather than silently discarded.
    if edited.descriptions.as_ref().is_some_and(|v| !v.is_empty()) {
        return Err(ProjectionError::UnsupportedAffProperty {
            object_type: "CLAS",
            field: "descriptions",
        });
    }

    let mut merged = original.clone();
    let previous = ProjectedClassProperties::from_adt(original)?;

    // Direct field mappings:
    //
    //   AFF header.description       -> ADT description
    //   AFF fixPointArithmetic       -> ADT fix_point_arithmetic
    //   AFF header.originalLanguage  -> ADT master_language (converted above)
    merged.description = edited.header.description;
    merged.fix_point_arithmetic = edited.fix_point_arithmetic;
    merged.master_language = language;

    // AFF header.abapLanguageVersion -> ADT abap_language_version.
    // Several ADT spellings mean Standard in AFF. Keep the original spelling
    // unless the language version changed. A new Standard value uses "X".
    if edited.header.abap_language_version != previous.header.abap_language_version {
        merged.abap_language_version = Some(edited.header.abap_language_version.to_adt_reps());
    }

    // AFF category -> ADT category, using the ClassCategory spelling table.
    // Preserve accepted alternate ADT spellings when the AFF category is unchanged.
    if edited.category != previous.category {
        merged.category = edited.category.adt_value();
    }

    // AFF messageClass -> ADT message_class.name.
    // The ADT reference also carries URI, type, and description metadata that
    // AFF cannot express. Keep the complete reference for an unchanged name.
    // A different name gets a new name-only reference. An empty name removes it.
    if edited.message_class != previous.message_class {
        let empty = edited.message_class.is_empty();
        merged.message_class = (!empty).then(|| AdvertisedObjectReference {
            name: Some(edited.message_class),
            ..Default::default()
        });
    }

    if merged == *original {
        return Ok(None);
    }
    serde_json::to_value(merged).map(Some).map_err(Into::into)
}

/// Class properties represented by the AFF v1 JSON document.
///
/// # Field Mapping
///
/// ADT fields below are on [`ClassProperties`].
///
/// ```text
/// AFF field                   ADT field
/// ---------                   ---------
/// formatVersion               No backing field, this format supplies "1"
/// header.description          description
/// header.originalLanguage     master_language
/// header.abapLanguageVersion  abap_language_version
/// category                    category
/// fixPointArithmetic          fix_point_arithmetic
/// messageClass                message_class.name
/// descriptions                No implemented ADT backing
/// ```
///
/// See [`ClassHeader`] for language conversions and [`ClassCategory`] for the
/// category spellings. `fixPointArithmetic` is copied as a boolean and omitted
/// from AFF JSON when false. It does not change the ADT `unicode_check_active` field.
///
/// # Message Class
///
/// AFF stores only a message-class name. ADT stores an optional
/// [`AdvertisedObjectReference`] in `message_class`, with a name and additional
/// URI, type, and description metadata.
///
/// An absent reference or absent name produces an empty AFF name, which is
/// omitted from the JSON. If the AFF name is unchanged, the complete original
/// reference is preserved. Changing the name creates a new reference with only
/// that name. Clearing the name removes the reference.
///
/// For example, changing only the Class description leaves the message-class
/// URI intact. Changing `messageClass` from `Z_OLD` to `Z_NEW` must not retain a
/// URI that still points to `Z_OLD`.
///
/// # Component Descriptions
///
/// The `descriptions` block has no implemented ADT mapping. It is omitted from
/// generated documents. Missing or empty blocks are accepted, but nonempty
/// entries are rejected. See [`ClassDescriptions`] for the AFF field layout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProjectedClassProperties {
    #[garde(custom(one_of([CLASS_FORMAT.version()])))]
    pub format_version: String,

    #[garde(dive)]
    pub header: ClassHeader,

    #[serde(default, skip_serializing_if = "ClassCategory::is_default")]
    pub category: ClassCategory,

    #[serde(default, skip_serializing_if = "is_false")]
    pub fix_point_arithmetic: bool,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 20))]
    pub message_class: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub descriptions: Option<ClassDescriptions>,
}

/// Common AFF Class header fields.
///
/// # Field Mapping
///
/// ```text
/// AFF field                   ADT ClassProperties field
/// ---------                   -------------------------
/// header.description          description
/// header.originalLanguage     master_language
/// header.abapLanguageVersion  abap_language_version
/// ```
///
/// Description text is copied directly. Original language is converted between
/// SAP codes in ADT and BCP47 tags in AFF, for example `EN` and `en`.
/// Unsupported language codes or tags are rejected.
///
/// # Language Versions
///
/// ```text
/// AFF value         Accepted ADT value on render  ADT value written for an edit
/// ---------         ----------------------------  -----------------------------
/// standard          Absent, "", " ", or "X"       "X"
/// keyUser           "2"                           "2"
/// cloudDevelopment  "5"                           "5"
/// ```
///
/// Standard is omitted from AFF JSON. An unchanged Standard value keeps the
/// original absent, blank, or `"X"` ADT representation. Class uses the REPS
/// Standard encoding `"X"`, unlike the DTEL encoding `"0"`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ClassHeader {
    #[garde(length(chars, max = 60))]
    pub description: String,

    pub original_language: String,

    #[serde(default, skip_serializing_if = "AbapLanguageVersion::is_standard")]
    pub abap_language_version: AbapLanguageVersion,
}

/// Semantic Class categories in AFF.
///
/// # Category Mapping
///
/// ```text
/// AFF category                  ADT category written for an edit
/// ------------                  --------------------------------
/// generalObjectType             generalObjectType
/// exitClass                     exitClass
/// testclassAbapUnit             testClass
/// behaviorClass                 behaviorPool
/// entityEventHandler            entityEventHandler
/// persistentClass               persistentClass
/// factoryForPersistentClass     factoryForPersistentClass
/// statusClassForPersistClass    statusClassForPersistClass
/// rfcProxyClass                 rfcProxyClass
/// communicationConnectionClass  communicationConnectionClass
/// exceptionClass                exceptionClass
/// areaClassSharedObjects        areaClass
/// businessClass                 businessClass
/// bspApplicationClass           bspClass
/// basisClassBspElementHdlr      basisClassBspElementHdlr
/// webDynproRuntimeObject        webDynproRuntimeObject
/// ```
///
/// `generalObjectType` is the default and is omitted from AFF JSON.
/// The existing ADT string mappings also accept the AFF spellings in the left
/// column. An unchanged category keeps the original ADT spelling. For example,
/// ADT `testClass` and `testclassAbapUnit` both appear as AFF `testclassAbapUnit`,
/// but an unrelated edit does not normalize one spelling to the other.
///
/// Categories without a named variant in the ZADT model use its `Other` string
/// variant. This table describes the implemented mapping, not a guarantee that
/// every backend supports every category.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClassCategory {
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

impl ClassCategory {
    fn from_adt(value: &AdtClassCategory) -> Result<Self, ProjectionError> {
        match value {
            AdtClassCategory::GeneralObjectType => Ok(Self::GeneralObjectType),
            AdtClassCategory::ExceptionClass => Ok(Self::ExceptionClass),
            AdtClassCategory::TestClass => Ok(Self::TestclassAbapUnit),
            AdtClassCategory::AreaClass => Ok(Self::AreaClassSharedObjects),
            AdtClassCategory::BspClass => Ok(Self::BspApplicationClass),
            AdtClassCategory::BehaviorPool => Ok(Self::BehaviorClass),
            AdtClassCategory::RfcProxyClass => Ok(Self::RfcProxyClass),
            // Retain the existing string mappings for categories not modeled by zadt.
            AdtClassCategory::Other(value) => match value.as_str() {
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
                value => Err(ProjectionError::InvalidAffField {
                    field: "category",
                    message: format!("unsupported ADT Class category `{value}`"),
                }),
            },
        }
    }

    fn adt_value(self) -> AdtClassCategory {
        match self {
            Self::GeneralObjectType => AdtClassCategory::GeneralObjectType,
            Self::TestclassAbapUnit => AdtClassCategory::TestClass,
            Self::BehaviorClass => AdtClassCategory::BehaviorPool,
            Self::RfcProxyClass => AdtClassCategory::RfcProxyClass,
            Self::ExceptionClass => AdtClassCategory::ExceptionClass,
            Self::AreaClassSharedObjects => AdtClassCategory::AreaClass,
            Self::BspApplicationClass => AdtClassCategory::BspClass,
            Self::ExitClass => AdtClassCategory::Other("exitClass".to_owned()),
            Self::EntityEventHandler => AdtClassCategory::Other("entityEventHandler".to_owned()),
            Self::PersistentClass => AdtClassCategory::Other("persistentClass".to_owned()),
            Self::FactoryForPersistentClass => {
                AdtClassCategory::Other("factoryForPersistentClass".to_owned())
            }
            Self::StatusClassForPersistClass => {
                AdtClassCategory::Other("statusClassForPersistClass".to_owned())
            }
            Self::CommunicationConnectionClass => {
                AdtClassCategory::Other("communicationConnectionClass".to_owned())
            }
            Self::BusinessClass => AdtClassCategory::Other("businessClass".to_owned()),
            Self::BasisClassBspElementHandler => {
                AdtClassCategory::Other("basisClassBspElementHdlr".to_owned())
            }
            Self::WebDynproRuntimeObject => {
                AdtClassCategory::Other("webDynproRuntimeObject".to_owned())
            }
        }
    }

    const fn is_default(&self) -> bool {
        matches!(self, Self::GeneralObjectType)
    }
}

/// Optional SE80 descriptions represented by the Class AFF schema.
///
/// # AFF Fields
///
/// These are component descriptions, not declarations or ABAP Doc comments.
/// All paths below are inside the `descriptions` block.
///
/// ```text
/// AFF field   Contents                           ADT backing
/// ---------   --------                           -----------
/// types       Type names and descriptions        Not implemented
/// attributes  Attribute names and descriptions   Not implemented
/// events      Event descriptions and parameters  Not implemented
/// methods     Method descriptions and members    Not implemented
/// ```
///
/// Events contain `parameters`. Methods contain `parameters` and `exceptions`.
/// Each entry identifies the component by `name` and supplies a `description`.
///
/// Empty arrays are omitted. The whole block is accepted only when all four
/// arrays are empty. Nonempty edits are rejected because there is no implemented
/// ADT backing. Duplicate complete entries are rejected by schema validation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct ClassDescriptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub types: Vec<NameDescription>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub attributes: Vec<NameDescription>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub events: Vec<EventDescription>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub methods: Vec<MethodDescription>,
}

impl ClassDescriptions {
    pub(super) fn is_empty(&self) -> bool {
        self.types.is_empty()
            && self.attributes.is_empty()
            && self.events.is_empty()
            && self.methods.is_empty()
    }
}

/// A named Class component description.
///
/// Used by `descriptions.types`, `descriptions.attributes`, event parameters,
/// method parameters, and method exceptions.
///
/// ```text
/// AFF field    Meaning                          ADT backing
/// ---------    -------                          -----------
/// name         Name of the described component  Not implemented
/// description  SE80 description text            Not implemented
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct NameDescription {
    #[garde(length(chars, max = 30))]
    pub name: String,

    #[garde(length(chars, max = 60))]
    pub description: String,
}

/// A Class event description and its parameter descriptions.
///
/// Paths below are relative to an entry in `descriptions.events`.
///
/// ```text
/// AFF field    Meaning                       ADT backing
/// ---------    -------                       -----------
/// name         Event name                    Not implemented
/// description  Event description             Not implemented
/// parameters   Named parameter descriptions  Not implemented
/// ```
///
/// Parameter entries use [`NameDescription`]. Even an event with no parameters
/// is a nonempty component-description edit and is rejected by this mapping.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct EventDescription {
    #[garde(length(chars, max = 30))]
    pub name: String,

    #[garde(length(chars, max = 60))]
    pub description: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub parameters: Vec<NameDescription>,
}

/// A Class method description and its parameter and exception descriptions.
///
/// Paths below are relative to an entry in `descriptions.methods`.
///
/// ```text
/// AFF field    Meaning                       ADT backing
/// ---------    -------                       -----------
/// name         Method name                   Not implemented
/// description  Method description            Not implemented
/// parameters   Named parameter descriptions  Not implemented
/// exceptions   Named exception descriptions  Not implemented
/// ```
///
/// Parameter and exception entries use [`NameDescription`]. These fields have
/// no implemented ADT backing, so nonempty method entries are rejected.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct MethodDescription {
    #[garde(length(chars, max = 30))]
    pub name: String,

    #[garde(length(chars, max = 60))]
    pub description: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub parameters: Vec<NameDescription>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive, custom(unique_items))]
    pub exceptions: Vec<NameDescription>,
}

impl ProjectedClassProperties {
    fn from_adt(properties: &ClassProperties) -> Result<Self, ProjectionError> {
        let document = Self {
            format_version: CLASS_FORMAT.version().to_owned(),
            header: ClassHeader {
                description: properties.description.clone(),
                original_language: language_from_adt(
                    &properties.master_language,
                    "header.originalLanguage",
                )?,
                abap_language_version: AbapLanguageVersion::from_adt(
                    properties.abap_language_version.as_ref(),
                    "X",
                )
                .map_err(|value| ProjectionError::InvalidAffField {
                    field: "header.abapLanguageVersion",
                    message: format!("unsupported ADT value `{value}`"),
                })?,
            },
            category: ClassCategory::from_adt(&properties.category)?,
            fix_point_arithmetic: properties.fix_point_arithmetic,
            message_class: properties
                .message_class
                .as_ref()
                .and_then(|reference| reference.name.clone())
                .unwrap_or_default(),
            descriptions: None,
        };
        document.validate()?;
        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use zadt::{
        AbapLanguageVersion as AdtAbapLanguageVersion, Class, ClassProperties, ObjectSnapshot,
        ToXml,
    };

    use super::*;

    const CLASS_XML: &[u8] =
        include_bytes!("../../../zadt/tests/fixtures/class-cl-adt-uri-mapper-v4.xml");

    const MODELED_CATEGORIES: &[(AdtClassCategory, &str, ClassCategory, &str)] = &[
        (
            AdtClassCategory::GeneralObjectType,
            "generalObjectType",
            ClassCategory::GeneralObjectType,
            "generalObjectType",
        ),
        (
            AdtClassCategory::ExceptionClass,
            "exceptionClass",
            ClassCategory::ExceptionClass,
            "exceptionClass",
        ),
        (
            AdtClassCategory::TestClass,
            "testClass",
            ClassCategory::TestclassAbapUnit,
            "testclassAbapUnit",
        ),
        (
            AdtClassCategory::AreaClass,
            "areaClass",
            ClassCategory::AreaClassSharedObjects,
            "areaClassSharedObjects",
        ),
        (
            AdtClassCategory::BspClass,
            "bspClass",
            ClassCategory::BspApplicationClass,
            "bspApplicationClass",
        ),
        (
            AdtClassCategory::BehaviorPool,
            "behaviorPool",
            ClassCategory::BehaviorClass,
            "behaviorClass",
        ),
        (
            AdtClassCategory::RfcProxyClass,
            "rfcProxyClass",
            ClassCategory::RfcProxyClass,
            "rfcProxyClass",
        ),
    ];

    fn class() -> ClassProperties {
        class_snapshot(CLASS_XML).properties().clone()
    }

    fn class_snapshot(xml: &[u8]) -> ObjectSnapshot<Class> {
        let reference = crate::test_support::reference::<Class>(
            "CL_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper",
        );
        crate::test_support::properties(&reference, Class::MEDIA_TYPES[0], "class-etag", xml)
    }

    fn snapshot(properties: &ClassProperties) -> ObjectSnapshot<()> {
        class_snapshot(&properties.to_xml().unwrap()).into_erased()
    }

    #[test]
    fn modeled_categories_render_and_merge_without_wire_changes() {
        for (adt, adt_wire, aff, aff_wire) in MODELED_CATEGORIES {
            let xml = std::str::from_utf8(CLASS_XML).unwrap().replace(
                "class:category=\"generalObjectType\"",
                &format!("class:category=\"{adt_wire}\""),
            );
            let snapshot = class_snapshot(xml.as_bytes());
            assert_eq!(&snapshot.properties().category, adt);
            let projection = crate::project(snapshot.into_erased()).unwrap();
            let crate::FileBacking::Properties(properties) = projection
                .file("cl_adt_uri_mapper.clas.json")
                .unwrap()
                .backing()
            else {
                panic!("class JSON must have a properties backing");
            };
            let content = properties.render().unwrap();
            let document: ProjectedClassProperties = serde_json::from_str(&content).unwrap();
            assert_eq!(&document.category, aff);
            let document: Value = serde_json::from_str(&content).unwrap();
            if aff.is_default() {
                assert!(document.get("category").is_none());
            } else {
                assert_eq!(document["category"], *aff_wire);
            }
            assert_eq!(properties.merge(&content).unwrap(), None);
        }
    }

    #[test]
    fn modeled_category_edits_roundtrip_using_adt_wire_values() {
        for (original_category, _, _, _) in MODELED_CATEGORIES {
            let mut original = class();
            original.category = original_category.clone();
            let snapshot = snapshot(&original);
            for (adt, adt_wire, aff, _) in MODELED_CATEGORIES {
                let mut edited = ProjectedClassProperties::from_adt(&original).unwrap();
                edited.category = *aff;
                let payload = merge(&snapshot, &serde_json::to_string(&edited).unwrap()).unwrap();
                let mut expected = original.clone();
                expected.category = adt.clone();
                assert_eq!(
                    payload,
                    (expected != original).then(|| serde_json::to_value(&expected).unwrap())
                );
                let merged = payload.unwrap_or_else(|| snapshot.properties().unwrap());
                assert_eq!(merged["@class:category"], *adt_wire);
                let merged: ClassProperties = serde_json::from_value(merged).unwrap();
                let rendered = ProjectedClassProperties::from_adt(&merged).unwrap();
                assert_eq!(rendered, edited);
            }
        }
    }

    #[test]
    fn class_render_merge_preserves_language_encoding_and_message_reference_wire_fields() {
        for language_version in [None, Some(""), Some(" "), Some("X"), Some("2"), Some("5")] {
            let attribute = language_version
                .map(|value| format!("adtcore:abapLanguageVersion=\"{value}\""))
                .unwrap_or_default();
            let xml = std::str::from_utf8(CLASS_XML).unwrap()
                .replace("adtcore:abapLanguageVersion=\"X\"", &attribute)
                .replace(
                    "</class:abapClass>",
                    r#"<class:messageClassRef adtcore:name="Z_MESSAGES" adtcore:type="MSAG/N" adtcore:uri="/sap/bc/adt/messageclass/z_messages" adtcore:description="Messages"/>
                    </class:abapClass>"#,
                );
            let snapshot = class_snapshot(xml.as_bytes()).into_erased();
            let original = snapshot.properties().unwrap();
            let projection = crate::project(snapshot).unwrap();
            let crate::FileBacking::Properties(properties) = projection
                .file("cl_adt_uri_mapper.clas.json")
                .unwrap()
                .backing()
            else {
                panic!("class JSON must have a properties backing");
            };
            let content = properties.render().unwrap();
            let document: Value = serde_json::from_str(&content).unwrap();
            assert_eq!(document["messageClass"], "Z_MESSAGES");
            assert_eq!(document["header"]["originalLanguage"], "en");
            assert_eq!(properties.merge(&content).unwrap(), None);
            assert_eq!(properties.subject().properties().unwrap(), original);

            let mut edited = document;
            edited["header"]["description"] = json!("Updated class");
            let mut expected = original;
            expected["@adtcore:description"] = json!("Updated class");
            assert_eq!(
                properties.merge(&edited.to_string()).unwrap(),
                Some(expected)
            );
        }
    }

    #[test]
    fn language_version_edits_preserve_unchanged_encodings() {
        for original_version in [
            None,
            Some(AdtAbapLanguageVersion::Other(String::new())),
            Some(AdtAbapLanguageVersion::Other(" ".to_owned())),
            Some(AdtAbapLanguageVersion::StandardX),
            Some(AdtAbapLanguageVersion::KeyUser),
            Some(AdtAbapLanguageVersion::CloudDevelopment),
        ] {
            let mut original = class();
            original.abap_language_version = original_version;
            let snapshot = snapshot(&original);
            let baseline = ProjectedClassProperties::from_adt(&original).unwrap();
            for (aff, adt) in [
                (
                    AbapLanguageVersion::Standard,
                    AdtAbapLanguageVersion::StandardX,
                ),
                (
                    AbapLanguageVersion::KeyUser,
                    AdtAbapLanguageVersion::KeyUser,
                ),
                (
                    AbapLanguageVersion::CloudDevelopment,
                    AdtAbapLanguageVersion::CloudDevelopment,
                ),
            ] {
                let mut edited = baseline.clone();
                edited.header.description = "Updated class".to_owned();
                edited.header.abap_language_version = aff;
                let mut expected = original.clone();
                expected.description = edited.header.description.clone();
                if aff != baseline.header.abap_language_version {
                    expected.abap_language_version = Some(adt);
                }
                let merged = merge(&snapshot, &serde_json::to_string(&edited).unwrap())
                    .unwrap()
                    .unwrap();
                assert_eq!(merged, serde_json::to_value(expected).unwrap());
                let merged: ClassProperties = serde_json::from_value(merged).unwrap();
                assert_eq!(ProjectedClassProperties::from_adt(&merged).unwrap(), edited);
            }
        }
    }

    #[test]
    fn preserves_existing_unmodeled_category_mappings_and_rejects_unknown_values() {
        for value in [
            "exitClass",
            "testclassAbapUnit",
            "behaviorClass",
            "entityEventHandler",
            "persistentClass",
            "factoryForPersistentClass",
            "statusClassForPersistClass",
            "communicationConnectionClass",
            "areaClassSharedObjects",
            "businessClass",
            "bspApplicationClass",
            "basisClassBspElementHdlr",
            "webDynproRuntimeObject",
        ] {
            let mut original = class();
            original.category = AdtClassCategory::Other(value.to_owned());
            let snapshot = snapshot(&original);
            let content = render(&snapshot).unwrap();
            let document: Value = serde_json::from_str(&content).unwrap();
            assert_eq!(document["category"], value);
            assert_eq!(merge(&snapshot, &content).unwrap(), None);
        }
        for value in ["00", "backendSpecific"] {
            let mut original = class();
            original.category = AdtClassCategory::Other(value.to_owned());
            assert!(matches!(
                render(&snapshot(&original)),
                Err(ProjectionError::InvalidAffField {
                    field: "category",
                    ..
                })
            ));
        }
    }

    #[test]
    fn renders_class_properties_as_canonical_aff_v1() {
        let properties = class_snapshot(CLASS_XML).into_erased();
        let content = render(&properties).unwrap();
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
        let snapshot = snapshot(&original);
        let mut edited: ProjectedClassProperties =
            serde_json::from_str(&render(&snapshot).unwrap()).unwrap();
        edited.header.description = "Updated class".to_owned();
        edited.header.original_language = "en-GB".to_owned();
        edited.header.abap_language_version = AbapLanguageVersion::KeyUser;
        edited.category = ClassCategory::BusinessClass;
        edited.fix_point_arithmetic = false;
        edited.message_class = "Z_MESSAGES".to_owned();

        let merged = merge(&snapshot, &serde_json::to_string(&edited).unwrap())
            .unwrap()
            .unwrap();
        let merged: ClassProperties = serde_json::from_value(merged).unwrap();

        assert_eq!(merged.description, "Updated class");
        assert_eq!(merged.master_language, "6N");
        assert_eq!(
            merged.abap_language_version,
            Some(AdtAbapLanguageVersion::KeyUser)
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
        let original = class_snapshot(CLASS_XML).into_erased();
        let mut edited: ProjectedClassProperties =
            serde_json::from_str(&render(&original).unwrap()).unwrap();
        edited.descriptions = Some(ClassDescriptions {
            methods: vec![MethodDescription {
                name: "RUN".to_owned(),
                description: "Runs the class".to_owned(),
                parameters: Vec::new(),
                exceptions: Vec::new(),
            }],
            ..Default::default()
        });

        assert!(matches!(
            merge(&original, &serde_json::to_string(&edited).unwrap()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "descriptions",
                ..
            })
        ));
    }

    #[test]
    fn validates_class_schema_fields_and_unique_description_names() {
        let original = class_snapshot(CLASS_XML).into_erased();
        let content = render(&original).unwrap();
        let unknown = content.replacen('{', "{\n  \"unknown\": true,", 1);
        assert!(matches!(
            merge(&original, &unknown),
            Err(ProjectionError::Json(_))
        ));

        let mut duplicate: ProjectedClassProperties = serde_json::from_str(&content).unwrap();
        duplicate.descriptions = Some(ClassDescriptions {
            types: vec![
                NameDescription {
                    name: "TYPE".to_owned(),
                    description: "First".to_owned(),
                },
                NameDescription {
                    name: "TYPE".to_owned(),
                    description: "First".to_owned(),
                },
            ],
            ..Default::default()
        });
        let duplicate = serde_json::to_string(&duplicate).unwrap();
        assert!(matches!(
            merge(&original, &duplicate),
            Err(ProjectionError::Validation(_))
        ));

        let mut same_name: ProjectedClassProperties = serde_json::from_str(&content).unwrap();
        same_name.descriptions = Some(ClassDescriptions {
            types: vec![
                NameDescription {
                    name: "TYPE".to_owned(),
                    description: "First".to_owned(),
                },
                NameDescription {
                    name: "TYPE".to_owned(),
                    description: "Second".to_owned(),
                },
            ],
            ..Default::default()
        });
        assert!(matches!(
            merge(&original, &serde_json::to_string(&same_name).unwrap()),
            Err(ProjectionError::UnsupportedAffProperty {
                field: "descriptions",
                ..
            })
        ));

        let mut invalid_language: Value = serde_json::from_str(&content).unwrap();
        invalid_language["header"]["originalLanguage"] = json!("not-supported");
        assert!(matches!(
            merge(&original, &invalid_language.to_string()),
            Err(ProjectionError::InvalidAffField {
                field: "header.originalLanguage",
                ..
            })
        ));
    }
}
