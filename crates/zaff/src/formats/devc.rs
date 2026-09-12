//! Package mapping to AFF `.devc.json`.
//!
//! ```text
//! AFF generalInformation       ADT
//! type                         attributes.package_type
//! superPackage                 super_package.name
//! applicationComponent         application_component.name
//! softwareComponent            transport.software_component.name
//! transportLayer               transport.transport_layer.name
//! supportsRecordChanges        attributes.record_changes
//! isAddingObjectsNotAllowed    attributes.adding_objects_not_allowed (misnamed ADT wire attribute)
//! isEncapsulated               attributes.encapsulated
//! defaultAbapLanguageVersion    attributes.language_version
//! switch                       switch.name
//! ```
//!
//! Header description and original language map to ADT description/master_language.
//! Use accesses map interface names and severities. Unchanged names retain their
//! complete references, including when reordered. New names get name-only references.
//! Assignment changes clear stale descriptions while preserving capability flags.
//!
//! SPAK_ST_PACKAGES serializes IS_ADDING_OBJECTS_NOT_ALLOWED under the misleading
//! XML name `pak:isAddingObjectsAllowed`, without negation. The AFF mapping uses
//! the underlying negative meaning directly. `pak:languageVersion` contains raw
//! package-kind codes, so changing its AFF value to Standard writes an empty string.
//!
//! Switch names map through `pak:switch`. Unchanged assignments retain the switch
//! state, URI, and description. Self package references, extension aliases,
//! translation settings, and software-component type metadata remain on the baseline.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/devc/devc-v1.json>.

use crate::{
    AbapLanguageVersion, Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::{is_false, parse_object},
    models::{language_from_adt, language_to_adt},
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{
    AdvertisedObjectReference, ObjectSnapshot, ObjectType, Package, PackageAssignment,
    PackageProperties, PackageUseAccess, PackageUseAccesses,
};

pub(crate) static PACKAGE_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DEVC",
    version: "1",
    workbench_types: &[Package::WORKBENCH_TYPE],
    files: &[FileSpec::new(
        "<name>.devc.json",
        Cardinality::One,
        Mapping::Properties(PropertiesMapping { render, merge }),
    )],
};

/// AFF package metadata. ADT editor flags, interfaces, and subpackages stay on the baseline.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedPackageProperties {
    #[garde(custom(one_of([PACKAGE_FORMAT.version()])))]
    pub format_version: String,
    #[serde(deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub header: PackageHeader,
    #[serde(default, deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub general_information: PackageGeneralInformation,
    #[serde(
        default,
        deserialize_with = "crate::helpers::objects",
        skip_serializing_if = "Vec::is_empty"
    )]
    #[garde(dive)]
    pub use_accesses: Vec<PackageUseAccessProjection>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageHeader {
    #[garde(length(chars, max = 60))]
    pub description: String,
    #[garde(length(chars, min = 2))]
    pub original_language: String,
}

/// Package settings. `type` is required whenever the optional block is present.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct PackageGeneralInformation {
    #[serde(rename = "type", deserialize_with = "crate::helpers::string_enum")]
    pub package_type: PackageType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 30))]
    pub super_package: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 30))]
    pub switch: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 24))]
    pub application_component: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 30))]
    pub software_component: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 4))]
    pub transport_layer: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub supports_record_changes: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_adding_objects_not_allowed: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_encapsulated: bool,
    #[serde(
        default,
        deserialize_with = "crate::helpers::string_enum",
        skip_serializing_if = "AbapLanguageVersion::is_standard"
    )]
    pub default_abap_language_version: AbapLanguageVersion,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageType {
    #[default]
    Development,
    Main,
    Structure,
}

impl PackageType {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "development" => Ok(Self::Development),
            "main" => Ok(Self::Main),
            "structure" => Ok(Self::Structure),
            _ => Err(invalid("generalInformation.type", value)),
        }
    }
    fn adt_value(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Main => "main",
            Self::Structure => "structure",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct PackageUseAccessProjection {
    #[serde(default)]
    #[garde(length(chars, max = 30))]
    pub package_interface: String,
    #[serde(default, deserialize_with = "crate::helpers::string_enum")]
    pub severity: PackageUseAccessSeverity,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageUseAccessSeverity {
    #[default]
    None,
    Information,
    Warning,
    Error,
    Obsolete,
}

impl PackageUseAccessSeverity {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "none" => Ok(Self::None),
            "information" => Ok(Self::Information),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "obsolete" => Ok(Self::Obsolete),
            _ => Err(invalid("useAccesses.severity", value)),
        }
    }
    fn adt_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Information => "information",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Obsolete => "obsolete",
        }
    }
}

fn invalid(field: &'static str, value: &str) -> ProjectionError {
    ProjectionError::InvalidAffField {
        field,
        message: format!("unsupported ADT value `{value}`"),
    }
}

