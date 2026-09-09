//! Dictionary domain mapping to AFF `.doma.json`.
//!
//! ```text
//! AFF                         ADT
//! header                      description, master_language, abap_language_version
//! format                      content.type_information
//! outputCharacteristics       content.output_information
//! valueTable                  content.value_information.value_table.name
//! fixedValues                 fixed_values entries with an empty high value
//! fixedValueIntervals         fixed_values entries with a nonempty high value
//! fixedValueAppends           No name list exposed by ZADT
//! ```
//!
//! Unchanged dimensions retain zero padding. Unchanged fixed-value lists retain
//! their original positions and interleaving; edits replace entries in their
//! existing single/interval slots and append new entries after the last position.
//! Value-table references retain metadata until their names change. The append
//! flag is preserved, but nonempty AFF append lists cannot be merged.
//! Schema: <https://github.com/SAP/abap-file-formats/blob/main/file-formats/doma/doma-v1.json>.

use crate::{
    Cardinality, CdsHeader, FileSpec, ObjectFormat, ProjectionError,
    formats::{Mapping, PropertiesMapping, dtel::DATA_TYPES},
    helpers::{is_false, parse_object},
    models::language_to_adt,
    validate::one_of,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use zadt::{
    AdvertisedObjectReference, Domain, DomainFixedValue, DomainFixedValues, DomainProperties,
    DomainValueInformation, ObjectSnapshot, ObjectType,
};

pub(crate) static DOMAIN_FORMAT: ObjectFormat = ObjectFormat {
    object_type: "DOMA",
    version: "1",
    workbench_types: &[Domain::WORKBENCH_TYPE],
    files: &[FileSpec::new(
        "<name>.doma.json",
        Cardinality::One,
        Mapping::Properties(PropertiesMapping { render, merge }),
    )],
};

/// AFF DOMA v1 metadata, with separate single-value and interval collections.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectedDomainProperties {
    #[garde(custom(one_of([DOMAIN_FORMAT.version()])))]
    pub format_version: String,
    #[garde(dive)]
    pub header: CdsHeader,
    #[garde(dive)]
    pub format: DomainFormat,
    #[serde(default)]
    #[garde(dive)]
    pub output_characteristics: DomainOutputCharacteristics,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive)]
    pub fixed_values: Vec<DomainSingleValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive)]
    pub fixed_value_intervals: Vec<DomainValueInterval>,
    #[serde(default, skip_serializing_if = "DomainNamedObject::is_empty")]
    #[garde(dive)]
    pub value_table: DomainNamedObject,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(dive)]
    pub fixed_value_appends: Vec<DomainNamedObject>,
}

/// Storage datatype and numeric dimensions, written as six-digit ADT strings on edits.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainFormat {
    #[garde(custom(one_of(DATA_TYPES)))]
    pub data_type: String,
    #[garde(range(max = 999999))]
    pub length: u32,
    #[serde(default)]
    #[garde(range(max = 999999))]
    pub decimals: u32,
}

/// ADT output style codes 00..06 and the remaining display properties.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct DomainOutputCharacteristics {
    #[serde(default)]
    pub style: DomainOutputStyle,
    #[serde(default)]
    #[garde(range(max = 999999))]
    pub length: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[garde(length(chars, max = 5))]
    pub conversion_routine: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub case_sensitive: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub negative_values: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub am_pm_time_format: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DomainOutputStyle {
    #[default]
    Normal,
    SignRight,
    ScalePreserving,
    Scientific,
    ScientificWithLeadingZero,
    ScalePreservingScientific,
    Engineering,
}

