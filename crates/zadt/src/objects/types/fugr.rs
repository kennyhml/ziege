use serde::{Deserialize, Serialize};
use zadt_macros::{CreateProperties, object_type};

use crate::{
    AbapLanguageVersion, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    MediaTypes, ResourceView, SourceObjectStatus, SyntaxConfiguration, ToXml, WorkbenchVersion,
};

/// An ABAP function group.
#[object_type(
    properties = FunctionGroupProperties,
    media_types = MediaTypes::new(&[
        "application/vnd.sap.adt.functions.groups.v3+xml",
        "application/vnd.sap.adt.functions.groups.v2+xml",
    ]),
    resources = function_group_resources,
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
        Source,
        Structure,
    )
)]
pub struct FunctionGroup;

/// A function module owned by an ABAP function group.
#[object_type(
    properties = FunctionModuleProperties,
    media_types = MediaTypes::new(&["application/vnd.sap.adt.functions.fmodules.v3+xml"]),
    workbench_type = "FUGR/FF",
    subobject,
    container = Some(&properties.container),
    capabilities(
        Create(FunctionModuleCreateProperties),
        Source(properties.source_uri),
    )
)]
pub struct FunctionModule;

/// A source include owned by an ABAP function group.
#[object_type(
    properties = FunctionGroupIncludeProperties,
    media_types = MediaTypes::new(&["application/vnd.sap.adt.functions.fincludes.v2+xml"]),
    workbench_type = "FUGR/I",
    subobject,
    container = Some(&properties.container),
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
    /// The source object status exactly as supplied by ADT.
    #[serde(
        rename = "@abapsource:sourceObjectStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_object_status: Option<SourceObjectStatus>,

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
    pub(crate) workbench_type: GlobalWorkbenchType,
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

impl ToXml for FunctionGroupProperties {
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("group", "http://www.sap.com/adt/functions/groups"),
        ("abapsource", "http://www.sap.com/adt/abapsource"),
        ("adtcore", "http://www.sap.com/adt/core"),
        ("atom", "http://www.w3.org/2005/Atom"),
    ];
}

fn function_group_resources(properties: &FunctionGroupProperties) -> ResourceView<'_> {
    ResourceView::new(&properties.links)
        .with_syntax_links(&properties.syntax_configuration.language.links)
        .with_main(&properties.source_uri)
}

/// Function-module processing mode in ADT.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FunctionModuleProcessingType {
    Normal,
    Rfc,
    Update,
    Other(String),
}

