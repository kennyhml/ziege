//! CDS data definition metadata and advertised `.ddls.acds` source.
//!
//! Header fields map to ADT description, master language, and language version.
//! `sourceOrigin` maps the SAP origin codes 0 through 9. `sourceType` uses the
//! exact mappings in SDDIC_ST_ADT_DDLS, including distinct `view` and `view entity`
//! labels. Display descriptions are not used for classification. Nonempty
//! `parentName` is not serialized by the inspected DDLS transformation and has no
//! implemented backing. Nonempty edits are rejected. Source type is determined by
//! DDIC during activation, so callers refetch after updates to obtain its stored value.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/ddls/ddls-v1.json>.

use crate::{
    Cardinality, CdsHeader, CdsSourceOrigin, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::parse_object,
    models::language_to_adt,
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{DataDefinition, ObjectSnapshot, ObjectType};

pub(crate) static DATA_DEFINITION_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DDLS",
    version: "1",
    workbench_types: &[DataDefinition::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.ddls.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.ddls.acds",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
    ],
};

/// AFF DDLS v1. Required origin/type fields are serialized even at their defaults.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct ProjectedDataDefinitionProperties {
    #[garde(custom(one_of([DATA_DEFINITION_FORMAT.version()])))]
    pub format_version: String,
    #[serde(deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub header: CdsHeader,
    #[serde(deserialize_with = "crate::helpers::string_enum")]
    pub source_origin: CdsSourceOrigin,
    #[serde(deserialize_with = "crate::helpers::string_enum")]
    pub source_type: DataDefinitionSourceType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 40))]
    pub parent_name: String,
}

/// AFF source syntax categories mapped from ADT source-type labels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DataDefinitionSourceType {
    DdicBasedView,
    ViewEntity,
    ViewExtend,
    ViewEntityExtend,
    TableFunction,
    TableEntity,
    AbstractEntity,
    CustomEntity,
    Hierarchy,
    ProjectionView,
    ExternalEntity,
    Unknown,
}

impl DataDefinitionSourceType {
    fn from_adt(value: Option<&str>) -> Result<Self, ProjectionError> {
        match value {
            None | Some("") => Ok(Self::Unknown),
            Some("view") => Ok(Self::DdicBasedView),
            Some("view entity") => Ok(Self::ViewEntity),
            Some("extend") => Ok(Self::ViewExtend),
            Some("view entity extend") => Ok(Self::ViewEntityExtend),
            Some("table function") => Ok(Self::TableFunction),
            Some("table entity") => Ok(Self::TableEntity),
            Some("abstract entity") => Ok(Self::AbstractEntity),
            Some("custom entity") => Ok(Self::CustomEntity),
            Some("hierarchy") => Ok(Self::Hierarchy),
            Some("projection view") => Ok(Self::ProjectionView),
            Some("external entity") => Ok(Self::ExternalEntity),
            Some(value) => Err(ProjectionError::InvalidAffField {
                field: "sourceType",
                message: format!("unsupported ADT source type `{value}`"),
            }),
        }
    }

