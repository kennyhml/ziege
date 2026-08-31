use serde::{Deserialize, Serialize};
use zadt_macros::object_type;

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, SyntaxConfiguration, ToXml, WorkbenchVersion,
};

#[object_type(
    properties = FunctionGroupProperties,
    workbench_type = "FUGR/F",
    collection(
        scheme = "http://www.sap.com/adt/categories/functions",
        term = "groups",
    ),
    subobjects(
        FunctionModule(
            relation = "http://www.sap.com/adt/categories/functiongroups/functionmodules",
            parent_variable = "groupname",
        ),
        FunctionGroupInclude(
            relation = "http://www.sap.com/adt/categories/functiongroups/includes",
            parent_variable = "groupname",
        ),
    ),
    capabilities(Source(properties.source_uri), Structure)
)]
/// An ABAP function group.
pub struct FunctionGroup;

#[object_type(
    properties = FunctionModuleProperties,
    workbench_type = "FUGR/FF",
    subobject,
    capabilities(Source(properties.source_uri))
)]
/// A function module owned by an ABAP function group.
pub struct FunctionModule;

#[object_type(
    properties = FunctionGroupIncludeProperties,
    workbench_type = "FUGR/I",
    subobject,
    capabilities(Source(properties.source_uri))
)]
/// A source include owned by an ABAP function group.
pub struct FunctionGroupInclude;

/// The complete function-group properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "group:abapFunctionGroup", deny_unknown_fields)]
pub struct FunctionGroupProperties {
    #[serde(rename = "@group:lockedByEditor")]
    pub locked_by_editor: bool,
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    pub fix_point_arithmetic: bool,
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    pub unicode_check_active: bool,
    #[serde(rename = "@adtcore:responsible")]
    pub responsible: String,
    #[serde(rename = "@adtcore:masterLanguage")]
    pub master_language: String,
    #[serde(rename = "@adtcore:masterSystem")]
    pub master_system: String,
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: Option<AbapLanguageVersion>,
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    #[serde(rename = "@adtcore:version")]
    pub version: WorkbenchVersion,
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,
    #[serde(rename = "@adtcore:description")]
    pub description: String,
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
    #[serde(rename = "abapsource:syntaxConfiguration")]
    pub syntax_configuration: SyntaxConfiguration,
}

impl MediaTyped for FunctionGroupProperties {
    const MEDIA_TYPES: &'static [&'static str] = &[
        "application/vnd.sap.adt.functions.groups.v3+xml",
        "application/vnd.sap.adt.functions.groups.v2+xml",
    ];
}

