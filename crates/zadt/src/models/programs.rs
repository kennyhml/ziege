use serde::{Deserialize, Serialize};

use crate::{
    GlobalWorkbenchType, Include, MediaVersionNegotiation, ObjectError, ObjectRef, ObjectType,
    Program, RawObjectProperties, ResponseError,
};

/// The SAP media-type version used to decode program properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProgramPropertiesVersion {
    media_type: &'static str,
}

impl MediaVersionNegotiation for ProgramPropertiesVersion {
    const SUPPORTED: &'static [Self] = &[Self::V3, Self::V2];

    fn media_type(self) -> &'static str {
        self.media_type
    }
}

impl ProgramPropertiesVersion {
    pub const V2: Self = Self {
        media_type: "application/vnd.sap.adt.programs.programs.v2+xml",
    };

    pub const V3: Self = Self {
        media_type: "application/vnd.sap.adt.programs.programs.v3+xml",
    };
}

/// The SAP media-type version used to decode include properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IncludePropertyVersion {
    V2,
}

impl MediaVersionNegotiation for IncludePropertyVersion {
    const SUPPORTED: &'static [Self] = &[Self::V2];

    fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.programs.includes.v2+xml",
        }
    }
}

/// The plain-text console output produced by running an ABAP program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRunResult {
    /// The program that was executed.
    pub reference: ObjectRef<Program>,

    /// The rendered program output returned by SAP.
    pub content: String,
}

impl ProgramRunResult {
    pub(crate) fn new(reference: ObjectRef<Program>, content: String) -> Self {
        Self { reference, content }
    }
}

/// An unmodified object reference embedded in a properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct PropertyObjectReference {
    /// The URI exactly as supplied by ADT.
    #[serde(rename(deserialize = "@adtcore:uri"), alias = "uri")]
    pub uri: Option<String>,

    /// The global Workbench type supplied by ADT.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: Option<GlobalWorkbenchType>,

    /// The referenced object's name.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: Option<String>,

    /// The referenced object's description, when advertised.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: Option<String>,
}

/// An unmodified Atom link embedded in a properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct PropertyLink {
    /// The target exactly as advertised by ADT.
    #[serde(rename(deserialize = "@href"), alias = "href")]
    pub href: String,

    /// The Atom relation URI, when advertised.
    #[serde(rename(deserialize = "@rel"), alias = "relation")]
    pub relation: Option<String>,

    /// The target representation's media type, when advertised.
    #[serde(rename(deserialize = "@type"), alias = "mediaType")]
    pub media_type: Option<String>,

    /// The target representation's language, when advertised.
    #[serde(rename(deserialize = "@hreflang"), alias = "hreflang")]
    pub hreflang: Option<String>,

    /// The link title, when advertised.
    #[serde(rename(deserialize = "@title"), alias = "title")]
    pub title: Option<String>,

    /// The target length exactly as advertised by ADT.
    #[serde(rename(deserialize = "@length"), alias = "length")]
    pub length: Option<String>,

    /// The target representation's entity tag, when advertised.
    #[serde(rename(deserialize = "@etag"), alias = "etag")]
    pub etag: Option<String>,
}

/// The source parser configuration advertised by a program.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct SyntaxConfiguration {
    /// The configured ABAP language.
    #[serde(rename(deserialize = "abapsource:language"), alias = "language")]
    pub language: SyntaxLanguage,
}

/// An ABAP language version, description, and its advertised parser links.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct SyntaxLanguage {
    /// The language version identifier, such as `X`.
    #[serde(rename(deserialize = "abapsource:version"), alias = "version")]
    pub version: String,

    /// The server-provided language description.
    #[serde(rename(deserialize = "abapsource:description"), alias = "description")]
    pub description: String,

    /// Atom links nested in the language element.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<PropertyLink>,
}

/// The complete ABAP program-properties payload shared by V2 and V3.
#[derive(Debug, Deserialize, Serialize)]
#[serde(
    rename(deserialize = "program:abapProgram"),
    rename_all(serialize = "camelCase")
)]
pub struct ProgramProperties {
    /// The program name supplied by ADT.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,

    /// The root repository object type, normally `PROG/P`.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,

    /// The timestamp at which the program was last changed.
    #[serde(rename(deserialize = "@adtcore:changedAt"), alias = "lastChanged")]
    pub last_changed: String,

