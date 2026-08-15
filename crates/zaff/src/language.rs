use crate::ProjectionError;

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

pub(crate) fn from_adt(value: &str, field: &'static str) -> Result<String, ProjectionError> {
    LANGUAGES
        .iter()
        .find_map(|&(bcp47, sap)| (sap == value).then(|| bcp47.to_owned()))
        .ok_or_else(|| invalid(field, format!("unsupported ADT language `{value}`")))
}

pub(crate) fn to_adt(value: &str, field: &'static str) -> Result<String, ProjectionError> {
    LANGUAGES
        .iter()
        .find_map(|&(bcp47, sap)| (bcp47 == value).then(|| sap.to_owned()))
        .ok_or_else(|| invalid(field, format!("unsupported AFF language `{value}`")))
}

fn invalid(field: &'static str, message: String) -> ProjectionError {
    ProjectionError::InvalidAffField { field, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_simple_and_regional_languages_in_both_directions() {
        for (bcp47, sap) in [("en", "EN"), ("en-GB", "6N"), ("zh-Hant", "ZF")] {
            assert_eq!(from_adt(sap, "language").unwrap(), bcp47);
            assert_eq!(to_adt(bcp47, "language").unwrap(), sap);
        }
    }
}
