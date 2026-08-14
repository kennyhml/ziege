use serde::{Deserialize, Serialize};

use crate::{
    AdvertisedLink, AdvertisedObjectReference, Class, GlobalWorkbenchType, ObjectRef, PropertyModel,
};

/// The plain-text console output produced by running an ABAP class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassRunResult {
    /// The class that was executed.
    pub reference: ObjectRef<Class>,

    /// The rendered class-run output returned by SAP.
    pub content: String,
}

impl ClassRunResult {
    pub(crate) fn new(reference: ObjectRef<Class>, content: String) -> Self {
        Self { reference, content }
    }
}

/// The SAP media-type version used to decode class properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClassPropertiesVersion {
    /// Class properties V2.
    V2,
    /// Class properties V3.
    V3,
    /// Class properties V4.
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

/// The currently modeled class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "class:abapClass")]
pub struct ClassProperties {
    /// The class name supplied by SAP.
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    /// The repository object type, normally `CLAS/OC`.
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
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: Option<String>,
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
    #[serde(rename = "@class:category")]
    pub category: String,
    /// Whether the class is final.
    #[serde(rename = "@class:final")]
    pub is_final: bool,
    /// Whether the class is abstract.
    #[serde(rename = "@class:abstract")]
    pub is_abstract: bool,
    /// The class visibility.
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
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
    /// The syntax configuration advertised for the class.
    #[serde(rename = "abapsource:syntaxConfiguration")]
    pub syntax_configuration: Option<ClassSyntaxConfiguration>,
    /// Interfaces implemented by the class.
    #[serde(rename = "abapoo:interfaceRef", default)]
    pub interfaces: Vec<AdvertisedObjectReference>,
    /// Source includes advertised for the class, in document order.
    #[serde(rename = "class:include", default)]
    pub sources: Vec<ClassSourceProperties>,
    /// The direct superclass reference, when advertised.
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

    fn media_type(version: Self::Version) -> &'static str {
        version.media_type()
    }
}

/// The syntax configuration embedded in a class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
pub struct ClassSyntaxConfiguration {
    /// The configured ABAP language, when advertised.
    #[serde(rename = "abapsource:language")]
    pub language: Option<ClassSyntaxLanguage>,
}

/// An ABAP language description embedded in a class syntax configuration.
#[derive(Debug, Deserialize, Serialize)]
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
#[derive(Debug, Deserialize, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectType;

    const CLASS_XML: &str = include_str!("../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const LOCAL_TYPES_XML: &str = include_str!("../../tests/fixtures/class-cx-root-v4.xml");

    fn parse(body: &str) -> Result<ClassProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    #[test]
    fn parses_the_complete_live_v4_payload_without_transforming_it() {
        let class = parse(CLASS_XML).unwrap();

        assert_eq!(class.name, "CL_ADT_URI_MAPPER");
        assert_eq!(class.object_type, Class::WORKBENCH_TYPE);
        assert_eq!(class.version, "active");
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
