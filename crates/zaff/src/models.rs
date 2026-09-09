use serde::{Deserialize, Serialize};
use zadt::AbapLanguageVersion as AdtAbapLanguageVersion;

use crate::ProjectionError;

/// Common CDS AFF header: description, original language, and ABAP language version.
/// ADT uses SAP language codes and DDIC Standard `0`. Unchanged versions retain
/// their original spelling when merged by each format.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, garde::Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[garde(allow_unvalidated)]
pub struct CdsHeader {
    #[garde(length(chars, max = 60))]
    pub description: String,
    #[garde(length(chars, min = 2))]
    pub original_language: String,
    #[serde(default, skip_serializing_if = "AbapLanguageVersion::is_standard")]
    pub abap_language_version: AbapLanguageVersion,
}

impl CdsHeader {
    pub(crate) fn from_adt(
        description: &str,
        language: &str,
        version: &AdtAbapLanguageVersion,
    ) -> Result<Self, ProjectionError> {
        Ok(Self {
            description: description.to_owned(),
            original_language: language_from_adt(language, "header.originalLanguage")?,
            abap_language_version: AbapLanguageVersion::from_adt(Some(version), "0").map_err(
                |value| ProjectionError::InvalidAffField {
                    field: "header.abapLanguageVersion",
                    message: format!("unsupported ADT value `{value}`"),
                },
            )?,
        })
    }
}

/// CDS creation tools, mapped to SAP's source-origin codes `0` through `9`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CdsSourceOrigin {
    AbapDevelopmentTools,
    CustomCdsViews,
    CustomAnalyticalQueries,
    CustomBusinessObject,
    CustomCodeList,
    CustomCdsViewsVariantConfg,
    CustomFields,
    ExtensionsForDataSources,
    CustomSearchModeler,
    ServiceConsumptionModel,
}

impl CdsSourceOrigin {
    pub(crate) fn from_adt(value: &str, field: &'static str) -> Result<Self, ProjectionError> {
        match value {
            "0" => Ok(Self::AbapDevelopmentTools),
            "1" => Ok(Self::CustomCdsViews),
            "2" => Ok(Self::CustomAnalyticalQueries),
            "3" => Ok(Self::CustomBusinessObject),
            "4" => Ok(Self::CustomCodeList),
            "5" => Ok(Self::CustomCdsViewsVariantConfg),
            "6" => Ok(Self::CustomFields),
            "7" => Ok(Self::ExtensionsForDataSources),
            "8" => Ok(Self::CustomSearchModeler),
            "9" => Ok(Self::ServiceConsumptionModel),
            _ => Err(ProjectionError::InvalidAffField {
                field,
                message: format!("unsupported ADT value `{value}`"),
            }),
        }
    }

    pub(crate) fn adt_value(self) -> &'static str {
        match self {
            Self::AbapDevelopmentTools => "0",
            Self::CustomCdsViews => "1",
            Self::CustomAnalyticalQueries => "2",
            Self::CustomBusinessObject => "3",
            Self::CustomCodeList => "4",
            Self::CustomCdsViewsVariantConfg => "5",
            Self::CustomFields => "6",
            Self::ExtensionsForDataSources => "7",
            Self::CustomSearchModeler => "8",
            Self::ServiceConsumptionModel => "9",
        }
    }
}

/// AFF's common ABAP language-version vocabulary.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AbapLanguageVersion {
    #[default]
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "keyUser")]
    KeyUser,
    #[serde(rename = "cloudDevelopment")]
    CloudDevelopment,
}

