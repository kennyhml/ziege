//! CDS annotation definition metadata and advertised `.ddla.acds` source.
//!
//! AFF `header.description` maps to ADT `description`; `originalLanguage` maps
//! to `master_language` with SAP/BCP47 conversion. DDLA has no language-version
//! field. Merge copies the complete ADT baseline before changing these two fields.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/ddla/ddla-v1.json>.

use crate::{
    Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::parse_object,
    models::{language_from_adt, language_to_adt},
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{AnnotationDefinition, ObjectSnapshot, ObjectType};

pub(crate) static ANNOTATION_DEFINITION_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DDLA",
    version: "1",
    workbench_types: &[AnnotationDefinition::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.ddla.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.ddla.acds",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
    ],
};

/// AFF DDLA v1 metadata, independent of the annotation source text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedAnnotationDefinitionProperties {
    #[garde(custom(one_of([ANNOTATION_DEFINITION_FORMAT.version()])))]
    pub format_version: String,
    #[serde(deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub header: AnnotationDefinitionHeader,
}

/// Description and original language; language versions are not part of DDLA v1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnotationDefinitionHeader {
    #[garde(length(chars, max = 60))]
    pub description: String,
    #[garde(length(chars, min = 2))]
    pub original_language: String,
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<AnnotationDefinition>()?;
    let document = ProjectedAnnotationDefinitionProperties {
        format_version: ANNOTATION_DEFINITION_FORMAT.version().to_owned(),
        header: AnnotationDefinitionHeader {
            description: properties.description.clone().unwrap_or_default(),
            original_language: language_from_adt(
                &properties.master_language,
                "header.originalLanguage",
            )?,
        },
    };
    document.validate()?;
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<AnnotationDefinition>()?;
    let edited: ProjectedAnnotationDefinitionProperties = parse_object(edited)?;
    edited.validate()?;
    let mut merged = original.clone();
    if edited.header.description != original.description.as_deref().unwrap_or_default() {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
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
    use zadt::AnnotationDefinitionProperties;

    #[test]
    fn annotation_definition_roundtrip_and_edits_preserve_the_baseline() {
        let reference = test_support::reference::<AnnotationDefinition>(
            "UI",
            "/sap/bc/adt/ddic/ddlasources/ui",
        );
        let snapshot = test_support::properties(
            &reference,
            AnnotationDefinition::MEDIA_TYPES[0],
            "etag",
            include_bytes!("../../../zadt/tests/fixtures/annotation-definition-ui.xml"),
        );
        let original = snapshot.properties().clone();
        let projection = project(snapshot.into_erased()).unwrap();
        assert_eq!(projection.files().len(), 2);
        assert!(matches!(
            projection.file("ui.ddla.acds").unwrap().backing(),
            FileBacking::Source(_)
        ));
        let FileBacking::Properties(mapping) = projection.file("ui.ddla.json").unwrap().backing()
        else {
            panic!()
        };
        let content = mapping.render().unwrap();
        assert!(mapping.merge(&content).unwrap().is_none());
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        edited["header"]["description"] = json!("Changed annotations");
        edited["header"]["originalLanguage"] = json!("de");
        let merged: AnnotationDefinitionProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        let mut expected = original;
        expected.description = Some("Changed annotations".to_owned());
        expected.master_language = "DE".to_owned();
        assert_eq!(merged, expected);
        edited["header"]["abapLanguageVersion"] = json!("standard");
        assert!(mapping.merge(&edited.to_string()).is_err());
        edited["header"]
            .as_object_mut()
            .unwrap()
            .remove("abapLanguageVersion");
        edited["header"]["description"] = json!("x".repeat(61));
        assert!(mapping.merge(&edited.to_string()).is_err());
    }
}