impl ProjectedPackageProperties {
    fn from_adt(p: &PackageProperties) -> Result<Self, ProjectionError> {
        let document = Self {
            format_version: PACKAGE_FORMAT.version().to_owned(),
            header: PackageHeader {
                description: p.description.clone().unwrap_or_default(),
                original_language: language_from_adt(
                    &p.master_language,
                    "header.originalLanguage",
                )?,
            },
            general_information: PackageGeneralInformation {
                package_type: PackageType::from_adt(&p.attributes.package_type)?,
                super_package: p
                    .super_package
                    .as_ref()
                    .and_then(|v| v.name.clone())
                    .unwrap_or_default(),
                switch: p
                    .switch
                    .as_ref()
                    .and_then(|v| v.name.clone())
                    .unwrap_or_default(),
                application_component: p.application_component.name.clone(),
                software_component: p.transport.software_component.name.clone(),
                transport_layer: p.transport.transport_layer.name.clone(),
                supports_record_changes: p.attributes.record_changes,
                is_adding_objects_not_allowed: p.attributes.adding_objects_not_allowed,
                is_encapsulated: p.attributes.encapsulated,
                default_abap_language_version: AbapLanguageVersion::from_adt(
                    Some(&p.attributes.language_version),
                    "X",
                )
                .map_err(|v| invalid("generalInformation.defaultAbapLanguageVersion", v))?,
            },
            use_accesses: p
                .use_accesses
                .iter()
                .flat_map(|v| &v.use_access)
                .map(|v| {
                    Ok(PackageUseAccessProjection {
                        package_interface: v.package_interface.name.clone().unwrap_or_default(),
                        severity: PackageUseAccessSeverity::from_adt(&v.severity)?,
                    })
                })
                .collect::<Result<_, ProjectionError>>()?,
        };
        document.validate()?;
        Ok(document)
    }
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    Ok(
        serde_json::to_string_pretty(&ProjectedPackageProperties::from_adt(
            obj.typed_properties::<Package>()?,
        )?)? + "\n",
    )
}

