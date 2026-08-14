use serde::{Deserialize, Serialize};

use crate::{
    Class, GlobalWorkbenchType, MediaVersionNegotiation, ObjectError, ObjectRef, ObjectType,
    RawObjectProperties, ResponseError,
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

impl MediaVersionNegotiation for ClassPropertiesVersion {
    const SUPPORTED: &'static [Self] = &[Self::V4, Self::V3, Self::V2];

    fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.oo.classes.v2+xml",
            Self::V3 => "application/vnd.sap.adt.oo.classes.v3+xml",
            Self::V4 => "application/vnd.sap.adt.oo.classes.v4+xml",
        }
    }
}

/// The class-properties payload shared by the V2, V3, and V4 media types.
#[derive(Debug, Deserialize, Serialize)]
#[serde(
    rename(deserialize = "class:abapClass"),
    rename_all(serialize = "camelCase")
)]
pub struct ClassProperties {
    /// The class name supplied by SAP.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,
    /// The repository object type, normally `CLAS/OC`.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which the class was last changed.
    #[serde(rename(deserialize = "@adtcore:changedAt"), alias = "lastChanged")]
    pub last_changed: String,
    /// The object version exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:version"), alias = "version")]
    pub version: String,
    /// The timestamp at which the class was created.
    #[serde(rename(deserialize = "@adtcore:createdAt"), alias = "createdAt")]
    pub created_at: String,
    /// The user who last changed the class.
    #[serde(rename(deserialize = "@adtcore:changedBy"), alias = "changedBy")]
    pub changed_by: String,
    /// The user who created the class.
    #[serde(rename(deserialize = "@adtcore:createdBy"), alias = "createdBy")]
    pub created_by: String,
    /// The class description.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: String,
    /// The maximum class-description length.
    #[serde(
        rename(deserialize = "@adtcore:descriptionTextLimit"),
        alias = "descriptionTextLimit"
    )]
    pub description_text_limit: u32,
    /// The class's logon language.
    #[serde(rename(deserialize = "@adtcore:language"), alias = "language")]
    pub language: String,
    /// The user responsible for the class.
    #[serde(rename(deserialize = "@adtcore:responsible"), alias = "responsible")]
    pub responsible: String,
    /// The class's master language.
    #[serde(
        rename(deserialize = "@adtcore:masterLanguage"),
        alias = "masterLanguage"
    )]
    pub master_language: String,
    /// The class's master system.
    #[serde(rename(deserialize = "@adtcore:masterSystem"), alias = "masterSystem")]
    pub master_system: String,
    /// The configured ABAP language version when supplied by the media version.
    #[serde(
        rename(deserialize = "@adtcore:abapLanguageVersion"),
        alias = "abapLanguageVersion"
    )]
    pub abap_language_version: Option<String>,
    /// The purpose assigned to this source object by SAP.
    #[serde(
        rename(deserialize = "@abapsource:sourceObjectStatus"),
        alias = "sourceObjectStatus"
    )]
    pub source_object_status: Option<String>,
    /// Whether fixed-point arithmetic is enabled.
    #[serde(
        rename(deserialize = "@abapsource:fixPointArithmetic"),
        alias = "fixPointArithmetic"
    )]
    pub fix_point_arithmetic: bool,
    /// Whether the active Unicode check is enabled.
    #[serde(
        rename(deserialize = "@abapsource:activeUnicodeCheck"),
        alias = "unicodeCheckActive"
    )]
    pub unicode_check_active: bool,
    /// Whether this class is maintained through a higher-level model.
    #[serde(rename(deserialize = "@abapoo:modeled"), alias = "modeled")]
    pub modeled: bool,
    /// The semantic class category, such as `generalObjectType`.
    #[serde(rename(deserialize = "@class:category"), alias = "category")]
    pub category: String,
    /// Whether the class is final.
    #[serde(rename(deserialize = "@class:final"), alias = "isFinal")]
    pub is_final: bool,
    /// Whether the class is abstract.
    #[serde(rename(deserialize = "@class:abstract"), alias = "isAbstract")]
    pub is_abstract: bool,
    /// The class visibility.
    #[serde(rename(deserialize = "@class:visibility"), alias = "visibility")]
    pub visibility: String,
    /// An optional class state supplied by SAP.
    #[serde(rename(deserialize = "@class:state"), alias = "state")]
    pub state: Option<String>,
    /// Whether shared-memory support is enabled.
    #[serde(
        rename(deserialize = "@class:sharedMemoryEnabled"),
        alias = "sharedMemoryEnabled"
    )]
    pub shared_memory_enabled: bool,
    /// Whether SAP generated the constructor.
    #[serde(
        rename(deserialize = "@class:constructorGenerated"),
        alias = "constructorGenerated",
        default
    )]
    pub constructor_generated: bool,
    /// Whether SAP explicitly marks this class as having tests.
    #[serde(rename(deserialize = "@class:hasTests"), alias = "hasTests", default)]
    pub has_tests: bool,
    /// Atom links advertised for the class, in document order.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<ClassLink>,
    /// The package reference advertised for the class.
    #[serde(rename(deserialize = "adtcore:packageRef"), alias = "package")]
    pub package: ClassPackageReference,
    /// The syntax configuration advertised for the class.
    #[serde(
        rename(deserialize = "abapsource:syntaxConfiguration"),
        alias = "syntaxConfiguration"
    )]
    pub syntax_configuration: Option<ClassSyntaxConfiguration>,
    /// Interfaces implemented by the class.
    #[serde(
        rename(deserialize = "abapoo:interfaceRef"),
        alias = "interfaces",
        default
    )]
    pub interfaces: Vec<ClassObjectReference>,
    /// Source includes advertised for the class, in document order.
    #[serde(rename(deserialize = "class:include"), alias = "sources", default)]
    pub sources: Vec<ClassSourceProperties>,
    /// The direct superclass reference, when advertised.
    #[serde(rename(deserialize = "class:superClassRef"), alias = "superClass")]
    pub super_class: Option<ClassObjectReference>,
    /// The assigned message-class reference, when advertised.
    #[serde(rename(deserialize = "class:messageClassRef"), alias = "messageClass")]
    pub message_class: Option<ClassObjectReference>,
    /// The root-entity reference, when advertised.
    #[serde(rename(deserialize = "class:rootEntityRef"), alias = "rootEntity")]
    pub root_entity: Option<ClassObjectReference>,
}