impl AbapLanguageVersion {
    /// Converts from ADT using the family's Standard spelling, such as `"X"` or `"0"`.
    /// Absent and blank values also mean Standard; errors return the unsupported wire value.
    pub fn from_adt<'a>(
        value: Option<&'a AdtAbapLanguageVersion>,
        standard: &str,
    ) -> Result<Self, &'a str> {
        match value.map(AdtAbapLanguageVersion::as_str) {
            None | Some("" | " ") => Ok(Self::Standard),
            Some("2") => Ok(Self::KeyUser),
            Some("5") => Ok(Self::CloudDevelopment),
            Some(value) if value == standard => Ok(Self::Standard),
            Some(value) => Err(value),
        }
    }

    /// Converts to ADT using the supplied family-specific representation of Standard.
    pub fn to_adt(self, standard: AdtAbapLanguageVersion) -> AdtAbapLanguageVersion {
        match self {
            Self::Standard => standard,
            Self::KeyUser => AdtAbapLanguageVersion::KeyUser,
            Self::CloudDevelopment => AdtAbapLanguageVersion::CloudDevelopment,
        }
    }

    /// Converts to ADT with Standard encoded as `"X"` for REPS-style objects.
    pub fn to_adt_reps(self) -> AdtAbapLanguageVersion {
        self.to_adt(AdtAbapLanguageVersion::StandardX)
    }

    /// Converts to ADT with Standard encoded as `"0"` for DDIC-style objects.
    pub fn to_adt_ddic(self) -> AdtAbapLanguageVersion {
        self.to_adt(AdtAbapLanguageVersion::Other("0".to_owned()))
    }

    pub(crate) const fn is_standard(&self) -> bool {
        matches!(self, Self::Standard)
    }
}

const LANGUAGES: &[(&str, &str)] = &[
    ("af", "AF"),
    ("sq", "SQ"),
    ("eu", "EU"),
    ("bs", "BS"),
    ("bg", "BG"),
    ("ca", "CA"),
    ("zh", "ZH"),
    ("zh-SG", "3C"),
    ("zh-Hant", "ZF"),
    ("hr", "HR"),
    ("cs", "CS"),
    ("da", "DA"),
    ("nl", "NL"),
    ("nl-BE", "1D"),
    ("en", "EN"),
    ("en-GB", "6N"),
    ("en-AU", "1E"),
    ("en-BZ", "2E"),
    ("en-CA", "3E"),
    ("en-HK", "5E"),
    ("en-IN", "6E"),
    ("en-ID", "7E"),
    ("en-IE", "8E"),
    ("en-JM", "9E"),
    ("en-MY", "0E"),
    ("en-NZ", "1N"),
    ("en-PH", "2N"),
    ("en-SG", "3N"),
    ("en-ZA", "4N"),
    ("en-TT", "5N"),
    ("en-ZW", "7N"),
    ("et", "ET"),
    ("fi", "FI"),
    ("fr", "FR"),
    ("fr-BE", "1F"),
    ("fr-CM", "2F"),
    ("fr-CA", "3F"),
    ("fr-CG", "4F"),
    ("fr-CI", "5F"),
    ("fr-HT", "6F"),
    ("fr-LU", "7F"),
    ("fr-ML", "8F"),
    ("fr-MC", "9F"),
    ("fr-MA", "1H"),
    ("fr-RE", "2H"),
    ("fr-SN", "3H"),
    ("fr-CH", "4H"),
    ("gd", "GD"),
    ("gl", "GL"),
    ("de", "DE"),
    ("de-AT", "1G"),
    ("de-LI", "2G"),
    ("de-LU", "3G"),
    ("de-CH", "4G"),
    ("el", "EL"),
    ("he", "HE"),
    ("hu", "HU"),
    ("is", "IS"),
    ("id", "ID"),
    ("ga", "GA"),
    ("it", "IT"),
    ("it-CH", "1I"),
    ("ja", "JA"),
    ("ko", "KO"),
    ("lv", "LV"),
    ("lt", "LT"),
    ("ms", "MS"),
    ("ms-BN", "1M"),
    ("no", "NO"),
    ("pl", "PL"),
    ("pt", "PT"),
    ("rm", "RM"),
    ("ro", "RO"),
    ("ru", "RU"),
    ("sr-Cyrl", "SR"),
    ("sr-Latn", "SH"),
    ("sk", "SK"),
    ("sl", "SL"),
    ("wen", "SB"),
    ("dsb", "DS"),
    ("hsb", "HS"),
    ("es", "ES"),
    ("es-AR", "1S"),
    ("es-BO", "2S"),
    ("es-CL", "3S"),
    ("es-CO", "0S"),
    ("es-CR", "4S"),
    ("es-DO", "5S"),
    ("es-EC", "6S"),
    ("es-SV", "7S"),
    ("es-GT", "8S"),
    ("es-HN", "9S"),
    ("es-MX", "1X"),
    ("es-NI", "2X"),
    ("es-PA", "3X"),
    ("es-PY", "4X"),
    ("es-PE", "5X"),
    ("es-PR", "6X"),
    ("es-UY", "7X"),
    ("es-VE", "8X"),
    ("sw", "SW"),
    ("sv", "SV"),
    ("tl", "TL"),
    ("th", "TH"),
    ("tr", "TR"),
    ("uk", "UK"),
    ("vi", "VI"),
    ("wa", "WA"),
];

