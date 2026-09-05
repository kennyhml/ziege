use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTyped, MediaTypes, SyntaxConfiguration, ToXml, WorkbenchVersion,
};

/// An ABAP function group.
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
    capabilities(
        Create(FunctionGroupCreateProperties),
        Source(properties.source_uri),
        Structure,
    )
)]
pub struct FunctionGroup;

/// A function module owned by an ABAP function group.
#[object_type(
    properties = FunctionModuleProperties,
    workbench_type = "FUGR/FF",
    subobject,
    capabilities(
        Create(FunctionModuleCreateProperties),
        Source(properties.source_uri),
    )
)]
pub struct FunctionModule;

/// A source include owned by an ABAP function group.
#[object_type(
    properties = FunctionGroupIncludeProperties,
    workbench_type = "FUGR/I",
    subobject,
    capabilities(
        Create(FunctionGroupIncludeCreateProperties),
        Source(properties.source_uri),
    )
)]
pub struct FunctionGroupInclude;

/// The complete function-group properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = FunctionGroupCreateProperties,
    doc = "The sparse payload used to create an ABAP function group."
)]
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
    #[for_create(
        optional,
        doc = "The requested ABAP language version, or the package default when omitted."
    )]
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    pub abap_language_version: Option<AbapLanguageVersion>,
    #[for_create(identity, default, doc = "The function-group name.")]
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,
    #[for_create(
        identity,
        default = <FunctionGroup as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The function group's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    #[serde(rename = "@adtcore:createdBy")]
    pub created_by: String,
    #[for_create(doc = "The description, limited by SAP to 40 characters.")]
    #[serde(rename = "@adtcore:description")]
    pub description: String,
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
    #[for_create(doc = "The package receiving the function group.")]
    #[serde(rename = "adtcore:packageRef")]
    pub package: AdvertisedObjectReference,
    #[serde(rename = "abapsource:syntaxConfiguration")]
    pub syntax_configuration: SyntaxConfiguration,
}

impl MediaTyped for FunctionGroupProperties {
    const MEDIA_TYPES: MediaTypes = MediaTypes::new(&[
        "application/vnd.sap.adt.functions.groups.v3+xml",
        "application/vnd.sap.adt.functions.groups.v2+xml",
    ]);
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
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = FunctionModuleCreateProperties,
    doc = "The sparse payload used to create an ABAP function module."
)]
#[serde(rename = "fmodule:abapFunctionModule", deny_unknown_fields)]
pub struct FunctionModuleProperties {
    #[serde(rename = "@fmodule:releaseState")]
    pub release_state: String,
    #[serde(rename = "@fmodule:processingType")]
    pub processing_type: String,
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,
    #[for_create(identity, default, doc = "The function-module name.")]
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,
    #[for_create(
        identity,
        default = <FunctionModule as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The function module's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    #[for_create(doc = "The function-module description.")]
    #[serde(rename = "@adtcore:description")]
    pub description: String,
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    pub description_text_limit: u32,
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    #[for_create(parent, doc = "The function group containing this module.")]
    #[serde(rename = "adtcore:containerRef")]
    pub container: AdvertisedObjectReference,
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl MediaTyped for FunctionModuleProperties {
    const MEDIA_TYPES: MediaTypes =
        MediaTypes::new(&["application/vnd.sap.adt.functions.fmodules.v3+xml"]);
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
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = FunctionGroupIncludeCreateProperties,
    doc = "The sparse payload used to create an ABAP function-group include."
)]
#[serde(rename = "finclude:abapFunctionGroupInclude", deny_unknown_fields)]
pub struct FunctionGroupIncludeProperties {
    #[serde(rename = "@abapsource:sourceUri")]
    pub source_uri: String,
    #[for_create(identity, default, doc = "The function-group include name.")]
    #[serde(rename = "@adtcore:name")]
    pub(crate) name: String,
    #[for_create(
        identity,
        default = <FunctionGroupInclude as crate::ObjectType>::WORKBENCH_TYPE,
        doc = "The include's global Workbench type."
    )]
    #[serde(rename = "@adtcore:type")]
    pub(crate) object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    pub last_changed: String,
    #[serde(rename = "@adtcore:version")]
    pub(crate) version: WorkbenchVersion,
    #[serde(rename = "@adtcore:createdAt")]
    pub created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    pub changed_by: String,
    #[for_create(optional, doc = "The include description.")]
    #[serde(rename = "@adtcore:description", default)]
    pub description: Option<String>,
    #[serde(rename = "@adtcore:descriptionTextLimit", default)]
    pub description_text_limit: Option<u32>,
    #[serde(rename = "@adtcore:language")]
    pub language: String,
    #[for_create(parent, doc = "The function group containing this include.")]
    #[serde(rename = "adtcore:containerRef")]
    pub container: AdvertisedObjectReference,
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

impl MediaTyped for FunctionGroupIncludeProperties {
    const MEDIA_TYPES: MediaTypes =
        MediaTypes::new(&["application/vnd.sap.adt.functions.fincludes.v2+xml"]);
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
    use crate::{AdtUri, AssignObjectIdentity, ObjectKey, ObjectRef};

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
    fn builds_sparse_function_group_creation_properties() {
        let mut properties = FunctionGroupCreateProperties::builder()
            .description("Created function group")
            .package("$TMP")
            .abap_language_version(AbapLanguageVersion::CloudDevelopment)
            .build()
            .unwrap();
        let reference = ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP");
        properties.assign_reference(&ObjectRef::for_test(
            reference,
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group").unwrap(),
            None,
        ));

        let body = String::from_utf8(properties.to_xml().unwrap()).unwrap();
        assert!(body.contains("<group:abapFunctionGroup"));
        assert!(body.contains("adtcore:name=\"Z_TEST_GROUP\""));
        assert!(body.contains("adtcore:type=\"FUGR/F\""));
        assert!(body.contains("adtcore:description=\"Created function group\""));
        assert!(body.contains("adtcore:abapLanguageVersion=\"5\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(!body.contains("abapsource:sourceUri"));
        assert!(!body.contains("abapsource:syntaxConfiguration"));
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
    fn assigns_the_parent_to_function_group_include_creation_properties() {
        let group = ObjectKey::<FunctionGroup>::new("ZGROUP123");
        let include = group.subobject::<FunctionGroupInclude>("LZGROUP123RRR");
        let resolved_parent = ObjectRef::for_test(
            group.erase(),
            AdtUri::parse("/sap/bc/adt/functions/groups/zgroup123").unwrap(),
            None,
        );
        let mut properties = FunctionGroupIncludeCreateProperties::builder()
            .description("zttfart")
            .build()
            .unwrap();
        properties.assign_reference(&ObjectRef::for_test(
            include,
            AdtUri::parse("/sap/bc/adt/functions/groups/zgroup123/includes/lzgroup123rrr").unwrap(),
            Some(resolved_parent),
        ));

        let body = String::from_utf8(properties.to_xml().unwrap()).unwrap();
        assert!(body.contains("<finclude:abapFunctionGroupInclude"));
        assert!(body.contains("adtcore:description=\"zttfart\""));
        assert!(body.contains("adtcore:name=\"LZGROUP123RRR\""));
        assert!(body.contains("adtcore:type=\"FUGR/I\""));
        assert!(body.contains("<adtcore:containerRef"));
        assert!(body.contains("adtcore:name=\"ZGROUP123\""));
        assert!(body.contains("adtcore:type=\"FUGR/F\""));
        assert!(body.contains("adtcore:uri=\"/sap/bc/adt/functions/groups/zgroup123\""));
        assert!(!body.contains("adtcore:packageRef"));
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
