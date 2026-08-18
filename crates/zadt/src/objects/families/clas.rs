use super::super::{
    AbapLanguageVersion, AdvertisedObjectReference, GlobalWorkbenchType, ObjectRef, ObjectType,
    PropertyModel, Source, SourceComponents,
};
use crate::resource::AdvertisedLink;
use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

#[object_type(
    workbench_type = "CLAS/OC",
    collection(scheme = "http://www.sap.com/adt/categories/oo", term = "classes",),
    capabilities(
        Create(ClassCreateProperties, ClassPropertiesVersion::V4),
        Source,
        SourceComponents,
        Run,
        UpdateProperties,
    )
)]
/// An ABAP global class object.
///
/// Classes are one of the more special objects ADT provides, because the main
/// global class include that ADT lets us edit is, in reality, a projection of
/// individual includes. Each method, the declaration, testclasses, and more
/// have their own include under the hood.
///
/// In ADT, only a small subset of the includes matter for us - such as the main
/// source, local types and the testclasses. [`ClassSourceComponent`] describes
/// the full list of possible source includes. Which includes are available also
/// differs based on how old the class is. Legacy classes follow a different layout.
pub type Class = ClassProperties;

/// The media-type version used to decode class properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClassPropertiesVersion {
    V2,
    V3,
    V4,
}

impl ClassPropertiesVersion {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.oo.classes.v2+xml",
            Self::V3 => "application/vnd.sap.adt.oo.classes.v3+xml",
            Self::V4 => "application/vnd.sap.adt.oo.classes.v4+xml",
        }
    }
}

/// The properties describing a class. Contains editable information, such
/// as `description` and `shared-memory-enabled`.
///
/// The same payload is used for updating as is retrieved when reading the
/// properties initially.
///
/// Despite ADT advertising several different versions of the media, they
/// all seem to contain the same fields.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = ClassCreateProperties,
    doc = "The sparse V4 payload used to create an ABAP class."
)]
#[serde(rename = "class:abapClass", deny_unknown_fields)]
pub struct ClassProperties {
    /// The class name supplied by SAP.
    #[for_create(identity, default, doc = "The class name.")]
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    /// The repository object type, normally `CLAS/OC`.
    #[for_create(
        identity,
        default = <Class as ObjectType>::WORKBENCH_TYPE,
        doc = "The class's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which the class was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    /// The object version exactly as advertised by SAP.
    #[serde(rename = "@adtcore:version")]
    pub version: String,
    /// The timestamp at which the class was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    /// The user who last changed the class.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    /// The user who created the class.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,
    /// The class description.
    #[for_create]
    #[serde(rename = "@adtcore:description")]
    pub description: String,
    /// The maximum class-description length.
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,
    /// The class's logon language.
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    /// The user responsible for the class.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,
    /// The class's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,
    /// The class's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,
    /// The configured ABAP language version when supplied by the media version.
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: Option<AbapLanguageVersion>,
    /// The purpose assigned to this source object by SAP.
    #[serde(rename = "@abapsource:sourceObjectStatus")]
    pub source_object_status: Option<String>,
    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,
    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,
    /// Whether this class is maintained through a higher-level model.
    #[serde(rename = "@abapoo:modeled")]
    pub modeled: bool,
    /// The semantic class category, such as `generalObjectType`.
    #[for_create(
        optional,
        doc = "The semantic class category, when explicitly requested."
    )]
    #[serde(rename = "@class:category")]
    pub category: ClassCategory,
    /// Whether the class is final.
    #[for_create(default = true, doc = "Whether the created class is final.")]
    #[serde(rename = "@class:final")]
    pub is_final: bool,
    /// Whether the class is abstract.
    #[serde(rename = "@class:abstract")]
    pub is_abstract: bool,
    /// The class visibility.
    #[for_create(default = String::from("public"))]
    #[serde(rename = "@class:visibility")]
    pub visibility: String,
    /// An optional class state supplied by SAP.
    #[serde(rename = "@class:state")]
    pub state: Option<String>,
    /// Whether shared-memory support is enabled.
    #[serde(rename = "@class:sharedMemoryEnabled")]
    pub shared_memory_enabled: bool,
    /// Whether SAP generated the constructor.
    #[serde(rename = "@class:constructorGenerated", default)]
    pub constructor_generated: bool,
    /// Whether SAP explicitly marks this class as having tests.
    #[serde(rename = "@class:hasTests", default)]
    pub has_tests: bool,
    /// Atom links advertised for the class, in document order.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
    /// The package reference advertised for the class.
    #[for_create(doc = "The package receiving the class.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
    /// The syntax configuration advertised for the class.
    #[serde(rename = "abapsource:syntaxConfiguration")]
    pub syntax_configuration: Option<ClassSyntaxConfiguration>,
    /// The source template used to create the class, when advertised.
    #[for_create(
        optional,
        doc = "The source template used to create or populate the class."
    )]
    #[serde(rename = "abapsource:template")]
    pub template: Option<ClassTemplate>,
    /// Interfaces implemented by the class.
    #[serde(rename = "abapoo:interfaceRef", default)]
    pub interfaces: Vec<AdvertisedObjectReference>,
    /// Source includes advertised for the class, in document order.
    #[for_create(
        default = vec![ClassSourceProperties::test_classes()],
        each = "source",
        with = "class_create_sources",
        doc = "Source includes requested for the class."
    )]
    #[serde(rename = "class:include", default)]
    pub sources: Vec<ClassSourceProperties>,
    /// The direct superclass reference, when advertised.
    #[for_create(
        optional,
        default = Some(AdvertisedObjectReference::default()),
        doc = "The direct superclass, or an empty reference for no superclass."
    )]
    #[serde(rename = "class:superClassRef")]
    pub super_class: Option<AdvertisedObjectReference>,
    /// The assigned message-class reference, when advertised.
    #[serde(rename = "class:messageClassRef")]
    pub message_class: Option<AdvertisedObjectReference>,
    /// The root-entity reference, when advertised.
    #[serde(rename = "class:rootEntityRef")]
    pub root_entity: Option<AdvertisedObjectReference>,
}