pub(crate) fn language_from_adt(
    value: &str,
    field: &'static str,
) -> Result<String, ProjectionError> {
    LANGUAGES
        .iter()
        .find_map(|&(bcp47, sap)| (sap == value).then(|| bcp47.to_owned()))
        .ok_or_else(|| ProjectionError::InvalidAffField {
            field,
            message: format!("unsupported ADT language `{value}`"),
        })
}

pub(crate) fn language_to_adt(value: &str, field: &'static str) -> Result<String, ProjectionError> {
    LANGUAGES
        .iter()
        .find_map(|&(bcp47, sap)| (bcp47 == value).then(|| sap.to_owned()))
        .ok_or_else(|| ProjectionError::InvalidAffField {
            field,
            message: format!("unsupported AFF language `{value}`"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_languages_in_both_directions() {
        for &(bcp47, sap) in LANGUAGES {
            assert_eq!(language_from_adt(sap, "language").unwrap(), bcp47);
            assert_eq!(language_to_adt(bcp47, "language").unwrap(), sap);
        }
    }

    #[test]
    fn conversions_use_the_supplied_standard_encoding() {
        for standard in [
            AdtAbapLanguageVersion::StandardX,
            AdtAbapLanguageVersion::Other("0".to_owned()),
        ] {
            for (aff, adt) in [
                (AbapLanguageVersion::Standard, standard.clone()),
                (
                    AbapLanguageVersion::KeyUser,
                    AdtAbapLanguageVersion::KeyUser,
                ),
                (
                    AbapLanguageVersion::CloudDevelopment,
                    AdtAbapLanguageVersion::CloudDevelopment,
                ),
            ] {
                assert_eq!(aff.to_adt(standard.clone()), adt);
                let converted = if standard == AdtAbapLanguageVersion::StandardX {
                    aff.to_adt_reps()
                } else {
                    aff.to_adt_ddic()
                };
                assert_eq!(converted, adt);
                assert_eq!(
                    AbapLanguageVersion::from_adt(Some(&adt), standard.as_str()),
                    Ok(aff)
                );
            }
            for value in [
                None,
                Some(AdtAbapLanguageVersion::Other(String::new())),
                Some(AdtAbapLanguageVersion::Standard),
            ] {
                assert_eq!(
                    AbapLanguageVersion::from_adt(value.as_ref(), standard.as_str()),
                    Ok(AbapLanguageVersion::Standard)
                );
            }
            for wire in ["X", "0", "unsupported"] {
                if wire != standard.as_str() {
                    let value = AdtAbapLanguageVersion::Other(wire.to_owned());
                    assert_eq!(
                        AbapLanguageVersion::from_adt(Some(&value), standard.as_str()),
                        Err(wire)
                    );
                }
            }
        }
    }
}