impl DomainOutputStyle {
    fn from_adt(value: &str) -> Result<Self, ProjectionError> {
        match value {
            "00" | "0" | "" => Ok(Self::Normal),
            "01" | "1" => Ok(Self::SignRight),
            "02" | "2" => Ok(Self::ScalePreserving),
            "03" | "3" => Ok(Self::Scientific),
            "04" | "4" => Ok(Self::ScientificWithLeadingZero),
            "05" | "5" => Ok(Self::ScalePreservingScientific),
            "06" | "6" => Ok(Self::Engineering),
            _ => Err(invalid("outputCharacteristics.style", value)),
        }
    }
    fn adt_value(self) -> &'static str {
        match self {
            Self::Normal => "00",
            Self::SignRight => "01",
            Self::ScalePreserving => "02",
            Self::Scientific => "03",
            Self::ScientificWithLeadingZero => "04",
            Self::ScalePreservingScientific => "05",
            Self::Engineering => "06",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainSingleValue {
    #[serde(default)]
    #[garde(length(chars, max = 10))]
    pub fixed_value: String,
    #[serde(default)]
    #[garde(length(chars, max = 60))]
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainValueInterval {
    #[serde(default)]
    #[garde(length(chars, max = 10))]
    pub low_limit: String,
    #[garde(length(chars, max = 10))]
    pub high_limit: String,
    #[serde(default)]
    #[garde(length(chars, max = 60))]
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainNamedObject {
    #[serde(default)]
    #[garde(length(chars, max = 30))]
    pub name: String,
}
impl DomainNamedObject {
    fn is_empty(&self) -> bool {
        self.name.is_empty()
    }
}

fn invalid(field: &'static str, value: &str) -> ProjectionError {
    ProjectionError::InvalidAffField {
        field,
        message: format!("unsupported ADT value `{value}`"),
    }
}

fn number(value: &str, field: &'static str) -> Result<u32, ProjectionError> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(field, value));
    }
    value.parse().map_err(|_| invalid(field, value))
}

impl ProjectedDomainProperties {
    fn from_adt(properties: &DomainProperties) -> Result<Self, ProjectionError> {
        let storage = &properties.content.type_information;
        let output = &properties.content.output_information;
        let mut document = Self {
            format_version: DOMAIN_FORMAT.version().to_owned(),
            header: CdsHeader::from_adt(
                &properties.description,
                &properties.master_language,
                &properties.abap_language_version,
            )?,
            format: DomainFormat {
                data_type: storage.datatype.clone(),
                length: number(&storage.length, "format.length")?,
                decimals: number(&storage.decimals, "format.decimals")?,
            },
            output_characteristics: DomainOutputCharacteristics {
                style: DomainOutputStyle::from_adt(&output.style)?,
                length: number(&output.length, "outputCharacteristics.length")?,
                conversion_routine: output.conversion_exit.clone(),
                case_sensitive: output.lowercase,
                negative_values: output.sign_exists,
                am_pm_time_format: output.ampm_format,
            },
            fixed_values: Vec::new(),
            fixed_value_intervals: Vec::new(),
            value_table: DomainNamedObject::default(),
            fixed_value_appends: Vec::new(),
        };
        if let Some(values) = &properties.content.value_information {
            document.value_table.name = values.value_table.name.clone().unwrap_or_default();
            for value in &values.fixed_values.values {
                if value.high.is_empty() {
                    document.fixed_values.push(DomainSingleValue {
                        fixed_value: value.low.clone(),
                        description: value.text.clone(),
                    });
                } else {
                    document.fixed_value_intervals.push(DomainValueInterval {
                        low_limit: value.low.clone(),
                        high_limit: value.high.clone(),
                        description: value.text.clone(),
                    });
                }
            }
        }
        document.validate()?;
        Ok(document)
    }
}

fn render(obj: &ObjectSnapshot<()>) -> Result<String, ProjectionError> {
    Ok(
        serde_json::to_string_pretty(&ProjectedDomainProperties::from_adt(
            obj.typed_properties::<Domain>()?,
        )?)? + "\n",
    )
}

fn merge(
    obj: &ObjectSnapshot<()>,
    edited: &str,
) -> Result<Option<serde_json::Value>, ProjectionError> {
    let original = obj.typed_properties::<Domain>()?;
    let edited: ProjectedDomainProperties = parse_object(edited)?;
    edited.validate()?;
    if !edited.fixed_value_appends.is_empty() {
        return Err(ProjectionError::UnsupportedAffProperty {
            object_type: "DOMA",
            field: "fixedValueAppends",
        });
    }
    if edited
        .fixed_value_intervals
        .iter()
        .any(|v| v.high_limit.is_empty())
    {
        return Err(ProjectionError::InvalidAffField {
            field: "fixedValueIntervals.highLimit",
            message: "ADT represents an empty high value as a single value".to_owned(),
        });
    }
    let previous = ProjectedDomainProperties::from_adt(original)?;
    let mut merged = original.clone();
    merged.description = edited.header.description;
    merged.master_language =
        language_to_adt(&edited.header.original_language, "header.originalLanguage")?;
    if edited.header.abap_language_version != previous.header.abap_language_version {
        merged.abap_language_version = edited.header.abap_language_version.to_adt_ddic();
    }
    let storage = &mut merged.content.type_information;
    storage.datatype = edited.format.data_type;
    if edited.format.length != previous.format.length {
        storage.length = format!("{:06}", edited.format.length);
    }
    if edited.format.decimals != previous.format.decimals {
        storage.decimals = format!("{:06}", edited.format.decimals);
    }
    let output = &mut merged.content.output_information;
    let e = &edited.output_characteristics;
    let p = &previous.output_characteristics;
    if e.style != p.style {
        output.style = e.style.adt_value().to_owned();
    }
    if e.length != p.length {
        output.length = format!("{:06}", e.length);
    }
    output.conversion_exit = e.conversion_routine.clone();
    output.lowercase = e.case_sensitive;
    output.sign_exists = e.negative_values;
    output.ampm_format = e.am_pm_time_format;
    let values_changed = edited.fixed_values != previous.fixed_values
        || edited.fixed_value_intervals != previous.fixed_value_intervals;
    if edited.value_table != previous.value_table || values_changed {
        let values =
            merged
                .content
                .value_information
                .get_or_insert_with(|| DomainValueInformation {
                    value_table: AdvertisedObjectReference::default(),
                    append_exists: false,
                    fixed_values: DomainFixedValues { values: Vec::new() },
                });
        if edited.value_table != previous.value_table {
            values.value_table = AdvertisedObjectReference {
                name: (!edited.value_table.name.is_empty()).then_some(edited.value_table.name),
                ..Default::default()
            };
        }
        if values_changed {
            values.fixed_values.values = merge_values(
                &values.fixed_values.values,
                &edited.fixed_values,
                &edited.fixed_value_intervals,
            )?;
        }
    }
    if merged == *original {
        return Ok(None);
    }
    Ok(Some(serde_json::to_value(merged)?))
}

fn merge_values(
    original: &[DomainFixedValue],
    singles: &[DomainSingleValue],
    intervals: &[DomainValueInterval],
) -> Result<Vec<DomainFixedValue>, ProjectionError> {
    let mut singles = singles
        .iter()
        .map(|v| (v.fixed_value.clone(), String::new(), v.description.clone()));
    let mut intervals = intervals.iter().map(|v| {
        (
            v.low_limit.clone(),
            v.high_limit.clone(),
            v.description.clone(),
        )
    });
    let mut result = Vec::new();
    let mut position = 0;
    for old in original {
        position = position.max(number(&old.position, "fixedValues.position")?);
        let next = if old.high.is_empty() {
            singles.next()
        } else {
            intervals.next()
        };
        if let Some((low, high, text)) = next {
            result.push(DomainFixedValue {
                position: old.position.clone(),
                low,
                high,
                text,
            });
        }
    }
    for (low, high, text) in singles.chain(intervals) {
        position = position
            .checked_add(1)
            .filter(|p| *p <= 9999)
            .ok_or_else(|| invalid("fixedValues.position", "position overflow"))?;
        result.push(DomainFixedValue {
            position: format!("{position:04}"),
            low,
            high,
            text,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileBacking, project, test_support};
    use serde_json::{Value, json};
    use zadt::ToXml;

    fn snapshot(name: &str, xml: &[u8]) -> ObjectSnapshot<Domain> {
        let reference = test_support::reference::<Domain>(
            name,
            &format!("/sap/bc/adt/ddic/domains/{}", name.to_ascii_lowercase()),
        );
        test_support::properties(&reference, Domain::MEDIA_TYPES[0], "etag", xml)
    }

    #[test]
    fn domain_fixtures_preserve_wire_padding_references_and_fixed_values() {
        for (name, xml) in [
            (
                "XFELD",
                include_bytes!("../../../zadt/tests/fixtures/domain-xfeld.xml").as_slice(),
            ),
            (
                "TRKORR",
                include_bytes!("../../../zadt/tests/fixtures/domain-trkorr.xml").as_slice(),
            ),
        ] {
            let snapshot = snapshot(name, xml);
            let original = snapshot.properties().clone();
            let projection = project(snapshot.into_erased()).unwrap();
            assert_eq!(projection.files().len(), 1);
            let FileBacking::Properties(mapping) = projection.files()[0].backing() else {
                panic!()
            };
            let content = mapping.render().unwrap();
            assert!(mapping.merge(&content).unwrap().is_none());
            let mut edited: Value = serde_json::from_str(&content).unwrap();
            edited["header"]["description"] = json!("Changed domain");
            let merged: DomainProperties =
                serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap())
                    .unwrap();
            let mut expected = original;
            expected.description = "Changed domain".to_owned();
            assert_eq!(merged, expected);
            edited["format"]["length"] = json!(23);
            edited["valueTable"] = json!({"name":"Z_NEW"});
            let merged: DomainProperties =
                serde_json::from_value(mapping.merge(&edited.to_string()).unwrap().unwrap())
                    .unwrap();
            assert_eq!(merged.content.type_information.length, "000023");
            assert_eq!(
                merged.content.value_information.unwrap().value_table,
                AdvertisedObjectReference {
                    name: Some("Z_NEW".to_owned()),
                    ..Default::default()
                }
            );
            edited["fixedValueAppends"] = json!([{"name":"Z_APPEND"}]);
            assert!(matches!(
                mapping.merge(&edited.to_string()),
                Err(ProjectionError::UnsupportedAffProperty {
                    field: "fixedValueAppends",
                    ..
                })
            ));
        }
    }

    #[test]
    fn fixed_value_edits_preserve_interleaving_and_roundtrip() {
        let mut original = snapshot(
            "XFELD",
            include_bytes!("../../../zadt/tests/fixtures/domain-xfeld.xml"),
        )
        .properties()
        .clone();
        let values = original.content.value_information.as_mut().unwrap();
        values.append_exists = true;
        values.fixed_values.values.insert(
            1,
            DomainFixedValue {
                position: "0010".to_owned(),
                low: "A".to_owned(),
                high: "Z".to_owned(),
                text: "Letters".to_owned(),
            },
        );
        let snapshot = snapshot("XFELD", &original.to_xml().unwrap()).into_erased();
        let mut edited = ProjectedDomainProperties::from_adt(&original).unwrap();
        edited.fixed_values[0].description = "Yes".to_owned();
        edited.fixed_values.push(DomainSingleValue {
            fixed_value: "?".to_owned(),
            description: "Unknown".to_owned(),
        });
        let merged: DomainProperties = serde_json::from_value(
            merge(&snapshot, &serde_json::to_string(&edited).unwrap())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let values = merged.content.value_information.as_ref().unwrap();
        assert!(values.append_exists);
        assert_eq!(
            values.fixed_values.values[1],
            original
                .content
                .value_information
                .as_ref()
                .unwrap()
                .fixed_values
                .values[1]
        );
        assert_eq!(values.fixed_values.values.last().unwrap().position, "0011");
        assert_eq!(
            ProjectedDomainProperties::from_adt(&merged).unwrap(),
            edited
        );
        edited.fixed_value_intervals[0].high_limit.clear();
        assert!(merge(&snapshot, &serde_json::to_string(&edited).unwrap()).is_err());
    }
}