impl PropertyModel for ClassProperties {
    type Version = ClassPropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] = &[
        ClassPropertiesVersion::V4,
        ClassPropertiesVersion::V3,
        ClassPropertiesVersion::V2,
    ];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("class", "http://www.sap.com/adt/oo/classes"),
        ("abapoo", "http://www.sap.com/adt/oo"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];

    fn media_type(version: Self::Version) -> &'static str {
        version.media_type()
    }

    fn object_name(&self) -> &str {
        &self.name
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }
}

/// Semantic category assigned to an ABAP class.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ClassCategory {
    GeneralObjectType,
    ExceptionClass,
    TestClass,
    AreaClass,
    BspClass,
    BehaviorPool,
    RfcProxyClass,
    /// A backend-specific category value, including two-digit categories.
    Other(String),
}

impl ClassCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::GeneralObjectType => "generalObjectType",
            Self::ExceptionClass => "exceptionClass",
            Self::TestClass => "testClass",
            Self::AreaClass => "areaClass",
            Self::BspClass => "bspClass",
            Self::BehaviorPool => "behaviorPool",
            Self::RfcProxyClass => "rfcProxyClass",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for ClassCategory {
    fn from(value: String) -> Self {
        match value.as_str() {
            "generalObjectType" => Self::GeneralObjectType,
            "exceptionClass" => Self::ExceptionClass,
            "testClass" => Self::TestClass,
            "areaClass" => Self::AreaClass,
            "bspClass" => Self::BspClass,
            "behaviorPool" => Self::BehaviorPool,
            "rfcProxyClass" => Self::RfcProxyClass,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for ClassCategory {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for ClassCategory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ClassCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::from)
    }
}

impl PropertyModel for ClassCreateProperties {
    type Version = ClassPropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] = &[ClassPropertiesVersion::V4];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("class", "http://www.sap.com/adt/oo/classes"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
    ];

    fn media_type(version: Self::Version) -> &'static str {
        version.media_type()
    }

    fn object_name(&self) -> &str {
        &self.name
    }

    fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }
}

