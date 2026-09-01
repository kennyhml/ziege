use serde::{Deserialize, Serialize};
use zadt_macros::object_type;

use super::super::{
    AbapLanguageVersion, GlobalWorkbenchType, MediaTyped, ObjectRef, ToXml, WorkbenchVersion,
};
use crate::{AdvertisedLink, AdvertisedObjectReference};

#[object_type(
    properties = ProgramProperties,
    workbench_type = "PROG/P",
    collection(
        scheme = "http://www.sap.com/adt/categories/programs",
        term = "programs",
    ),
    capabilities(Source(properties.source_uri), Structure, Run)
)]
/// The ABAP program object type.
pub struct Program;

#[object_type(
    properties = IncludeProperties,
    workbench_type = "PROG/I",
    collection(
        scheme = "http://www.sap.com/adt/categories/programs",
        term = "includes",
    ),
    capabilities(Source(properties.source_uri))
)]
/// The standalone ABAP include object type.
pub struct Include;

impl ObjectRef<Program> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::new(name.to_ascii_uppercase(), uri)
    }
}

impl ObjectRef<Include> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::new(name.to_ascii_uppercase(), uri)
    }
}

/// The source parser configuration advertised by a program.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SyntaxConfiguration {
    /// The configured ABAP language.
    #[serde(rename = "abapsource:language")]
    pub language: SyntaxLanguage,
}

/// An ABAP language version, description, and its advertised parser links.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SyntaxLanguage {
    /// The ABAP language version.
    #[serde(rename = "abapsource:version")]
    pub version: AbapLanguageVersion,

    /// The server-provided language description.
    #[serde(rename = "abapsource:description")]
    pub description: String,

    /// Atom links nested in the language element.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

/// The currently modeled ABAP program-properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "program:abapProgram", deny_unknown_fields)]
pub struct ProgramProperties {
    /// The program name supplied by ADT.
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,

    /// The root repository object type, normally `PROG/P`.
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,

    /// The timestamp at which the program was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,

    /// The timestamp at which the program was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,

    /// The user who last changed the program.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The program description.
    #[serde(rename = "@adtcore:description")]
    pub description: String,

    /// The maximum length of the program description.
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,

    /// The program's logon language.
    #[serde(rename = "@adtcore:language")]
    pub language: String,

    /// Whether this program is locked by the current editor.
    #[serde(rename = "@program:lockedByEditor")]
    pub locked_by_editor: bool,

    /// The semantic program type, such as `executableProgram`.
    #[serde(rename = "@program:programType")]
    pub program_type: String,

    /// The source URI exactly as supplied by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the program.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The program's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The program's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The configured ABAP language version.
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: AbapLanguageVersion,

    /// The package reference exactly as embedded in the payload.
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,

    /// The source syntax configuration embedded in the payload.
    #[serde(rename = "abapsource:syntaxConfiguration")]
    pub syntax_configuration: SyntaxConfiguration,

    /// Atom links embedded at the payload root.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl MediaTyped for ProgramProperties {
    const MEDIA_TYPES: &'static [&'static str] = &[
        "application/vnd.sap.adt.programs.programs.v3+xml",
        "application/vnd.sap.adt.programs.programs.v2+xml",
    ];
}

impl ToXml for ProgramProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("program", "http://www.sap.com/adt/programs/programs"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

/// The complete standalone ABAP include-properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "include:abapInclude", deny_unknown_fields)]
pub struct IncludeProperties {
    /// The include name supplied by ADT.
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,

    /// The root repository object type, normally `PROG/I`.
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,

    /// The timestamp at which the include was last changed.
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,

    /// The object version.
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,

    /// The timestamp at which the include was created.
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,

    /// The user who last changed the include.
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,

    /// The include description.
    #[serde(rename = "@adtcore:description")]
    pub description: String,

    /// The maximum length of the include description.
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,

    /// The include's logon language.
    #[serde(rename = "@adtcore:language")]
    pub language: String,

    /// Number of objects reported as using this include.
    #[serde(rename = "@include:contextRefCount", default)]
    pub context_ref_count: u32,

    /// The source URI exactly as supplied by ADT.
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,

    /// Whether fixed-point arithmetic is enabled.
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,

    /// Whether the active Unicode check is enabled.
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,

    /// The user responsible for the include.
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,

    /// The include's master language.
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,

    /// The include's master system.
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,

    /// The package reference exactly as embedded in the payload.
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,

    /// The using object exactly as embedded in the payload.
    #[serde(rename = "include:contextRef")]
    pub context_ref: Option<AdvertisedObjectReference>,

    /// Atom links embedded at the payload root.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl MediaTyped for IncludeProperties {
    const MEDIA_TYPES: &'static [&'static str] =
        &["application/vnd.sap.adt.programs.includes.v2+xml"];
}