fn assignment(value: &mut PackageAssignment, name: String) {
    if value.name != name {
        value.name = name;
        value.description.clear();
        value.assignment_type = None;
        value.type_description = None;
    }
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<Package>()?;
    let edited: ProjectedPackageProperties = parse_object(edited)?;
    edited.validate()?;
    let previous = ProjectedPackageProperties::from_adt(original)?;
    let mut merged = original.clone();
    if edited.header.description != original.description.as_deref().unwrap_or_default() {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    let e = edited.general_information;
    let p = previous.general_information;
    merged.attributes.package_type = e.package_type.adt_value().to_owned();
    merged.attributes.record_changes = e.supports_record_changes;
    merged.attributes.adding_objects_not_allowed = e.is_adding_objects_not_allowed;
    merged.attributes.encapsulated = e.is_encapsulated;
    if e.default_abap_language_version != p.default_abap_language_version {
        // SPAK_ST_PACKAGES writes the raw package-kind value, whose Standard is blank.
        merged.attributes.language_version = e
            .default_abap_language_version
            .to_adt(zadt::AbapLanguageVersion::Other(String::new()));
    }
    if e.super_package != p.super_package {
        merged.super_package = (!e.super_package.is_empty()).then_some(AdvertisedObjectReference {
            name: Some(e.super_package),
            ..Default::default()
        });
    }
    if e.switch != p.switch {
        merged.switch = (!e.switch.is_empty()).then_some(zadt::AdvertisedSwitchReference {
            name: Some(e.switch),
            ..Default::default()
        });
    }
    assignment(&mut merged.application_component, e.application_component);
    assignment(
        &mut merged.transport.software_component,
        e.software_component,
    );
    assignment(&mut merged.transport.transport_layer, e.transport_layer);
    if edited.use_accesses != previous.use_accesses {
        let accesses = merged
            .use_accesses
            .get_or_insert_with(|| PackageUseAccesses {
                visible: false,
                use_access: Vec::new(),
            });
        let mut remaining: Vec<_> = accesses.use_access.drain(..).map(Some).collect();
        for edited in edited.use_accesses {
            // Consume each matching old reference once, preserving duplicates and reordering.
            let old = remaining
                .iter_mut()
                .find(|v| {
                    v.as_ref().is_some_and(|v| {
                        v.package_interface.name.as_deref().unwrap_or_default()
                            == edited.package_interface
                    })
                })
                .and_then(Option::take);
            let mut access = old.unwrap_or_else(|| PackageUseAccess {
                severity: String::new(),
                package_ref: None,
                package_interface: AdvertisedObjectReference {
                    name: Some(edited.package_interface),
                    ..Default::default()
                },
            });
            access.severity = edited.severity.adt_value().to_owned();
            accesses.use_access.push(access);
        }
    }
    let value = serde_json::to_value(merged)?;
    if value == serde_json::to_value(original)? {
        return Ok(None);
    }
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileBacking, project, test_support};
    use serde_json::{Value, json};

    #[test]
    fn switch_edits_preserve_reference_metadata_and_package_flags_match_wire_semantics() {
        use zadt::{AdvertisedSwitchReference, ToXml};
        let reference = test_support::reference::<Package>(
            "SADT_TOOLS_CORE",
            "/sap/bc/adt/packages/sadt_tools_core",
        );
        let loaded = test_support::properties(
            &reference,
            Package::MEDIA_TYPES[0],
            "etag",
            include_bytes!("../../../zadt/tests/fixtures/package-sadt-tools-core.xml"),
        );
        let mut original = loaded.properties().clone();
        original.switch = Some(AdvertisedSwitchReference {
            name: Some("DTINF_FW".into()),
            uri: Some("/sap/bc/adt/vit/wb/object_type/sfsw6s/object_name/DTINF_FW".into()),
            state: Some("off".into()),
            description: Some("Information Framework Switch".into()),
            ..Default::default()
        });
        original.attributes.adding_objects_not_allowed = false;
        original.attributes.language_version = zadt::AbapLanguageVersion::CloudDevelopment;
        let obj = test_support::properties(
            &reference,
            Package::MEDIA_TYPES[0],
            "etag",
            &original.to_xml().unwrap(),
        )
        .into_erased();
        let content = render(&obj).unwrap();
        assert_eq!(merge(&obj, &content).unwrap(), None);
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(edited["generalInformation"]["switch"], "DTINF_FW");
        assert!(
            edited["generalInformation"]
                .get("isAddingObjectsNotAllowed")
                .is_none()
        );
        edited["header"]["description"] = json!("Changed");
        let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
        assert_eq!(
            payload["pak:switch"],
            serde_json::to_value(&original).unwrap()["pak:switch"]
        );
        edited["generalInformation"]["switch"] = json!("Z_SWITCH");
        edited["generalInformation"]["isAddingObjectsNotAllowed"] = json!(true);
        edited["generalInformation"]["defaultAbapLanguageVersion"] = json!("standard");
        let payload = merge(&obj, &edited.to_string()).unwrap().unwrap();
        assert_eq!(payload["pak:switch"], json!({"@adtcore:name":"Z_SWITCH"}));
        assert_eq!(
            payload["pak:attributes"]["@pak:isAddingObjectsAllowed"],
            true
        );
        assert_eq!(payload["pak:attributes"]["@pak:languageVersion"], "");
        edited["generalInformation"]["switch"] = json!("");
        assert!(
            merge(&obj, &edited.to_string())
                .unwrap()
                .unwrap()
                .get("pak:switch")
                .is_none()
        );
    }

    #[test]
    fn package_noops_and_edits_preserve_capabilities_and_reference_metadata() {
        let reference = test_support::reference::<Package>(
            "SADT_TOOLS_CORE",
            "/sap/bc/adt/packages/sadt_tools_core",
        );
        let snapshot = test_support::properties(
            &reference,
            Package::MEDIA_TYPES[0],
            "etag",
            include_bytes!("../../../zadt/tests/fixtures/package-sadt-tools-core.xml"),
        );
        let original = snapshot.properties().clone();
        let projection = project(snapshot.into_erased()).unwrap();
        assert_eq!(projection.files().len(), 1);
        let FileBacking::Properties(mapping) = projection
            .file("sadt_tools_core.devc.json")
            .unwrap()
            .backing()
        else {
            panic!()
        };
        let content = mapping.render().unwrap();
        assert!(mapping.merge(&content).unwrap().is_none());
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        edited["header"]["description"] = json!("Changed package");
        let merged = mapping.merge(&edited.to_string()).unwrap().unwrap();
        let mut expected = original.clone();
        expected.description = Some("Changed package".to_owned());
        assert_eq!(merged, serde_json::to_value(&expected).unwrap());
        edited["generalInformation"]["isAddingObjectsNotAllowed"] = json!(true);
        edited["generalInformation"]["superPackage"] = json!("Z_PARENT");
        edited["generalInformation"]["applicationComponent"] = json!("BC-NEW");
        edited["useAccesses"][0]["severity"] = json!("warning");
        let merged: PackageProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        assert!(merged.attributes.adding_objects_not_allowed);
        assert_eq!(
            merged.attributes.adding_objects_allowed_editable,
            original.attributes.adding_objects_allowed_editable
        );
        assert_eq!(
            merged.super_package.unwrap(),
            AdvertisedObjectReference {
                name: Some("Z_PARENT".to_owned()),
                ..Default::default()
            }
        );
        assert!(merged.application_component.description.is_empty());
        let access = &merged.use_accesses.unwrap().use_access[0];
        assert_eq!(access.severity, "warning");
        assert_eq!(
            access.package_interface,
            original.use_accesses.as_ref().unwrap().use_access[0].package_interface
        );
        assert_eq!(
            access.package_ref,
            original.use_accesses.as_ref().unwrap().use_access[0].package_ref
        );
        edited["useAccesses"][0]["packageInterface"] = json!("Z_NEW_INTERFACE");
        let merged: PackageProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        let access = &merged.use_accesses.unwrap().use_access[0];
        assert!(access.package_interface.uri.is_none());
        assert!(access.package_ref.is_none());
        edited["generalInformation"]["switch"] = json!("Z_SWITCH");
        let merged: PackageProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        assert_eq!(merged.switch.unwrap().name.as_deref(), Some("Z_SWITCH"));
        edited["generalInformation"] = json!({});
        assert!(mapping.merge(&edited.to_string()).is_err());
    }
}