impl FunctionModuleProcessingType {
    /// Returns the exact ADT wire value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Normal => "normal",
            Self::Rfc => "rfc",
            Self::Update => "update",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for FunctionModuleProcessingType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "normal" => Self::Normal,
            "rfc" => Self::Rfc,
            "update" => Self::Update,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for FunctionModuleProcessingType {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for FunctionModuleProcessingType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FunctionModuleProcessingType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// Function-module release state in ADT, distinct from AFF enum spellings.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FunctionModuleReleaseState {
    NotReleased,
    External,
    Internal,
    Obsolete,
    MarkedForRelease,
    Other(String),
}

impl FunctionModuleReleaseState {
    /// Returns the exact ADT wire value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::NotReleased => "notReleased",
            Self::External => "external",
            Self::Internal => "internal",
            Self::Obsolete => "obsolete",
            Self::MarkedForRelease => "markedForRelease",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for FunctionModuleReleaseState {
    fn from(value: String) -> Self {
        match value.as_str() {
            "notReleased" => Self::NotReleased,
            "external" => Self::External,
            "internal" => Self::Internal,
            "obsolete" => Self::Obsolete,
            "markedForRelease" => Self::MarkedForRelease,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for FunctionModuleReleaseState {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for FunctionModuleReleaseState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FunctionModuleReleaseState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// Permitted RFC caller scope.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RfcScope {
    NotClassified,
    FromSameClientAndUser,
    FromSameSystem,
    FromAnySystem,
    Other(String),
}

impl RfcScope {
    /// Returns the exact ADT wire value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::NotClassified => "notClassified",
            Self::FromSameClientAndUser => "fromSameClientAndUser",
            Self::FromSameSystem => "fromSameSystem",
            Self::FromAnySystem => "fromAnySystem",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for RfcScope {
    fn from(value: String) -> Self {
        match value.as_str() {
            "notClassified" => Self::NotClassified,
            "fromSameClientAndUser" => Self::FromSameClientAndUser,
            "fromSameSystem" => Self::FromSameSystem,
            "fromAnySystem" => Self::FromAnySystem,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for RfcScope {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for RfcScope {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RfcScope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// Permitted RFC serialization, displayed as the interface contract.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RfcVersion {
    Any,
    FastSerializationRequired,
    Other(String),
}

impl RfcVersion {
    /// Returns the exact ADT wire value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Any => "any",
            Self::FastSerializationRequired => "fastSerializationRequired",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for RfcVersion {
    fn from(value: String) -> Self {
        match value.as_str() {
            "any" => Self::Any,
            "fastSerializationRequired" => Self::FastSerializationRequired,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for RfcVersion {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for RfcVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RfcVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// Update-task execution mode in ADT.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum UpdateTaskKind {
    StartImmediate,
    StartDelayed,
    ImmediateStartNoRestart,
    CollectiveRun,
    UnsupportedKind,
    Other(String),
}

impl UpdateTaskKind {
    /// Returns the exact ADT wire value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::StartImmediate => "startImmediate",
            Self::StartDelayed => "startDelayed",
            Self::ImmediateStartNoRestart => "immediateStartNoRestart",
            Self::CollectiveRun => "collectiveRun",
            Self::UnsupportedKind => "unsupportedKind",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for UpdateTaskKind {
    fn from(value: String) -> Self {
        match value.as_str() {
            "startImmediate" => Self::StartImmediate,
            "startDelayed" => Self::StartDelayed,
            "immediateStartNoRestart" => Self::ImmediateStartNoRestart,
            "collectiveRun" => Self::CollectiveRun,
            "unsupportedKind" => Self::UnsupportedKind,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for UpdateTaskKind {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for UpdateTaskKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for UpdateTaskKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// The complete function-module properties payload.
#[derive(Clone, CreateProperties, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[create_properties(
    name = FunctionModuleCreateProperties,
    doc = "The sparse payload used to create an ABAP function module."
)]
#[serde(rename = "fmodule:abapFunctionModule", deny_unknown_fields)]
pub struct FunctionModuleProperties {
    #[serde(
        rename = "@fmodule:releaseState",
        skip_serializing_if = "Option::is_none"
    )]
    pub release_state: Option<FunctionModuleReleaseState>,

    #[serde(
        rename = "@fmodule:processingType",
        skip_serializing_if = "Option::is_none"
    )]
    pub processing_type: Option<FunctionModuleProcessingType>,

    /// Whether parameters are globally visible within the function group.
    #[serde(rename = "@fmodule:global", skip_serializing_if = "Option::is_none")]
    pub global: Option<bool>,

    /// Whether classic RFC and basXML have equivalent semantics.
    #[serde(
        rename = "@fmodule:basXMLEnabled",
        skip_serializing_if = "Option::is_none"
    )]
    pub basxml_enabled: Option<bool>,

    #[serde(
        rename = "@fmodule:abapFromJava",
        skip_serializing_if = "Option::is_none"
    )]
    pub abap_from_java: Option<bool>,

    #[serde(
        rename = "@fmodule:javaFromAbap",
        skip_serializing_if = "Option::is_none"
    )]
    pub java_from_abap: Option<bool>,

    #[serde(
        rename = "@fmodule:javaRemote",
        skip_serializing_if = "Option::is_none"
    )]
    pub java_remote: Option<bool>,

    /// Release date in the advertised YYYY-MM-DD representation.
    #[serde(
        rename = "@fmodule:releaseDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub release_date: Option<String>,

    #[serde(
        rename = "@fmodule:updateTaskKind",
        skip_serializing_if = "Option::is_none"
    )]
    pub update_task_kind: Option<UpdateTaskKind>,

    #[serde(rename = "@fmodule:rfcScope", skip_serializing_if = "Option::is_none")]
    pub rfc_scope: Option<RfcScope>,

    #[serde(
        rename = "@fmodule:rfcVersion",
        skip_serializing_if = "Option::is_none"
    )]
    pub rfc_version: Option<RfcVersion>,

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
    pub(crate) workbench_type: GlobalWorkbenchType,
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
// TODO: Add creation-template support for createIncludeStatement (defaults to true).
// Generalize ClassTemplate/ClassTemplateProperty for reuse and confirm whether
// property-only templates can omit the template name.
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
    pub(crate) workbench_type: GlobalWorkbenchType,
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
    use crate::{AdtUri, Create, ObjectKey, ObjectRef};

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
        let reference = ObjectRef::for_test(
            ObjectKey::<FunctionGroup>::new("Z_TEST_GROUP"),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group").unwrap(),
            None,
        );
        FunctionGroup::prepare_payload(&mut properties, &reference);

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
        assert_eq!(
            properties.release_state,
            Some(FunctionModuleReleaseState::NotReleased)
        );
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
    fn module_attributes_preserve_wire_values_and_omissions() {
        let xml =
            include_str!("../../../tests/fixtures/function-module-bapi-transaction-commit.xml");
        let properties: FunctionModuleProperties = serde_xml_rs::from_str(xml).unwrap();
        assert_eq!(
            properties.release_state,
            Some(FunctionModuleReleaseState::External)
        );
        assert_eq!(properties.release_date.as_deref(), Some("1998-01-15"));
        assert_eq!(
            properties.processing_type,
            Some(FunctionModuleProcessingType::Rfc)
        );
        assert_eq!(properties.rfc_scope, Some(RfcScope::NotClassified));
        assert_eq!(properties.rfc_version, Some(RfcVersion::Any));
        let baseline = serde_json::to_value(properties).unwrap();
        for (attribute, values) in [
            ("processingType", &["normal", "rfc", "update"][..]),
            (
                "releaseState",
                &[
                    "notReleased",
                    "external",
                    "internal",
                    "obsolete",
                    "markedForRelease",
                ][..],
            ),
            (
                "rfcScope",
                &[
                    "notClassified",
                    "fromSameClientAndUser",
                    "fromSameSystem",
                    "fromAnySystem",
                ][..],
            ),
            ("rfcVersion", &["any", "fastSerializationRequired"][..]),
            (
                "updateTaskKind",
                &[
                    "startImmediate",
                    "startDelayed",
                    "immediateStartNoRestart",
                    "collectiveRun",
                    "unsupportedKind",
                ][..],
            ),
        ] {
            let key = format!("@fmodule:{attribute}");
            for value in values.iter().copied().chain(["", "futureValue"]) {
                let mut wire = baseline.clone();
                wire[&key] = serde_json::json!(value);
                let loaded: FunctionModuleProperties =
                    serde_json::from_value(wire.clone()).unwrap();
                let xml = String::from_utf8(loaded.to_xml().unwrap()).unwrap();
                assert!(xml.contains(&format!("fmodule:{attribute}=\"{value}\"")));
                let reparsed: FunctionModuleProperties = serde_xml_rs::from_str(&xml).unwrap();
                assert_eq!(reparsed, loaded);
                assert_eq!(serde_json::to_value(reparsed).unwrap(), wire);
            }
            let mut wire = baseline.clone();
            wire.as_object_mut().unwrap().remove(&key);
            let loaded: FunctionModuleProperties = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(&loaded).unwrap(), wire);
            assert!(
                !String::from_utf8(loaded.to_xml().unwrap())
                    .unwrap()
                    .contains(&format!("fmodule:{attribute}="))
            );
        }
        for attribute in [
            "global",
            "basXMLEnabled",
            "abapFromJava",
            "javaFromAbap",
            "javaRemote",
        ] {
            let key = format!("@fmodule:{attribute}");
            assert!(baseline.get(&key).is_none());
            for value in [false, true] {
                let mut wire = baseline.clone();
                wire[&key] = serde_json::json!(value);
                let loaded: FunctionModuleProperties =
                    serde_json::from_value(wire.clone()).unwrap();
                let xml = String::from_utf8(loaded.to_xml().unwrap()).unwrap();
                assert!(xml.contains(&format!("fmodule:{attribute}=\"{value}\"")));
                assert_eq!(
                    serde_xml_rs::from_str::<FunctionModuleProperties>(&xml).unwrap(),
                    loaded
                );
            }
        }
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
        let reference = ObjectRef::for_test(
            include,
            AdtUri::parse("/sap/bc/adt/functions/groups/zgroup123/includes/lzgroup123rrr").unwrap(),
            Some(resolved_parent),
        );
        FunctionGroupInclude::prepare_payload(&mut properties, &reference);

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
