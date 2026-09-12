//! CDS metadata extension metadata and advertised `.ddlx.acds` source.
//!
//! AFF header fields map to ADT `description`, `master_language` (SAP/BCP47),
//! and `abap_language_version` (Standard `0`). Other properties are preserved.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/ddlx/ddlx-v1.json>.

use crate::{
    Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::parse_object,
    models::{CdsHeader, language_to_adt},
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{MetadataExtension, ObjectSnapshot, ObjectType};

pub(crate) static METADATA_EXTENSION_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DDLX",
    version: "1",
    workbench_types: &[MetadataExtension::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.ddlx.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.ddlx.acds",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
    ],
};

/// AFF DDLX v1 metadata. Source text is projected independently.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedMetadataExtensionProperties {
    #[garde(custom(one_of([METADATA_EXTENSION_FORMAT.version()])))]
    pub format_version: String,
    #[serde(deserialize_with = "crate::helpers::object")]
    #[garde(dive)]
    pub header: CdsHeader,
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<MetadataExtension>()?;
    let document = ProjectedMetadataExtensionProperties {
        format_version: METADATA_EXTENSION_FORMAT.version().to_owned(),
        header: CdsHeader::from_adt(
            properties.description.as_deref().unwrap_or_default(),
            &properties.master_language,
            &properties.abap_language_version,
        )?,
    };
    document.validate()?;
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<MetadataExtension>()?;
    let edited: ProjectedMetadataExtensionProperties = parse_object(edited)?;
    edited.validate()?;
    let previous = CdsHeader::from_adt(
        original.description.as_deref().unwrap_or_default(),
        &original.master_language,
        &original.abap_language_version,
    )?;
    let mut merged = original.clone();
    if edited.header.description != original.description.as_deref().unwrap_or_default() {
        merged.description = Some(edited.header.description);
    }
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    if edited.header.abap_language_version != previous.abap_language_version {
        merged.abap_language_version = edited.header.abap_language_version.to_adt_ddic();
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
    use zadt::MetadataExtensionProperties;

    #[test]
    fn metadata_extension_roundtrip_and_edits_preserve_the_baseline() {
        let reference = test_support::reference::<MetadataExtension>(
            "C_MDOAPPLICATIONSCOPE",
            "/sap/bc/adt/ddic/ddlxsources/c_mdoapplicationscope",
        );
        let snapshot = test_support::properties(
            &reference,
            MetadataExtension::MEDIA_TYPES[0],
            "etag",
            include_bytes!(
                "../../../zadt/tests/fixtures/metadata-extension-c-mdoapplicationscope.xml"
            ),
        );
        let original = snapshot.properties().clone();
        let projection = project(snapshot.into_erased()).unwrap();
        assert_eq!(projection.files().len(), 2);
        assert!(matches!(
            projection
                .file("c_mdoapplicationscope.ddlx.acds")
                .unwrap()
                .backing(),
            FileBacking::Source(_)
        ));
        let FileBacking::Properties(mapping) = projection
            .file("c_mdoapplicationscope.ddlx.json")
            .unwrap()
            .backing()
        else {
            panic!()
        };
        let content = mapping.render().unwrap();
        assert!(mapping.merge(&content).unwrap().is_none());
        let mut edited: Value = serde_json::from_str(&content).unwrap();
        edited["header"]["description"] = json!("Changed metadata extension");
        edited["header"]["originalLanguage"] = json!("de");
        edited["header"]["abapLanguageVersion"] = json!("cloudDevelopment");
        let merged: MetadataExtensionProperties =
            serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap()).unwrap();
        let mut expected = original;
        expected.description = Some("Changed metadata extension".to_owned());
        expected.master_language = "DE".to_owned();
        expected.abap_language_version = zadt::AbapLanguageVersion::CloudDevelopment;
        assert_eq!(merged, expected);
        edited["header"]["originalLanguage"] = json!("xx-invalid");
        assert!(mapping.merge(&edited.to_string()).is_err());
        edited["formatVersion"] = json!("2");
        assert!(mapping.merge(&edited.to_string()).is_err());
    }
}