/// A source template associated with an ABAP class.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassTemplate {
    /// The existing class name or ADT template implementation name.
    #[serde(rename = "@abapsource:name")]
    pub name: String,
    /// Parameters passed to an ADT template implementation.
    #[serde(
        rename = "abapsource:property",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub properties: Vec<ClassTemplateProperty>,
}

impl ClassTemplate {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: Vec::new(),
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push(ClassTemplateProperty::new(key, value));
        self
    }
}

/// One parameter passed to an ADT source template.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassTemplateProperty {
    /// The template-defined parameter key.
    #[serde(rename = "@abapsource:key")]
    pub key: String,
    /// The parameter value stored as element text.
    #[serde(rename = "#text", default)]
    pub value: String,
}

impl ClassTemplateProperty {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

impl Source for Class {
    fn source_uri(properties: &Self::Properties) -> Option<&str> {
        properties
            .sources
            .iter()
            .find(|source| source.include_type == "main")
            .map(|source| source.source_uri.as_str())
    }
}

impl SourceComponents for Class {
    fn source_component_uri<'a>(properties: &'a Self::Properties, name: &str) -> Option<&'a str> {
        properties
            .sources
            .iter()
            .find(|source| source.include_type == name)
            .map(|source| source.source_uri.as_str())
    }
}

/// The syntax configuration embedded in a class-properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassSyntaxConfiguration {
    /// The configured ABAP language, when advertised.
    #[serde(rename = "abapsource:language")]
    pub language: Option<ClassSyntaxLanguage>,
}

/// An ABAP language description embedded in a class syntax configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassSyntaxLanguage {
    /// The language-version token.
    #[serde(rename = "abapsource:version")]
    pub version: String,
    /// The language description.
    #[serde(rename = "abapsource:description")]
    pub description: String,
    /// Atom links advertised for this syntax language.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

/// One source include embedded in a class-properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClassSourceProperties {
    /// The include type exactly as advertised by SAP.
    #[serde(rename = "@class:includeType")]
    pub include_type: String,
    /// The source URI exactly as advertised by SAP.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,
    /// The include name exactly as advertised by SAP.
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    /// The include object type.
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which this source was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    /// The source version exactly as advertised by SAP.
    #[serde(rename = "@adtcore:version")]
    pub version: String,
    /// The timestamp at which this source was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    /// The user who last changed this source.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    /// The user who created this source.
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,
    /// Atom links advertised for this source, in document order.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl ClassSourceProperties {
    /// Creates a sparse source declaration for class creation.
    pub fn for_creation(
        name: impl Into<String>,
        object_type: GlobalWorkbenchType,
        include_type: impl Into<String>,
    ) -> Self {
        Self {
            include_type: include_type.into(),
            source_uri: String::new(),
            name: name.into(),
            object_type,
            last_changed: String::new(),
            version: String::new(),
            created_at: String::new(),
            changed_by: String::new(),
            created_by: String::new(),
            links: Vec::new(),
        }
    }

    /// Creates the source declaration that enables local test classes.
    pub fn test_classes() -> Self {
        Self::for_creation(
            "CLAS/OC",
            <Class as ObjectType>::WORKBENCH_TYPE,
            "testclasses",
        )
    }
}

mod class_create_sources {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{ClassSourceProperties, GlobalWorkbenchType};

