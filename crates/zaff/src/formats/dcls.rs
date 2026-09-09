//! CDS access-control metadata and advertised `.dcls.acds` source.
//!
//! AFF header fields map to ADT `description`, `master_language` (SAP/BCP47),
//! and `abap_language_version` (Standard `0`). Source and other properties are preserved.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/dcls/dcls-v1.json>.

use crate::{
    Cardinality, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping},
    helpers::parse_object,
    models::{CdsHeader, language_to_adt},
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{AccessControl, ObjectSnapshot, ObjectType};

pub(crate) static ACCESS_CONTROL_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DCLS",
    version: "1",
    workbench_types: &[AccessControl::WORKBENCH_TYPE],
    files: &[
        FileSpec::new(
            "<name>.dcls.json",
            Cardinality::One,
            Mapping::Properties(PropertiesMapping { render, merge }),
        ),
        FileSpec::new(
            "<name>.dcls.acds",
            Cardinality::One,
            Mapping::Source { component: None },
        ),
    ],
};

/// AFF DCLS v1 metadata. Access-control source is bound independently.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedAccessControlProperties {
    #[garde(custom(one_of([ACCESS_CONTROL_FORMAT.version()])))]
    pub format_version: String,
    #[garde(dive)]
    pub header: CdsHeader,
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    let properties = obj.typed_properties::<AccessControl>()?;
    let document = ProjectedAccessControlProperties {
        format_version: ACCESS_CONTROL_FORMAT.version().to_owned(),
        header: CdsHeader::from_adt(
            &properties.description,
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
    let original = obj.typed_properties::<AccessControl>()?;
    let edited: ProjectedAccessControlProperties = parse_object(edited)?;
    edited.validate()?;
    let previous = CdsHeader::from_adt(
        &original.description,
        &original.master_language,
        &original.abap_language_version,
    )?;
    let mut merged = original.clone();
    merged.description = edited.header.description;
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
    use zadt::{AccessControlProperties, ToXml};

    #[test]
    fn access_control_preserves_language_spellings_and_unrepresented_fields() {
        let reference = test_support::reference::<AccessControl>(
            "SDSH_CDS_DOMAIN_VAL_DCL",
            "/sap/bc/adt/acm/dclsources/sdsh_cds_domain_val_dcl",
        );
        let fixture = test_support::properties(
            &reference,
            AccessControl::MEDIA_TYPES[0],
            "etag",
            include_bytes!(
                "../../../zadt/tests/fixtures/access-control-sdsh-cds-domain-val-dcl.xml"
            ),
        );
        for version in ["0", "", " ", "2", "5"] {
            let mut original = fixture.properties().clone();
            original.abap_language_version = zadt::AbapLanguageVersion::Other(version.to_owned());
            let snapshot = test_support::properties(
                &reference,
                AccessControl::MEDIA_TYPES[0],
                "etag",
                &original.to_xml().unwrap(),
            );
            let original = snapshot.properties().clone();
            let projection = project(snapshot.into_erased()).unwrap();
            assert_eq!(projection.files().len(), 2);
            assert!(matches!(
                projection
                    .file("sdsh_cds_domain_val_dcl.dcls.acds")
                    .unwrap()
                    .backing(),
                FileBacking::Source(_)
            ));
            let FileBacking::Properties(mapping) = projection
                .file("sdsh_cds_domain_val_dcl.dcls.json")
                .unwrap()
                .backing()
            else {
                panic!()
            };
            let content = mapping.render().unwrap();
            assert!(mapping.merge(&content).unwrap().is_none());
            let mut edited: Value = serde_json::from_str(&content).unwrap();
            edited["header"]["description"] = json!("Changed access control");
            let merged: AccessControlProperties =
                serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap())
                    .unwrap();
            let mut expected = original;
            expected.description = "Changed access control".to_owned();
            assert_eq!(merged, expected);
            edited["header"]["abapLanguageVersion"] = json!("standard");
            let merged: AccessControlProperties =
                serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap())
                    .unwrap();
            assert_eq!(
                merged.abap_language_version.as_str(),
                if matches!(version, "2" | "5") {
                    "0"
                } else {
                    version
                }
            );
            edited["header"]["description"] = json!("x".repeat(61));
            assert!(mapping.merge(&edited.to_string()).is_err());
        }
    }
}