impl ToXml for FunctionGroupProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("group", "http://www.sap.com/adt/functions/groups"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

/// The complete function-module properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "fmodule:abapFunctionModule", deny_unknown_fields)]
pub struct FunctionModuleProperties {
    #[serde(rename = "@fmodule:releaseState")]
    pub release_state: String,
    #[serde(rename = "@fmodule:processingType")]
    pub processing_type: String,
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    #[serde(rename = "@adtcore:version")]
    pub version: WorkbenchVersion,
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    #[serde(rename = "@adtcore:description")]
    pub description: String,
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    #[serde(rename = "adtcore:containerRef")]
    pub container: AdvertisedObjectReference,
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl MediaTyped for FunctionModuleProperties {
    const MEDIA_TYPES: &'static [&'static str] =
        &["application/vnd.sap.adt.functions.fmodules.v3+xml"];
}

impl ToXml for FunctionModuleProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("fmodule", "http://www.sap.com/adt/functions/fmodules"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

/// The complete function-group include properties payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename = "finclude:abapFunctionGroupInclude", deny_unknown_fields)]
pub struct FunctionGroupIncludeProperties {
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,
    #[serde(rename = "@adtcore:name")]
    pub name: String,
    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    #[serde(rename = "@adtcore:version")]
    pub version: WorkbenchVersion,
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    #[serde(rename = "@adtcore:description", default)]
    pub description: Option<String>,
    #[serde(rename = "@adtcore:descriptionTextLimit", default)]
    pub description_text_limit: Option<u32>,
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    #[serde(rename = "adtcore:containerRef")]
    pub container: AdvertisedObjectReference,
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl MediaTyped for FunctionGroupIncludeProperties {
    const MEDIA_TYPES: &'static [&'static str] =
        &["application/vnd.sap.adt.functions.fincludes.v2+xml"];
}

impl ToXml for FunctionGroupIncludeProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("finclude", "http://www.sap.com/adt/functions/fincludes"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUP_XML: &str = include_str!("../../../tests/fixtures/function-group-z-test-group.xml");
    const GROUP_V2_XML: &str =
        include_str!("../../../tests/fixtures/function-group-z-test-group-v2.xml");
    const MODULE_XML: &str = include_str!("../../../tests/fixtures/function-module-zzzzfunc.xml");
    const INCLUDE_XML: &str =
        include_str!("../../../tests/fixtures/function-group-include-lz-test-grouptop.xml");

    #[test]
    fn parses_live_function_group_properties() {
        let properties: FunctionGroupProperties = serde_xml_rs::from_str(GROUP_XML).unwrap();

        assert_eq!(properties.name, "Z_TEST_GROUP");
        assert_eq!(properties.package.name.as_deref(), Some("$TMP"));
        assert_eq!(
            properties.syntax_configuration.language.version.as_str(),
            "X"
        );
        assert_eq!(properties.links.len(), 9);
    }

    #[test]
    fn parses_live_v2_function_group_without_direct_language_version() {
        let properties: FunctionGroupProperties = serde_xml_rs::from_str(GROUP_V2_XML).unwrap();

        assert_eq!(properties.name, "Z_TEST_GROUP");
        assert!(properties.abap_language_version.is_none());
        assert_eq!(
            properties.syntax_configuration.language.version.as_str(),
            "X"
        );
    }

    #[test]
    fn parses_live_function_module_properties() {
        let properties: FunctionModuleProperties = serde_xml_rs::from_str(MODULE_XML).unwrap();

        assert_eq!(properties.name, "ZZZZFUNC");
        assert_eq!(properties.release_state, "notReleased");
        assert_eq!(properties.container.name.as_deref(), Some("Z_TEST_GROUP"));
        assert_eq!(properties.links.len(), 8);
    }

    #[test]
    fn parses_live_function_group_include_properties() {
        let properties: FunctionGroupIncludeProperties =
            serde_xml_rs::from_str(INCLUDE_XML).unwrap();

        assert_eq!(properties.name, "LZ_TEST_GROUPTOP");
        assert_eq!(properties.container.name.as_deref(), Some("Z_TEST_GROUP"));
        assert_eq!(properties.links.len(), 6);
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let group: FunctionGroupProperties = serde_xml_rs::from_str(GROUP_XML).unwrap();
        let group_xml = String::from_utf8(group.to_xml().unwrap()).unwrap();
        assert!(group_xml.contains("<group:abapFunctionGroup"));
        assert!(group_xml.contains("adtcore:name=\"Z_TEST_GROUP\""));
        assert!(group_xml.contains("<adtcore:packageRef"));
        assert_eq!(
            serde_xml_rs::from_str::<FunctionGroupProperties>(&group_xml).unwrap(),
            group
        );

        let module: FunctionModuleProperties = serde_xml_rs::from_str(MODULE_XML).unwrap();
        let module_xml = String::from_utf8(module.to_xml().unwrap()).unwrap();
        assert!(module_xml.contains("<fmodule:abapFunctionModule"));
        assert!(module_xml.contains("adtcore:name=\"ZZZZFUNC\""));
        assert!(module_xml.contains("<adtcore:containerRef"));
        assert_eq!(
            serde_xml_rs::from_str::<FunctionModuleProperties>(&module_xml).unwrap(),
            module
        );

        let include: FunctionGroupIncludeProperties = serde_xml_rs::from_str(INCLUDE_XML).unwrap();
        let include_xml = String::from_utf8(include.to_xml().unwrap()).unwrap();
        assert!(include_xml.contains("<finclude:abapFunctionGroupInclude"));
        assert!(include_xml.contains("adtcore:name=\"LZ_TEST_GROUPTOP\""));
        assert!(include_xml.contains("<adtcore:containerRef"));
        assert_eq!(
            serde_xml_rs::from_str::<FunctionGroupIncludeProperties>(&include_xml).unwrap(),
            include
        );
    }
}