/// The V2 class-properties media type uses the shared class payload.
pub type ClassPropertiesV2 = ClassProperties;

/// The V3 class-properties media type uses the shared class payload.
pub type ClassPropertiesV3 = ClassProperties;

/// The V4 class-properties media type uses the shared class payload.
pub type ClassPropertiesV4 = ClassProperties;

impl TryFrom<RawObjectProperties<Class>> for ClassProperties {
    type Error = ResponseError;

    fn try_from(raw: RawObjectProperties<Class>) -> Result<Self, Self::Error> {
        let properties: Self =
            serde_xml_rs::from_reader(raw.body.as_slice()).map_err(ObjectError::InvalidResponse)?;
        if properties.object_type != Class::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: Class::WORKBENCH_TYPE,
                actual: properties.object_type,
            }
            .into());
        }
        if !properties.name.eq_ignore_ascii_case(raw.resource.name()) {
            return Err(ObjectError::UnexpectedObjectName {
                expected: raw.resource.name().to_owned(),
                actual: properties.name,
            }
            .into());
        }
        Ok(properties)
    }
}

/// One raw Atom link advertised in a class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ClassLink {
    /// The target exactly as advertised by SAP.
    #[serde(rename(deserialize = "@href"), alias = "href")]
    pub href: String,
    /// The Atom relation URI.
    #[serde(rename(deserialize = "@rel"), alias = "relation")]
    pub relation: Option<String>,
    /// The target media type.
    #[serde(rename(deserialize = "@type"), alias = "mediaType")]
    pub media_type: Option<String>,
    /// The target language.
    #[serde(rename(deserialize = "@hreflang"), alias = "hreflang")]
    pub hreflang: Option<String>,
    /// A human-readable link title.
    #[serde(rename(deserialize = "@title"), alias = "title")]
    pub title: Option<String>,
    /// The target length exactly as advertised.
    #[serde(rename(deserialize = "@length"), alias = "length")]
    pub length: Option<String>,
    /// The target entity tag.
    #[serde(rename(deserialize = "@etag"), alias = "etag")]
    pub etag: Option<String>,
}

/// The package reference embedded in a class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ClassPackageReference {
    /// The package name.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,
    /// The package URI exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:uri"), alias = "uri")]
    pub uri: String,
    /// The package object type.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,
}

/// The syntax configuration embedded in a class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ClassSyntaxConfiguration {
    /// The configured ABAP language, when advertised.
    #[serde(rename(deserialize = "abapsource:language"), alias = "language")]
    pub language: Option<ClassSyntaxLanguage>,
}

/// An ABAP language description embedded in a class syntax configuration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ClassSyntaxLanguage {
    /// The language-version token.
    #[serde(rename(deserialize = "abapsource:version"), alias = "version")]
    pub version: String,
    /// The language description.
    #[serde(rename(deserialize = "abapsource:description"), alias = "description")]
    pub description: String,
    /// Atom links advertised for this syntax language.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<ClassLink>,
}

/// One source include embedded in a class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ClassSourceProperties {
    /// The include type exactly as advertised by SAP.
    #[serde(rename(deserialize = "@class:includeType"), alias = "includeType")]
    pub include_type: String,
    /// The source URI exactly as advertised by SAP.
    #[serde(rename(deserialize = "@abapsource:sourceUri"), alias = "sourceUri")]
    pub source_uri: String,
    /// The include name exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,
    /// The include object type.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which this source was last changed.
    #[serde(rename(deserialize = "@adtcore:changedAt"), alias = "lastChanged")]
    pub last_changed: String,
    /// The source version exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:version"), alias = "version")]
    pub version: String,
    /// The timestamp at which this source was created.
    #[serde(rename(deserialize = "@adtcore:createdAt"), alias = "createdAt")]
    pub created_at: String,
    /// The user who last changed this source.
    #[serde(rename(deserialize = "@adtcore:changedBy"), alias = "changedBy")]
    pub changed_by: String,
    /// The user who created this source.
    #[serde(rename(deserialize = "@adtcore:createdBy"), alias = "createdBy")]
    pub created_by: String,
    /// Atom links advertised for this source, in document order.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<ClassLink>,
}