    /// The object version exactly as supplied by ADT.
    #[serde(rename(deserialize = "@adtcore:version"), alias = "version")]
    pub version: String,

    /// The timestamp at which the program was created.
    #[serde(rename(deserialize = "@adtcore:createdAt"), alias = "createdAt")]
    pub created_at: String,

    /// The user who last changed the program.
    #[serde(rename(deserialize = "@adtcore:changedBy"), alias = "changedBy")]
    pub changed_by: String,

    /// The program description.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: String,

    /// The maximum length of the program description.
    #[serde(
        rename(deserialize = "@adtcore:descriptionTextLimit"),
        alias = "descriptionTextLimit"
    )]
    pub description_text_limit: u32,

    /// The program's logon language.
    #[serde(rename(deserialize = "@adtcore:language"), alias = "language")]
    pub language: String,

    /// Whether this program is locked by the current editor.
    #[serde(
        rename(deserialize = "@program:lockedByEditor"),
        alias = "lockedByEditor"
    )]
    pub locked_by_editor: bool,

    /// The semantic program type, such as `executableProgram`.
    #[serde(rename(deserialize = "@program:programType"), alias = "programType")]
    pub program_type: String,

    /// The source URI exactly as supplied by ADT.
    #[serde(rename(deserialize = "@abapsource:sourceUri"), alias = "sourceUri")]
    pub source_uri: String,

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

    /// The user responsible for the program.
    #[serde(rename(deserialize = "@adtcore:responsible"), alias = "responsible")]
    pub responsible: String,

    /// The program's master language.
    #[serde(
        rename(deserialize = "@adtcore:masterLanguage"),
        alias = "masterLanguage"
    )]
    pub master_language: String,

    /// The program's master system.
    #[serde(rename(deserialize = "@adtcore:masterSystem"), alias = "masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[serde(
        rename(deserialize = "@adtcore:abapLanguageVersion"),
        alias = "abapLanguageVersion"
    )]
    pub abap_language_version: String,

    /// The package reference exactly as embedded in the payload.
    #[serde(rename(deserialize = "adtcore:packageRef"), alias = "package")]
    pub package: PropertyObjectReference,

    /// The source syntax configuration embedded in the payload.
    #[serde(
        rename(deserialize = "abapsource:syntaxConfiguration"),
        alias = "syntaxConfiguration"
    )]
    pub syntax_configuration: SyntaxConfiguration,

    /// Atom links embedded at the payload root.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<PropertyLink>,
}

/// The V2 program-properties media type uses the shared payload.
pub type ProgramPropertiesV2 = ProgramProperties;

/// The V3 program-properties media type uses the shared payload.
pub type ProgramPropertiesV3 = ProgramProperties;

impl TryFrom<RawObjectProperties<Program>> for ProgramProperties {
    type Error = ResponseError;

    fn try_from(raw: RawObjectProperties<Program>) -> Result<Self, Self::Error> {
        let properties: Self =
            serde_xml_rs::from_reader(raw.body.as_slice()).map_err(ObjectError::InvalidResponse)?;
        if properties.object_type != Program::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: Program::WORKBENCH_TYPE,
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

/// The complete standalone ABAP include-properties payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(
    rename(deserialize = "include:abapInclude"),
    rename_all(serialize = "camelCase")
)]
pub struct IncludeProperties {
    /// The include name supplied by ADT.
    #[serde(rename(deserialize = "@adtcore:name"), alias = "name")]
    pub name: String,

    /// The root repository object type, normally `PROG/I`.
    #[serde(rename(deserialize = "@adtcore:type"), alias = "objectType")]
    pub object_type: GlobalWorkbenchType,

    /// The timestamp at which the include was last changed.
    #[serde(rename(deserialize = "@adtcore:changedAt"), alias = "lastChanged")]
    pub last_changed: String,

    /// The object version exactly as supplied by ADT.
    #[serde(rename(deserialize = "@adtcore:version"), alias = "version")]
    pub version: String,

    /// The timestamp at which the include was created.
    #[serde(rename(deserialize = "@adtcore:createdAt"), alias = "createdAt")]
    pub created_at: String,

    /// The user who last changed the include.
    #[serde(rename(deserialize = "@adtcore:changedBy"), alias = "changedBy")]
    pub changed_by: String,

    /// The include description.
    #[serde(rename(deserialize = "@adtcore:description"), alias = "description")]
    pub description: String,