    const fn adt_value(self) -> Option<&'static str> {
        match self {
            Self::Unknown => None,
            Self::DdicBasedView => Some("view"),
            Self::ViewEntity => Some("view entity"),
            Self::ViewExtend => Some("extend"),
            Self::ViewEntityExtend => Some("view entity extend"),
            Self::TableFunction => Some("table function"),
            Self::TableEntity => Some("table entity"),
            Self::AbstractEntity => Some("abstract entity"),
            Self::CustomEntity => Some("custom entity"),
            Self::Hierarchy => Some("hierarchy"),
            Self::ProjectionView => Some("projection view"),
            Self::ExternalEntity => Some("external entity"),
        }
    }
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<DataDefinition>()?;
    let document = ProjectedDataDefinitionProperties {
        format_version: DATA_DEFINITION_FORMAT.version().to_owned(),
        header: CdsHeader::from_adt(
            properties.description.as_deref().unwrap_or_default(),
            &properties.master_language,
            &properties.abap_language_version,
        )?,
        source_origin: CdsSourceOrigin::from_adt(&properties.source_origin, "sourceOrigin")?,
        source_type: DataDefinitionSourceType::from_adt(properties.source_type.as_deref())?,
        parent_name: String::new(),
    };
    document.validate()?;
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<DataDefinition>()?;
    let edited: ProjectedDataDefinitionProperties = parse_object(edited)?;
    edited.validate()?;
    if !edited.parent_name.is_empty() {
        return Err(ProjectionError::UnsupportedAffProperty {
            object_type: "DDLS",
            field: "parentName",
        });
    }
    let previous = CdsHeader::from_adt(
        original.description.as_deref().unwrap_or_default(),
        &original.master_language,
        &original.abap_language_version,
    )?;
    let origin = CdsSourceOrigin::from_adt(&original.source_origin, "sourceOrigin")?;
    let mut merged = original.clone();
    if edited.source_type != DataDefinitionSourceType::from_adt(original.source_type.as_deref())? {
        merged.source_type = edited.source_type.adt_value().map(str::to_owned);
        merged.source_type_description = None;
    }
    if edited.header.description != original.description.as_deref().unwrap_or_default() {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    if edited.header.abap_language_version != previous.abap_language_version {
        merged.abap_language_version = edited.header.abap_language_version.to_adt_ddic();
    }
    if edited.source_origin != origin {
        merged.source_origin = edited.source_origin.adt_value().to_owned();
        // A description of the old origin must not accompany the new code.
        merged.source_origin_description.clear();
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
    use zadt::DataDefinitionProperties;

    #[test]
    fn data_definition_preserves_semantic_type_and_rejects_unbacked_fields() {
        let reference = test_support::reference::<DataDefinition>(
            "I_BUSINESSPARTNER",
            "/sap/bc/adt/ddic/ddlsources/i_businesspartner",
        );
        let snapshot = test_support::properties(
            &reference,
            DataDefinition::MEDIA_TYPES[0],
            "etag",
            include_bytes!("../../../zadt/tests/fixtures/data-definition-i-businesspartner.xml"),
        );
        let original = snapshot.properties().clone();
        let projection = project(snapshot.into_erased()).unwrap();
        assert_eq!(projection.files().len(), 2);
        assert!(matches!(
            projection
                .file("i_businesspartner.ddls.acds")
                .unwrap()
                .backing(),
            FileBacking::Source(_)
        ));
        let FileBacking::Properties(mapping) = projection
            .file("i_businesspartner.ddls.json")
            .unwrap()
            .backing()
        else {
            panic!()
        };
        let content = mapping.render().unwrap();
        assert!(mapping.merge(&content).unwrap().is_none());
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(edited["sourceType"], "ddicBasedView");
        assert_eq!(edited["sourceOrigin"], "abapDevelopmentTools");
        edited["header"]["description"] = json!("Changed data definition");
        let merged: DataDefinitionProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        let mut expected = original;
        expected.description = Some("Changed data definition".to_owned());
        assert_eq!(merged, expected);
        let mut invalid = edited.clone();
        invalid["parentName"] = json!("I_PARENT");
        assert!(matches!(
            mapping.merge(&invalid.to_string()),
            Err(ProjectionError::UnsupportedAffProperty { .. })
        ));
        for (aff, adt) in [
            ("ddicBasedView", Some("view")),
            ("viewEntity", Some("view entity")),
            ("viewExtend", Some("extend")),
            ("viewEntityExtend", Some("view entity extend")),
            ("tableFunction", Some("table function")),
            ("tableEntity", Some("table entity")),
            ("abstractEntity", Some("abstract entity")),
            ("customEntity", Some("custom entity")),
            ("hierarchy", Some("hierarchy")),
            ("projectionView", Some("projection view")),
            ("externalEntity", Some("external entity")),
            ("unknown", None),
        ] {
            let mut edit = edited.clone();
            edit["sourceType"] = json!(aff);
            let merged: DataDefinitionProperties =
                serde_json::from_value(mapping.merge(&edit.to_string()).unwrap().unwrap()).unwrap();
            assert_eq!(merged.source_type.as_deref(), adt);
            if aff != "ddicBasedView" {
                assert!(merged.source_type_description.is_none());
            }
            assert_eq!(
                serde_json::to_value(DataDefinitionSourceType::from_adt(adt).unwrap()).unwrap(),
                aff
            );
        }
        edited["sourceOrigin"] = json!("customCdsViews");
        let merged: DataDefinitionProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        assert_eq!(merged.source_origin, "1");
        assert!(merged.source_origin_description.is_empty());
        edited.as_object_mut().unwrap().remove("sourceType");
        assert!(mapping.merge(&edited.to_string()).is_err());
    }
}
