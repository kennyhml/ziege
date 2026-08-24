use serde::{Deserialize, Serialize};
use zadt_macros::object_type;

use crate::{
    AbapLanguageVersion, AdtUri, AdvertisedLink, AdvertisedObjectReference, GlobalWorkbenchType,
    ObjectRef, ObjectType, ObjectVersion, PropertyModel, Structure, SyntaxConfiguration,
};

fn belongs_to_function_group_child<T>(
    name: &str,
    object_type: &GlobalWorkbenchType,
    container: &AdvertisedObjectReference,
    reference: &ObjectRef<T>,
) -> bool {
    if name != reference.name() || object_type != reference.object_type() {
        return false;
    }

    let Some(container_uri) = container
        .uri
        .as_deref()
        .and_then(|uri| AdtUri::parse(uri).ok())
    else {
        return false;
    };
    let Some((parent_name, parent_uri, parent_type)) = reference.parent_identity() else {
        return container.object_type.as_ref() == Some(&FunctionGroup::WORKBENCH_TYPE)
            && container
                .name
                .as_deref()
                .is_some_and(|name| container_uri.last_segment_matches(name))
            && reference.uri().is_descendant_of(&container_uri);
    };
    container.name.as_deref() == Some(parent_name)
        && container.object_type.as_ref() == Some(parent_type)
        && container_uri.semantically_eq(parent_uri)
}

fn describe_function_group_child(
    name: &str,
    object_type: &GlobalWorkbenchType,
    container: &AdvertisedObjectReference,
) -> String {
    format!(
        "{} ({}) in container {} ({}) at {}",
        name,
        object_type,
        container.name.as_deref().unwrap_or("<unknown>"),
        container
            .object_type
            .as_ref()
            .map(GlobalWorkbenchType::as_str)
            .unwrap_or("<unknown>"),
        container.uri.as_deref().unwrap_or("<unknown>")
    )
}

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

/// The SAP media-type version used to decode function-group properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FunctionGroupPropertiesVersion {
    V2,
    V3,
}

impl FunctionGroupPropertiesVersion {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.functions.groups.v2+xml",
            Self::V3 => "application/vnd.sap.adt.functions.groups.v3+xml",
        }
    }
}

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
    pub version: ObjectVersion,
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

impl PropertyModel for FunctionGroupProperties {
    type Version = FunctionGroupPropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] = &[
        FunctionGroupPropertiesVersion::V3,
        FunctionGroupPropertiesVersion::V2,
    ];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("group", "http://www.sap.com/adt/functions/groups"),
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

    fn links(&self) -> &[AdvertisedLink] {
        &self.links
    }
}

impl Structure for FunctionGroup {}

/// The SAP media-type version used to decode function-module properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FunctionModulePropertiesVersion {
    V3,
}

impl FunctionModulePropertiesVersion {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::V3 => "application/vnd.sap.adt.functions.fmodules.v3+xml",
        }
    }
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
    pub version: ObjectVersion,
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

impl PropertyModel for FunctionModuleProperties {
    type Version = FunctionModulePropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] = &[FunctionModulePropertiesVersion::V3];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("fmodule", "http://www.sap.com/adt/functions/fmodules"),
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

    fn belongs_to<T>(&self, reference: &ObjectRef<T>) -> bool {
        belongs_to_function_group_child(&self.name, &self.object_type, &self.container, reference)
    }

    fn object_description(&self) -> String {
        describe_function_group_child(&self.name, &self.object_type, &self.container)
    }

    fn links(&self) -> &[AdvertisedLink] {
        &self.links
    }
}

/// The SAP media-type version used to decode function-group include properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FunctionGroupIncludePropertiesVersion {
    V2,
}

impl FunctionGroupIncludePropertiesVersion {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.functions.fincludes.v2+xml",
        }
    }
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
    pub version: ObjectVersion,
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

impl PropertyModel for FunctionGroupIncludeProperties {
    type Version = FunctionGroupIncludePropertiesVersion;

    const SUPPORTED_VERSIONS: &'static [Self::Version] =
        &[FunctionGroupIncludePropertiesVersion::V2];
    const XML_NAMESPACES: &'static [(&'static str, &'static str)] = &[
        ("finclude", "http://www.sap.com/adt/functions/fincludes"),
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

    fn belongs_to<T>(&self, reference: &ObjectRef<T>) -> bool {
        belongs_to_function_group_child(&self.name, &self.object_type, &self.container, reference)
    }

    fn object_description(&self) -> String {
        describe_function_group_child(&self.name, &self.object_type, &self.container)
    }