    #[derive(Serialize)]
    struct WireSource<'a> {
        #[serde(rename = "@adtcore:name")]
        name: &'a str,
        #[serde(rename = "@adtcore:type")]
        object_type: &'a GlobalWorkbenchType,
        #[serde(rename = "@class:includeType")]
        include_type: &'a str,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OwnedWireSource {
        #[serde(rename = "@adtcore:name")]
        name: String,
        #[serde(rename = "@adtcore:type")]
        object_type: GlobalWorkbenchType,
        #[serde(rename = "@class:includeType")]
        include_type: String,
    }

    pub fn serialize<S>(sources: &[ClassSourceProperties], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        sources
            .iter()
            .map(|source| WireSource {
                name: &source.name,
                object_type: &source.object_type,
                include_type: &source.include_type,
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ClassSourceProperties>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<OwnedWireSource>::deserialize(deserializer).map(|sources| {
            sources
                .into_iter()
                .map(|source| {
                    ClassSourceProperties::for_creation(
                        source.name,
                        source.object_type,
                        source.include_type,
                    )
                })
                .collect()
        })
    }
}

/// A secondary source component owned and locked by an ABAP class.
///
/// Local class includes are ADT resources beneath the class object rather than
/// independent repository objects.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClassSourceComponent {
    Definitions,
    Implementations,
    Macros,
    TestClasses,
    LocalTypes,
}

impl ClassSourceComponent {
    /// Returns the component name used by ADT.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Definitions => "definitions",
            Self::Implementations => "implementations",
            Self::Macros => "macros",
            Self::TestClasses => "testclasses",
            Self::LocalTypes => "localtypes",
        }
    }

    /// Parses a component name used by ADT.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "definitions" => Some(Self::Definitions),
            "implementations" => Some(Self::Implementations),
            "macros" => Some(Self::Macros),
            "testclasses" => Some(Self::TestClasses),
            "localtypes" => Some(Self::LocalTypes),
            _ => None,
        }
    }
}