/// An object reference embedded in a class-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ClassObjectReference {
    /// The object URI exactly as advertised by SAP.
    #[serde(rename(deserialize = "@adtcore:uri"), alias = "uri")]
    pub uri: Option<String>,
    /// The referenced object type.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: Option<String>,
    /// The referenced object name.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: Option<String>,
    /// The referenced package name.
    #[serde(rename(deserialize = "@adtcore:packageName"), alias = "packageName")]
    pub package_name: Option<String>,
    /// The referenced object description.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdtUri;

    const CLASS_XML: &str = include_str!("../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const LOCAL_TYPES_XML: &str = include_str!("../../tests/fixtures/class-cx-root-v4.xml");

    fn parse(body: &str) -> Result<ClassProperties, ResponseError> {
        ClassProperties::try_from(RawObjectProperties {
            resource: ObjectRef::<Class>::for_test(
                "CL_ADT_URI_MAPPER",
                AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            ),
            version: ClassPropertiesVersion::V4,
            body: body.as_bytes().to_vec(),
            etag: None,
        })
    }

    #[test]
    fn parses_the_complete_live_v4_payload_without_transforming_it() {
        let class = parse(CLASS_XML).unwrap();

        assert_eq!(class.name, "CL_ADT_URI_MAPPER");
        assert_eq!(class.object_type, Class::WORKBENCH_TYPE);
        assert_eq!(class.version, "active");
        assert_eq!(class.package.name, "SADT_TOOLS_CORE");
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
        let class = ClassProperties::try_from(RawObjectProperties {
            resource: ObjectRef::<Class>::for_test(
                "CX_ROOT",
                AdtUri::parse("/sap/bc/adt/oo/classes/cx_root").unwrap(),
            ),
            version: ClassPropertiesVersion::V4,
            body: LOCAL_TYPES_XML.as_bytes().to_vec(),
            etag: None,
        })
        .unwrap();

        assert!(class.is_abstract);
        assert!(class.constructor_generated);
        assert_eq!(class.sources.len(), 2);
        assert_eq!(class.sources[0].include_type, "localtypes");
        assert_eq!(class.sources[0].source_uri, "includes/localtypes");
    }

    #[test]
    fn parses_v2_without_the_v4_language_version() {
        let body = CLASS_XML.replace(" adtcore:abapLanguageVersion=\"X\"", "");
        let class = ClassProperties::try_from(RawObjectProperties {
            resource: ObjectRef::<Class>::for_test(
                "CL_ADT_URI_MAPPER",
                AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            ),
            version: ClassPropertiesVersion::V2,
            body: body.into_bytes(),
            etag: None,
        })
        .unwrap();

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
    fn friendly_json_round_trips_the_full_payload() {
        let class: ClassProperties = serde_xml_rs::from_str(LOCAL_TYPES_XML).unwrap();
        let json = serde_json::to_value(&class).unwrap();
        assert_eq!(json["name"], "CX_ROOT");
        assert_eq!(json["objectType"], "CLAS/OC");
        assert_eq!(json["sources"][1]["sourceUri"], "source/main");
        assert_eq!(
            json["links"][0]["mediaType"],
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
    fn validates_only_the_root_type_and_name() {
        let wrong_type =
            CLASS_XML.replacen("adtcore:type=\"CLAS/OC\"", "adtcore:type=\"PROG/P\"", 1);
        assert!(matches!(
            parse(&wrong_type),
            Err(ResponseError::Object(
                ObjectError::UnexpectedObjectType { .. }
            ))
        ));

        let wrong_name = CLASS_XML.replacen(
            "adtcore:name=\"CL_ADT_URI_MAPPER\"",
            "adtcore:name=\"OTHER_CLASS\"",
            1,
        );
        assert!(matches!(
            parse(&wrong_name),
            Err(ResponseError::Object(
                ObjectError::UnexpectedObjectName { .. }
            ))
        ));

        let nested_values = CLASS_XML
            .replacen(
                "adtcore:type=\"DEVC/K\"",
                "adtcore:type=\"FUTURE/PACKAGE\"",
                1,
            )
            .replace("adtcore:type=\"CLAS/I\"", "adtcore:type=\"FUTURE/INCLUDE\"")
            .replace("adtcore:version=\"active\"", "adtcore:version=\"future\"");
        let class = parse(&nested_values).unwrap();
        assert_eq!(class.version, "future");
        assert_eq!(class.package.object_type.as_str(), "FUTURE/PACKAGE");
        assert!(
            class
                .sources
                .iter()
                .all(|include| include.object_type.as_str() == "FUTURE/INCLUDE")
        );
    }
}