    fn links(&self) -> &[AdvertisedLink] {
        &self.links
    }
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
    fn subobject_properties_validate_their_function_group() {
        let group_reference = ObjectRef::<FunctionGroup>::new(
            "Z_TEST_GROUP".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group").unwrap(),
        );
        let module: FunctionModuleProperties = serde_xml_rs::from_str(MODULE_XML).unwrap();
        let module_reference = ObjectRef::<FunctionModule>::new(
            module.name.clone(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc").unwrap(),
        )
        .with_parent(&group_reference);
        assert!(module.belongs_to(&module_reference));
        let detached_module_reference =
            ObjectRef::<FunctionModule>::new(module.name.clone(), module_reference.uri().clone());
        assert!(module.belongs_to(&detached_module_reference));

        let mut wrong_module = module.clone();
        wrong_module.container.uri = Some("/sap/bc/adt/functions/groups/z_other_group".to_owned());
        assert!(!wrong_module.belongs_to(&module_reference));
        assert!(!wrong_module.belongs_to(&detached_module_reference));
        assert!(matches!(
            wrong_module.to_xml_for(&module_reference),
            Err(crate::ObjectError::UnexpectedObjectReference { actual, .. })
                if actual == "ZZZZFUNC (FUGR/FF) in container Z_TEST_GROUP (FUGR/F) at /sap/bc/adt/functions/groups/z_other_group"
        ));
        assert!(matches!(
            crate::objects::object::validate_typed_object::<FunctionModule>(
                &module_reference,
                FunctionModulePropertiesVersion::V3.media_type(),
                &wrong_module,
            ),
            Err(crate::ObjectError::UnexpectedObjectReference { .. })
        ));

        let mut wrong_container_name = module.clone();
        wrong_container_name.container.name = Some("Z_OTHER_GROUP".to_owned());
        assert!(!wrong_container_name.belongs_to(&detached_module_reference));
        let restored_reference: ObjectRef<FunctionModule> =
            serde_json::from_value(serde_json::to_value(&module_reference).unwrap()).unwrap();
        assert!(!wrong_module.belongs_to(&restored_reference));

        let include: FunctionGroupIncludeProperties = serde_xml_rs::from_str(INCLUDE_XML).unwrap();
        let include_reference = ObjectRef::<FunctionGroupInclude>::new(
            include.name.clone(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/includes/lz_test_grouptop")
                .unwrap(),
        )
        .with_parent(&group_reference);
        assert!(include.belongs_to(&include_reference));

        let mut wrong_include = include;
        wrong_include.container.object_type = Some(FunctionModule::WORKBENCH_TYPE);
        assert!(!wrong_include.belongs_to(&include_reference));

        let namespaced_group = ObjectRef::<FunctionGroup>::new(
            "/DEMO/GROUP".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/%2Fdemo%2Fgroup").unwrap(),
        );
        let namespaced_module = ObjectRef::<FunctionModule>::new(
            "ZZZZFUNC".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/%2Fdemo%2Fgroup/fmodules/zzzzfunc")
                .unwrap(),
        )
        .with_parent(&namespaced_group);
        let mut namespaced_properties: FunctionModuleProperties =
            serde_xml_rs::from_str(MODULE_XML).unwrap();
        namespaced_properties.container.name = Some("/DEMO/GROUP".to_owned());
        namespaced_properties.container.uri =
            Some("/sap/bc/adt/functions/groups/%2fdemo%2fgroup".to_owned());
        assert!(namespaced_properties.belongs_to(&namespaced_module));
    }

    #[test]
    fn serializes_complete_properties_for_updates() {
        let group: FunctionGroupProperties = serde_xml_rs::from_str(GROUP_XML).unwrap();
        let group_reference = ObjectRef::<FunctionGroup>::new(
            group.name.clone(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group").unwrap(),
        );
        let group_xml = String::from_utf8(group.to_xml_for(&group_reference).unwrap()).unwrap();
        assert!(group_xml.contains("<group:abapFunctionGroup"));
        assert!(group_xml.contains("adtcore:name=\"Z_TEST_GROUP\""));
        assert!(group_xml.contains("<adtcore:packageRef"));
        assert!(serde_xml_rs::from_str::<FunctionGroupProperties>(&group_xml).is_ok());

        let module: FunctionModuleProperties = serde_xml_rs::from_str(MODULE_XML).unwrap();
        let module_reference = ObjectRef::<FunctionModule>::new(
            module.name.clone(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc").unwrap(),
        );
        let module_xml = String::from_utf8(module.to_xml_for(&module_reference).unwrap()).unwrap();
        assert!(module_xml.contains("<fmodule:abapFunctionModule"));
        assert!(module_xml.contains("adtcore:name=\"ZZZZFUNC\""));
        assert!(module_xml.contains("<adtcore:containerRef"));
        assert!(serde_xml_rs::from_str::<FunctionModuleProperties>(&module_xml).is_ok());

        let include: FunctionGroupIncludeProperties = serde_xml_rs::from_str(INCLUDE_XML).unwrap();
        let include_reference = ObjectRef::<FunctionGroupInclude>::new(
            include.name.clone(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/includes/lz_test_grouptop")
                .unwrap(),
        );
        let include_xml =
            String::from_utf8(include.to_xml_for(&include_reference).unwrap()).unwrap();
        assert!(include_xml.contains("<finclude:abapFunctionGroupInclude"));
        assert!(include_xml.contains("adtcore:name=\"LZ_TEST_GROUPTOP\""));
        assert!(include_xml.contains("<adtcore:containerRef"));
        assert!(serde_xml_rs::from_str::<FunctionGroupIncludeProperties>(&include_xml).is_ok());
    }
}