    /// The maximum length of the include description.
    #[serde(
        rename(deserialize = "@adtcore:descriptionTextLimit"),
        alias = "descriptionTextLimit"
    )]
    pub description_text_limit: u32,

    /// The include's logon language.
    #[serde(rename(deserialize = "@adtcore:language"), alias = "language")]
    pub language: String,

    /// Number of objects reported as using this include.
    #[serde(
        rename(deserialize = "@include:contextRefCount"),
        alias = "contextRefCount",
        default
    )]
    pub context_ref_count: u32,

    /// The source URI exactly as supplied by ADT.
    #[serde(rename(deserialize = "@abapsource:sourceUri"), alias = "sourceUri")]
    pub source_uri: String,

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

    /// The user responsible for the include.
    #[serde(rename(deserialize = "@adtcore:responsible"), alias = "responsible")]
    pub responsible: String,

    /// The include's master language.
    #[serde(
        rename(deserialize = "@adtcore:masterLanguage"),
        alias = "masterLanguage"
    )]
    pub master_language: String,

    /// The include's master system.
    #[serde(rename(deserialize = "@adtcore:masterSystem"), alias = "masterSystem")]
    pub master_system: String,

    /// The package reference exactly as embedded in the payload.
    #[serde(rename(deserialize = "adtcore:packageRef"), alias = "package")]
    pub package: PropertyObjectReference,

    /// The using object exactly as embedded in the payload.
    #[serde(rename(deserialize = "include:contextRef"), alias = "contextRef")]
    pub context_ref: Option<PropertyObjectReference>,

    /// Atom links embedded at the payload root.
    #[serde(rename(deserialize = "atom:link"), alias = "links", default)]
    pub links: Vec<PropertyLink>,
}

/// The V2 include-properties media type uses the shared payload.
pub type IncludePropertiesV2 = IncludeProperties;

impl TryFrom<RawObjectProperties<Include>> for IncludeProperties {
    type Error = ResponseError;

    fn try_from(raw: RawObjectProperties<Include>) -> Result<Self, Self::Error> {
        let properties: Self =
            serde_xml_rs::from_reader(raw.body.as_slice()).map_err(ObjectError::InvalidResponse)?;
        if properties.object_type != Include::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: Include::WORKBENCH_TYPE,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityTag;

    const PROGRAM_XML: &str = include_str!("../../tests/fixtures/program-z-test.xml");
    const INCLUDE_XML: &str = include_str!("../../tests/fixtures/include-ztest.xml");

    fn parse_program(body: &str) -> Result<ProgramProperties, ResponseError> {
        ProgramProperties::try_from(RawObjectProperties {
            resource: ObjectRef::<Program>::for_test(
                "Z_TEST",
                crate::AdtUri::parse("/sap/bc/adt/programs/programs/Z_TEST").unwrap(),
            ),
            version: ProgramPropertiesVersion::V3,
            body: body.as_bytes().to_vec(),
            etag: Some(EntityTag::from_static("program-etag")),
        })
    }

    fn parse_include(body: &str) -> Result<IncludeProperties, ResponseError> {
        IncludeProperties::try_from(RawObjectProperties {
            resource: ObjectRef::<Include>::for_test(
                "ZTEST",
                crate::AdtUri::parse("/sap/bc/adt/programs/includes/ZTEST").unwrap(),
            ),
            version: IncludePropertyVersion::V2,
            body: body.as_bytes().to_vec(),
            etag: Some(EntityTag::from_static("include-etag")),
        })
    }

    #[test]
    fn parses_complete_program_wire_payload() {
        let program = parse_program(PROGRAM_XML).unwrap();

        assert_eq!(program.name, "Z_TEST");
        assert_eq!(program.version, "inactive");
        assert_eq!(program.source_uri, "source/main");
        assert_eq!(program.package.name.as_deref(), Some("$TMP"));
        assert_eq!(program.links.len(), 9);
        assert_eq!(program.syntax_configuration.language.links.len(), 1);
        assert_eq!(
            program.syntax_configuration.language.links[0]
                .etag
                .as_deref(),
            Some("757")
        );
    }

    #[test]
    fn parses_complete_include_wire_payload() {
        let include = parse_include(INCLUDE_XML).unwrap();

        assert_eq!(include.name, "ZTEST");
        assert_eq!(include.version, "active");
        assert_eq!(include.source_uri, "source/main");
        assert_eq!(include.context_ref_count, 0);
        assert!(include.context_ref.is_none());
        assert_eq!(include.package.name.as_deref(), Some("$TMP"));
        assert_eq!(include.links.len(), 7);
    }

    #[test]
    fn program_json_uses_friendly_keys_and_round_trips() {
        let program = parse_program(PROGRAM_XML).unwrap();
        let value = serde_json::to_value(&program).unwrap();

        assert_eq!(value["name"], "Z_TEST");
        assert_eq!(value["objectType"], "PROG/P");
        assert_eq!(value["version"], "inactive");
        assert_eq!(value["sourceUri"], "source/main");
        assert!(value.get("@adtcore:name").is_none());
        let round_tripped: ProgramProperties = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped.name, program.name);
        assert_eq!(round_tripped.links.len(), program.links.len());
        assert_eq!(round_tripped.package.uri, program.package.uri);
    }

