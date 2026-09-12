//! Service definition metadata and advertised `.srvd.acds` source.
//!
//! Header fields map to ADT description, master language, and language version.
//! `generalInformation.sourceOrigin` maps SAP codes 0..9; `sourceType` maps
//! `S` (definition) and `X` (extension). Changed discriminators clear their old
//! display descriptions. All other ADT properties are retained from the baseline.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/srvd/srvd-v1.json>.

use crate::{
    Cardinality, CdsHeader, CdsSourceOrigin, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::parse_object,
    models::language_to_adt,
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{ObjectSnapshot, ObjectType, ServiceDefinition};

pub(crate) static SERVICE_DEFINITION_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "SRVD",
    version: "1",
    workbench_types: &[ServiceDefinition::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.srvd.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.srvd.acds",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
    ],
};

/// AFF SRVD v1, including the required general information block.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedServiceDefinitionProperties {
    #[garde(custom(one_of([SERVICE_DEFINITION_FORMAT.version()])))]
    pub format_version: String,
    #[serde(deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub header: CdsHeader,
    #[serde(deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub general_information: ServiceDefinitionGeneralInformation,
}

/// Both discriminators are required by AFF, even when set to their default values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ServiceDefinitionGeneralInformation {
    #[serde(deserialize_with = "crate::helpers::string_enum")]
    pub source_origin: CdsSourceOrigin,
    #[serde(deserialize_with = "crate::helpers::string_enum")]
    pub source_type: ServiceDefinitionSourceType,
}

/// ADT `S` means definition and `X` means extension.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceDefinitionSourceType {
    Definition,
    Extension,
}

impl ServiceDefinitionSourceType {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "S" => Ok(Self::Definition),
            "X" => Ok(Self::Extension),
            _ => Err(ProjectionError::InvalidAffField {
                field: "generalInformation.sourceType",
                message: format!("unsupported ADT value `{value}`"),
            }),
        }
    }
    fn adt_value(self) -> &'static str {
        match self {
            Self::Definition => "S",
            Self::Extension => "X",
        }
    }
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<ServiceDefinition>()?;
    let document = ProjectedServiceDefinitionProperties {
        format_version: SERVICE_DEFINITION_FORMAT.version().to_owned(),
        header: CdsHeader::from_adt(
            properties.description.as_deref().unwrap_or_default(),
            &properties.master_language,
            &properties.abap_language_version,
        )?,
        general_information: ServiceDefinitionGeneralInformation {
            source_origin: CdsSourceOrigin::from_adt(
                &properties.source_origin,
                "generalInformation.sourceOrigin",
            )?,
            source_type: ServiceDefinitionSourceType::from_adt(&properties.source_type)?,
        },
    };
    document.validate()?;
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<ServiceDefinition>()?;
    let edited: ProjectedServiceDefinitionProperties = parse_object(edited)?;
    edited.validate()?;
    let previous = CdsHeader::from_adt(
        original.description.as_deref().unwrap_or_default(),
        &original.master_language,
        &original.abap_language_version,
    )?;
    let origin =
        CdsSourceOrigin::from_adt(&original.source_origin, "generalInformation.sourceOrigin")?;
    let source_type = ServiceDefinitionSourceType::from_adt(&original.source_type)?;
    let mut merged = original.clone();
    if edited.header.description != original.description.as_deref().unwrap_or_default() {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    if edited.header.abap_language_version != previous.abap_language_version {
        merged.abap_language_version = edited.header.abap_language_version.to_adt_ddic();
    }
    if edited.general_information.source_origin != origin {
        merged.source_origin = edited
            .general_information
            .source_origin
            .adt_value()
            .to_owned();
        merged.source_origin_description.clear();
    }
    if edited.general_information.source_type != source_type {
        merged.source_type = edited
            .general_information
            .source_type
            .adt_value()
            .to_owned();
        merged.source_type_description.clear();
    }
    if merged == *original {
        return Ok(None);
    }
    Ok(Some(serde_json::to_value(merged)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileBacking, project, test_support};
    use serde_json::{Value, json};
    use zadt::{ServiceDefinitionProperties, ToXml};

    #[test]
    fn service_definition_maps_discriminators_and_preserves_unrelated_fields() {
        let reference = test_support::reference::<ServiceDefinition>(
            "MANAGEDISTRIBUTIONS",
            "/sap/bc/adt/ddic/srvdsources/managedistributions",
        );
        let fixture = test_support::properties(
            &reference,
            ServiceDefinition::MEDIA_TYPES[0],
            "etag",
            include_bytes!(
                "../../../zadt/tests/fixtures/service-definition-managedistributions.xml"
            ),
        );
        for origin in 0..=9 {
            for source_type in ["S", "X"] {
                let mut original = fixture.properties().clone();
                original.source_origin = origin.to_string();
                original.source_type = source_type.to_owned();
                let snapshot = test_support::properties(
                    &reference,
                    ServiceDefinition::MEDIA_TYPES[0],
                    "etag",
                    &original.to_xml().unwrap(),
                );
                let projection = project(snapshot.into_erased()).unwrap();
                assert!(matches!(
                    projection
                        .file("managedistributions.srvd.acds")
                        .unwrap()
                        .backing(),
                    FileBacking::Source(_)
                ));
                let FileBacking::Properties(mapping) = projection
                    .file("managedistributions.srvd.json")
                    .unwrap()
                    .backing()
                else {
                    panic!()
                };
                let content = mapping.render().unwrap();
                assert!(mapping.merge(&content).unwrap().is_none());
                let mut edited: Value = serde_json::from_str(&content).unwrap();
                edited["header"]["description"] = json!("Changed service");
                let merged: ServiceDefinitionProperties =
                    serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap())
                        .unwrap();
                let mut expected = original;
                expected.description = Some("Changed service".to_owned());
                assert_eq!(merged, expected);
                edited["generalInformation"]["sourceType"] = json!(if source_type == "S" {
                    "extension"
                } else {
                    "definition"
                });
                let merged: ServiceDefinitionProperties =
                    serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap())
                        .unwrap();
                assert_eq!(
                    merged.source_type,
                    if source_type == "S" { "X" } else { "S" }
                );
                assert!(merged.source_type_description.is_empty());
                edited["generalInformation"]
                    .as_object_mut()
                    .unwrap()
                    .remove("sourceOrigin");
                assert!(mapping.merge(&edited.to_string()).is_err());
            }
        }
    }
}