impl AsRef<str> for ClassSourceComponent {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl ObjectRef<Class> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::new(name.to_ascii_uppercase(), uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectType;

    const CLASS_XML: &str = include_str!("../../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const LOCAL_TYPES_XML: &str = include_str!("../../../tests/fixtures/class-cx-root-v4.xml");

    fn parse(body: &str) -> Result<ClassProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    #[test]
    fn source_component_names_round_trip() {
        for component in [
            ClassSourceComponent::Definitions,
            ClassSourceComponent::Implementations,
            ClassSourceComponent::Macros,
            ClassSourceComponent::TestClasses,
            ClassSourceComponent::LocalTypes,
        ] {
            assert_eq!(
                ClassSourceComponent::from_name(component.as_str()),
                Some(component)
            );
        }
    }

    #[test]
    fn class_categories_use_the_adt_vocabulary() {
        for (category, value) in [
            (ClassCategory::GeneralObjectType, "generalObjectType"),
            (ClassCategory::ExceptionClass, "exceptionClass"),
            (ClassCategory::TestClass, "testClass"),
            (ClassCategory::AreaClass, "areaClass"),
            (ClassCategory::BspClass, "bspClass"),
            (ClassCategory::BehaviorPool, "behaviorPool"),
            (ClassCategory::RfcProxyClass, "rfcProxyClass"),
            (ClassCategory::Other("42".to_owned()), "42"),
        ] {
            assert_eq!(category.as_str(), value);
            assert_eq!(serde_json::to_value(&category).unwrap(), value);
            assert_eq!(
                serde_json::from_value::<ClassCategory>(value.into()).unwrap(),
                category
            );
        }
    }

    #[test]
    fn builds_sparse_class_creation_properties() {
        let properties = ClassCreatePropertiesBuilder::default()
            .description("Created class")
            .template(ClassTemplate::new("ZOTHERCLASS"))
            .package(AdvertisedObjectReference {
                name: Some("$TMP".to_owned()),
                ..Default::default()
            })
            .build()
            .unwrap();

        assert_eq!(
            properties.sources,
            vec![ClassSourceProperties::test_classes()]
        );
        assert_eq!(
            properties.super_class,
            Some(AdvertisedObjectReference::default())
        );
        assert_eq!(properties.name, "");
        assert_eq!(properties.object_type, Class::WORKBENCH_TYPE);
        assert_eq!(properties.template, Some(ClassTemplate::new("ZOTHERCLASS")));
        assert_eq!(properties.sources[0].source_uri, "");
        assert!(properties.is_final);
        assert_eq!(properties.visibility, "public");
    }

    #[test]
    fn parses_the_complete_live_v4_payload_without_transforming_it() {
        let class = parse(CLASS_XML).unwrap();

        assert_eq!(class.name, "CL_ADT_URI_MAPPER");
        assert_eq!(class.object_type, Class::WORKBENCH_TYPE);
        assert_eq!(class.version, "active");
        assert_eq!(
            class.abap_language_version,
            Some(AbapLanguageVersion::StandardX)
        );
        assert_eq!(class.category, ClassCategory::GeneralObjectType);
        assert_eq!(class.package.name.as_deref(), Some("SADT_TOOLS_CORE"));
        assert_eq!(class.links.len(), 7);
        assert_eq!(class.sources.len(), 5);
        assert_eq!(
            class
                .syntax_configuration
                .unwrap()
                .language
                .unwrap()
                .links
                .len(),
            1
        );

        let main = class
            .sources
            .iter()
            .find(|source| source.include_type == "main")
            .unwrap();
        assert_eq!(main.source_uri, "source/main");
        assert_eq!(main.version, "active");
        assert_eq!(main.links.len(), 4);
        assert_eq!(main.links[0].href, "includes/main/versions");
    }

    #[test]
    fn parses_the_live_local_types_payload() {
        let class: ClassProperties = serde_xml_rs::from_str(LOCAL_TYPES_XML).unwrap();

        assert!(class.is_abstract);
        assert!(class.constructor_generated);
        assert_eq!(
            class.abap_language_version,
            Some(AbapLanguageVersion::StandardX)
        );
        assert_eq!(class.category, ClassCategory::ExceptionClass);
        assert_eq!(class.sources.len(), 2);
        assert_eq!(class.sources[0].include_type, "localtypes");
        assert_eq!(class.sources[0].source_uri, "includes/localtypes");
    }

    #[test]
    fn parses_v2_without_the_v4_language_version() {
        let body = CLASS_XML.replace(" adtcore:abapLanguageVersion=\"X\"", "");
        let class: ClassProperties = serde_xml_rs::from_str(&body).unwrap();

        assert_eq!(class.abap_language_version, None);
    }

    #[test]
    fn retains_sources_without_classifying_or_resolving_them() {
        let body = CLASS_XML
            .replacen(
                "class:includeType=\"definitions\"",
                "class:includeType=\"future-source\"",
                1,
            )
            .replacen(
                "abapsource:sourceUri=\"includes/definitions\"",
                "abapsource:sourceUri=\"https://example.test/source\"",
                1,
            );
        let class = parse(&body).unwrap();

        assert_eq!(class.sources[0].include_type, "future-source");
        assert_eq!(class.sources[0].source_uri, "https://example.test/source");
    }

    #[test]
    fn retains_empty_object_references() {
        let body = CLASS_XML.replacen(
            "<class:include",
            "<abapoo:interfaceRef/><class:superClassRef/><class:messageClassRef/><class:rootEntityRef/><class:include",
            1,
        );
        let class = parse(&body).unwrap();

        assert_eq!(class.interfaces.len(), 1);
        assert!(class.interfaces[0].uri.is_none());
        assert!(class.super_class.unwrap().uri.is_none());
        assert!(class.message_class.unwrap().uri.is_none());
        assert!(class.root_entity.unwrap().uri.is_none());
    }

    #[test]
    fn wire_json_round_trips_the_full_payload() {
        let class: ClassProperties = serde_xml_rs::from_str(LOCAL_TYPES_XML).unwrap();
        let json = serde_json::to_value(&class).unwrap();
        assert_eq!(json["@adtcore:name"], "CX_ROOT");
        assert_eq!(json["@adtcore:type"], "CLAS/OC");
        assert_eq!(json["@adtcore:abapLanguageVersion"], "X");
        assert_eq!(json["@class:category"], "exceptionClass");
        assert_eq!(
            json["class:include"][1]["@abapsource:sourceUri"],
            "source/main"
        );
        assert_eq!(
            json["atom:link"][0]["@type"],
            "application/vnd.sap.adt.enhancementoptions.v2+xml"
        );

        let round_tripped: ClassProperties = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped.name, "CX_ROOT");
        assert_eq!(round_tripped.links.len(), 7);
        assert_eq!(round_tripped.sources.len(), 2);
        assert_eq!(round_tripped.sources[0].links.len(), 4);
        assert_eq!(
            round_tripped
                .syntax_configuration
                .unwrap()
                .language
                .unwrap()
                .links[0]
                .etag
                .as_deref(),
            Some("757")
        );
    }

    #[test]
    fn serializes_class_properties_as_a_complete_update_payload() {
        fn assert_writable<T: crate::UpdateProperties>() {}
        assert_writable::<Class>();

        let class = parse(CLASS_XML).unwrap();
        let object = crate::ObjectRef::erased(
            class.name.clone(),
            crate::AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            Class::WORKBENCH_TYPE,
        );
        let xml = String::from_utf8(
            Class::DESCRIPTOR
                .properties_to_xml(
                    &object,
                    ClassPropertiesVersion::V4.media_type(),
                    serde_json::to_value(&class).unwrap(),
                )
                .unwrap(),
        )
        .unwrap();

        assert!(xml.contains("<class:abapClass"));
        assert!(xml.contains("xmlns:class=\"http://www.sap.com/adt/oo/classes\""));
        assert!(xml.contains("adtcore:abapLanguageVersion=\"X\""));
        assert!(xml.contains("class:category=\"generalObjectType\""));
        assert!(xml.contains("xmlns:abapoo=\"http://www.sap.com/adt/oo\""));
        assert!(xml.contains("xmlns:abapsource=\"http://www.sap.com/adt/abapsource\""));
        assert!(xml.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
        assert!(xml.contains("xmlns:atom=\"http://www.w3.org/2005/Atom\""));
        assert!(xml.contains("adtcore:name=\"CL_ADT_URI_MAPPER\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<class:include"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse(&xml).unwrap(), class);
    }

    #[test]
    fn rejects_unmodeled_class_property_fields() {
        let xml = CLASS_XML.replacen(
            "class:final=",
            "class:futureAttribute=\"future\" class:final=",
            1,
        );

        assert!(parse(&xml).is_err());
    }

    #[test]
    fn preserves_root_identity_and_nested_wire_values() {
        let wire_values = CLASS_XML
            .replacen("adtcore:type=\"CLAS/OC\"", "adtcore:type=\"PROG/P\"", 1)
            .replacen(
                "adtcore:name=\"CL_ADT_URI_MAPPER\"",
                "adtcore:name=\"OTHER_CLASS\"",
                1,
            )
            .replacen(
                "adtcore:type=\"DEVC/K\"",
                "adtcore:type=\"FUTURE/PACKAGE\"",
                1,
            )
            .replace("adtcore:type=\"CLAS/I\"", "adtcore:type=\"FUTURE/INCLUDE\"")
            .replace("adtcore:version=\"active\"", "adtcore:version=\"future\"");
        let class = parse(&wire_values).unwrap();
        assert_eq!(class.object_type.as_str(), "PROG/P");
        assert_eq!(class.name, "OTHER_CLASS");
        assert_eq!(class.version, "future");
        assert_eq!(
            class.package.object_type.as_ref().unwrap().as_str(),
            "FUTURE/PACKAGE"
        );
        assert!(
            class
                .sources
                .iter()
                .all(|include| include.object_type.as_str() == "FUTURE/INCLUDE")
        );
    }
}