    #[test]
    fn include_json_with_context_reference_round_trips() {
        let mut value = serde_json::to_value(parse_include(INCLUDE_XML).unwrap()).unwrap();
        value["contextRef"] = serde_json::json!({
            "uri": "/sap/bc/adt/programs/programs/Z_CONTEXT",
            "objectType": "PROG/P",
            "name": "Z_CONTEXT",
            "description": "Context program"
        });

        let include: IncludeProperties = serde_json::from_value(value).unwrap();
        let context = include.context_ref.as_ref().unwrap();
        assert_eq!(
            context.uri.as_deref(),
            Some("/sap/bc/adt/programs/programs/Z_CONTEXT")
        );
        assert_eq!(context.object_type.as_ref(), Some(&Program::WORKBENCH_TYPE));
        assert_eq!(context.name.as_deref(), Some("Z_CONTEXT"));
        assert_eq!(context.description.as_deref(), Some("Context program"));
        let round_tripped: IncludeProperties =
            serde_json::from_value(serde_json::to_value(&include).unwrap()).unwrap();
        let context = round_tripped.context_ref.unwrap();
        assert_eq!(
            context.uri.as_deref(),
            Some("/sap/bc/adt/programs/programs/Z_CONTEXT")
        );
        assert_eq!(context.description.as_deref(), Some("Context program"));
    }

    #[test]
    fn preserves_unparsed_object_version_and_unresolved_links() {
        let invalid_href = "https://attacker.example/source";
        let body = PROGRAM_XML
            .replace("adtcore:version=\"inactive\"", "adtcore:version=\"dirty\"")
            .replace("adtcore:type=\"DEVC/K\"", "adtcore:type=\"FUTURE/PACKAGE\"")
            .replace(
                "adtcore:uri=\"/sap/bc/adt/packages/%24tmp\"",
                "adtcore:uri=\"https://example.test/package\"",
            )
            .replace("source/main/versions", invalid_href);
        let program = parse_program(&body).unwrap();

        assert_eq!(program.version, "dirty");
        assert_eq!(
            program.package.object_type.unwrap().as_str(),
            "FUTURE/PACKAGE"
        );
        assert_eq!(
            program.package.uri.as_deref(),
            Some("https://example.test/package")
        );
        assert_eq!(program.links[0].href, invalid_href);
    }

    #[test]
    fn rejects_malformed_program_xml() {
        assert!(matches!(
            parse_program("<program:abapProgram>"),
            Err(ResponseError::Object(ObjectError::InvalidResponse(_)))
        ));
    }

    #[test]
    fn rejects_unexpected_root_object_type() {
        let body = PROGRAM_XML.replace("adtcore:type=\"PROG/P\"", "adtcore:type=\"PROG/I\"");

        assert!(matches!(
            parse_program(&body),
            Err(ResponseError::Object(ObjectError::UnexpectedObjectType {
                expected,
                actual,
            })) if expected == Program::WORKBENCH_TYPE && actual == Include::WORKBENCH_TYPE
        ));
    }

    #[test]
    fn rejects_unexpected_root_object_name() {
        let body = INCLUDE_XML.replace("adtcore:name=\"ZTEST\"", "adtcore:name=\"ZOTHER\"");

        assert!(matches!(
            parse_include(&body),
            Err(ResponseError::Object(ObjectError::UnexpectedObjectName {
                expected,
                actual,
            })) if expected == "ZTEST" && actual == "ZOTHER"
        ));
    }
}
