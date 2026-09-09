//! Interface metadata and its advertised main source.
//!
//! Header fields map to ADT description, master language, and language version.
//! Category, proxy status, and SE80 descriptions have no ZADT backing; only their
//! empty/default values are accepted. Unrepresented ADT fields remain on the baseline.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/intf/intf-v1.json>.

use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{Interface, InterfaceProperties, ObjectSnapshot, ObjectType};

use crate::{
    AbapLanguageVersion, Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping, clas::ClassDescriptions},
    helpers::{is_false, parse_object},
    models::{language_from_adt, language_to_adt},
    validate::one_of,
};

pub(crate) static INTERFACE_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "INTF",
    version: "1",
    workbench_types: &[Interface::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.intf.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.intf.abap",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
    ],
};

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let document = ProjectedInterfaceProperties::from_adt(obj.typed_properties::<Interface>()?)?;
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<Interface>()?;
    let edited: ProjectedInterfaceProperties = parse_object(edited)?;
    edited.validate()?;
    for (unsupported, field) in [
        (edited.category != InterfaceCategory::General, "category"),
        (edited.proxy, "proxy"),
        (
            edited.descriptions.as_ref().is_some_and(|v| !v.is_empty()),
            "descriptions",
        ),
    ] {
        if unsupported {
            return Err(ProjectionError::UnsupportedAffProperty {
                object_type: "INTF",
                field,
            });
        }
    }
    let previous = ProjectedInterfaceProperties::from_adt(original)?;
    let mut merged = original.clone();
    merged.description = edited.header.description;
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    if edited.header.abap_language_version != previous.header.abap_language_version {
        merged.abap_language_version = Some(edited.header.abap_language_version.to_adt_reps());
    }
    if merged == *original {
        return Ok(None);
    }
    Ok(Some(serde_json::to_value(merged)?))
}

/// AFF INTF v1. Nondefault category/proxy and nonempty descriptions cannot be saved.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProjectedInterfaceProperties {
    #[garde(custom(one_of([INTERFACE_FORMAT.version()])))]
    pub format_version: String,
    #[garde(dive)]
    pub header: InterfaceHeader,
    #[serde(default, skip_serializing_if = "InterfaceCategory::is_default")]
    pub category: InterfaceCategory,
    #[serde(default, skip_serializing_if = "is_false")]
    pub proxy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub descriptions: Option<ClassDescriptions>,
}

/// `description` maps directly; `originalLanguage` converts SAP to BCP47.
/// Language version uses REPS Standard `X`; unchanged spellings are preserved.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct InterfaceHeader {
    #[garde(length(chars, max = 60))]
    pub description: String,
    #[garde(length(chars, min = 2))]
    pub original_language: String,
    #[serde(default, skip_serializing_if = "AbapLanguageVersion::is_standard")]
    pub abap_language_version: AbapLanguageVersion,
}

/// AFF categories; ADT interface properties do not expose this information.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InterfaceCategory {
    #[default]
    General,
    ClassicBadi,
    BusinessStaticComponents,
    BusinessInstanceComponents,
    DbProcedureProxy,
    WebDynproRuntime,
    EnterpriseService,
}

impl InterfaceCategory {
    fn is_default(&self) -> bool {
        *self == Self::General
    }
}

impl ProjectedInterfaceProperties {
    fn from_adt(properties: &InterfaceProperties) -> Result<Self, ProjectionError> {
        let document = Self {
            format_version: INTERFACE_FORMAT.version().to_owned(),
            header: InterfaceHeader {
                description: properties.description.clone(),
                original_language: language_from_adt(
                    &properties.master_language,
                    "header.originalLanguage",
                )?,
                abap_language_version: AbapLanguageVersion::from_adt(
                    properties.abap_language_version.as_ref(),
                    "X",
                )
                .map_err(|value| ProjectionError::InvalidAffField {
                    field: "header.abapLanguageVersion",
                    message: format!("unsupported ADT value `{value}`"),
                })?,
            },
            category: InterfaceCategory::General,
            proxy: false,
            descriptions: None,
        };
        document.validate()?;
        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileBacking, project, test_support};
    use serde_json::{Value, json};

    #[test]
    fn interface_projects_source_and_preserves_unrepresented_properties() {
        let reference = test_support::reference::<Interface>(
            "IF_ADT_URI_MAPPER",
            "/sap/bc/adt/oo/interfaces/if_adt_uri_mapper",
        );
        let snapshot = test_support::properties(
            &reference,
            Interface::MEDIA_TYPES[0],
            "etag",
            include_bytes!("../../../zadt/tests/fixtures/interface-if-adt-uri-mapper-v5.xml"),
        );
        let original = snapshot.properties().clone();
        let projection = project(snapshot.into_erased()).unwrap();
        assert!(matches!(
            projection
                .file("if_adt_uri_mapper.intf.abap")
                .unwrap()
                .backing(),
            FileBacking::Source(_)
        ));
        let FileBacking::Properties(mapping) = projection
            .file("if_adt_uri_mapper.intf.json")
            .unwrap()
            .backing()
        else {
            panic!()
        };
        let content = mapping.render().unwrap();
        assert!(content.ends_with('\n'));
        assert!(mapping.merge(&content).unwrap().is_none());
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        edited["header"]["description"] = json!("Changed interface");
        let merged: InterfaceProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        let mut expected = original;
        expected.description = "Changed interface".to_owned();
        assert_eq!(merged, expected);
        for (field, value) in [
            ("proxy", json!(true)),
            ("category", json!("classicBadi")),
            (
                "descriptions",
                json!({"types":[{"name":"T", "description":"Type"}]}),
            ),
        ] {
            let mut invalid = edited.clone();
            invalid[field] = value;
            assert!(matches!(
                mapping.merge(&invalid.to_string()),
                Err(ProjectionError::UnsupportedAffProperty { .. })
            ));
        }
        edited["header"]["description"] = json!("x".repeat(61));
        assert!(mapping.merge(&edited.to_string()).is_err());
    }
}