impl ToXml for IncludeProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("include", "http://www.sap.com/adt/programs/includes"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectType;

    const PROGRAM_XML: &str = include_str!("../../../tests/fixtures/program-z-test.xml");
    const INCLUDE_XML: &str = include_str!("../../../tests/fixtures/include-ztest.xml");

    fn parse_program(body: &str) -> Result<ProgramProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    fn parse_include(body: &str) -> Result<IncludeProperties, serde_xml_rs::Error> {
        serde_xml_rs::from_str(body)
    }

    #[test]
    fn parses_complete_program_wire_payload() {
        let program = parse_program(PROGRAM_XML).unwrap();

        assert_eq!(program.name, "Z_TEST");
        assert_eq!(program.version, WorkbenchVersion::Inactive);
        assert_eq!(
            program.abap_language_version,
            AbapLanguageVersion::StandardX
        );
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
        assert_eq!(include.version, WorkbenchVersion::Active);
        assert_eq!(include.source_uri, "source/main");
        assert_eq!(include.context_ref_count, 0);
        assert!(include.context_ref.is_none());
        assert_eq!(include.package.name.as_deref(), Some("$TMP"));
        assert_eq!(include.links.len(), 7);
    }

    #[test]
    fn program_json_uses_wire_keys_and_round_trips() {
        let program = parse_program(PROGRAM_XML).unwrap();
        let value = serde_json::to_value(&program).unwrap();

        assert_eq!(value["@adtcore:name"], "Z_TEST");
        assert_eq!(value["@adtcore:type"], "PROG/P");
        assert_eq!(value["@adtcore:version"], "inactive");
        assert_eq!(value["@abapsource:sourceUri"], "source/main");
        assert_eq!(value["adtcore:packageRef"]["@adtcore:name"], "$TMP");
        assert_eq!(
            value["abapsource:syntaxConfiguration"]["abapsource:language"]["abapsource:version"],
            "X"
        );
        let round_tripped: ProgramProperties = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped.name, program.name);
        assert_eq!(round_tripped.links.len(), program.links.len());
        assert_eq!(round_tripped.package.uri, program.package.uri);
    }

    #[test]
    fn serializes_program_properties_as_a_complete_update_payload() {
        let program = parse_program(PROGRAM_XML).unwrap();
        let reference = ObjectRef::<Program>::new(
            program.name.clone(),
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
        )
        .erase();
        let properties = Program::DESCRIPTOR
            .properties_from_json(&reference, serde_json::to_value(&program).unwrap())
            .unwrap();
        let xml = String::from_utf8(
            Program::DESCRIPTOR
                .properties_to_xml(&reference, &properties)
                .unwrap(),
        )
        .unwrap();

        assert!(xml.contains("<program:abapProgram"));
        assert!(xml.contains("xmlns:program=\"http://www.sap.com/adt/programs/programs\""));
        assert!(xml.contains("xmlns:abapsource=\"http://www.sap.com/adt/abapsource\""));
        assert!(xml.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
        assert!(xml.contains("xmlns:atom=\"http://www.w3.org/2005/Atom\""));
        assert!(xml.contains("adtcore:name=\"Z_TEST\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse_program(&xml).unwrap(), program);
    }

    #[test]
    fn serializes_include_properties_as_a_complete_update_payload() {
        let include = parse_include(INCLUDE_XML).unwrap();
        let reference = ObjectRef::<Include>::new(
            include.name.clone(),
            crate::AdtUri::parse("/sap/bc/adt/programs/includes/ztest").unwrap(),
        )
        .erase();
        let properties = Include::DESCRIPTOR
            .properties_from_json(&reference, serde_json::to_value(&include).unwrap())
            .unwrap();
        let xml = String::from_utf8(
            Include::DESCRIPTOR
                .properties_to_xml(&reference, &properties)
                .unwrap(),
        )
        .unwrap();

        assert!(xml.contains("<include:abapInclude"));
        assert!(xml.contains("xmlns:include=\"http://www.sap.com/adt/programs/includes\""));
        assert!(xml.contains("xmlns:abapsource=\"http://www.sap.com/adt/abapsource\""));
        assert!(xml.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
        assert!(xml.contains("xmlns:atom=\"http://www.w3.org/2005/Atom\""));
        assert!(xml.contains("adtcore:name=\"ZTEST\""));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert_eq!(parse_include(&xml).unwrap(), include);
    }

    #[test]
    fn include_json_with_context_reference_round_trips() {
        let mut value = serde_json::to_value(parse_include(INCLUDE_XML).unwrap()).unwrap();
        assert_eq!(value["@adtcore:name"], "ZTEST");
        assert_eq!(value["@abapsource:sourceUri"], "source/main");
        assert_eq!(value["adtcore:packageRef"]["@adtcore:name"], "$TMP");
        value["include:contextRef"] = serde_json::json!({
            "@adtcore:uri": "/sap/bc/adt/programs/programs/Z_CONTEXT",
            "@adtcore:type": "PROG/P",
            "@adtcore:name": "Z_CONTEXT",
            "@adtcore:description": "Context program"
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
        let value = serde_json::to_value(&include).unwrap();
        assert_eq!(value["include:contextRef"]["@adtcore:name"], "Z_CONTEXT");
        let round_tripped: IncludeProperties = serde_json::from_value(value).unwrap();
        let context = round_tripped.context_ref.unwrap();
        assert_eq!(
            context.uri.as_deref(),
            Some("/sap/bc/adt/programs/programs/Z_CONTEXT")
        );
        assert_eq!(context.description.as_deref(), Some("Context program"));
    }

    #[test]
    fn preserves_unresolved_links_and_object_values() {
        let invalid_href = "https://attacker.example/source";
        let body = PROGRAM_XML
            .replace("adtcore:type=\"DEVC/K\"", "adtcore:type=\"FUTURE/PACKAGE\"")
            .replace(
                "adtcore:uri=\"/sap/bc/adt/packages/%24tmp\"",
                "adtcore:uri=\"https://example.test/package\"",
            )
            .replace("source/main/versions", invalid_href);
        let program = parse_program(&body).unwrap();

        assert_eq!(program.version, WorkbenchVersion::Inactive);
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
    fn rejects_unknown_object_versions() {
        let body = PROGRAM_XML.replace("adtcore:version=\"inactive\"", "adtcore:version=\"dirty\"");

        assert!(parse_program(&body).is_err());
    }

    #[test]
    fn rejects_malformed_program_xml() {
        assert!(parse_program("<program:abapProgram>").is_err());
        assert!(
            parse_program(&PROGRAM_XML.replacen(
                "program:lockedByEditor=",
                "program:futureAttribute=\"future\" program:lockedByEditor=",
                1,
            ))
            .is_err()
        );
    }

    #[test]
    fn preserves_advertised_root_identity() {
        let program = parse_program(
            &PROGRAM_XML.replace("adtcore:type=\"PROG/P\"", "adtcore:type=\"PROG/I\""),
        )
        .unwrap();
        assert_eq!(program.object_type, Include::WORKBENCH_TYPE);

        let include = parse_include(
            &INCLUDE_XML.replace("adtcore:name=\"ZTEST\"", "adtcore:name=\"ZOTHER\""),
        )
        .unwrap();
        assert_eq!(include.name, "ZOTHER");
    }
}
