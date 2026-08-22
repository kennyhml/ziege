use thiserror::Error;

use crate::CategoryId;

struct ParsedMediaType<'a> {
    essence: &'a str,
    parameters: Vec<(&'a str, &'a str)>,
}

fn parse_media_type(value: &str) -> Option<ParsedMediaType<'_>> {
    let mut parts = value.split(';');
    let essence = parts.next()?.trim();
    if essence.is_empty() {
        return None;
    }

    let parameters = parts
        .map(|parameter| {
            let (name, value) = parameter.split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            (!name.is_empty() && !value.is_empty()).then_some((name, value))
        })
        .collect::<Option<Vec<_>>>()?;

    Some(ParsedMediaType {
        essence,
        parameters,
    })
}

pub(crate) fn media_types_match(expected: &str, candidate: &str) -> bool {
    let (Some(expected), Some(candidate)) =
        (parse_media_type(expected), parse_media_type(candidate))
    else {
        return false;
    };

    if !expected.essence.eq_ignore_ascii_case(candidate.essence) {
        return false;
    }

    let expected_count = expected
        .parameters
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("charset"))
        .count();
    let candidate_count = candidate
        .parameters
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("charset"))
        .count();

    expected_count == candidate_count
        && expected
            .parameters
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("charset"))
            .all(|(expected_name, expected_value)| {
                candidate
                    .parameters
                    .iter()
                    .any(|(candidate_name, candidate_value)| {
                        expected_name.eq_ignore_ascii_case(candidate_name)
                            && expected_value == candidate_value
                    })
            })
}

/// An error establishing protocol compatibility with an ADT backend.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompatibilityError {
    /// Central discovery did not advertise a required collection.
    #[error("ADT discovery did not advertise collection {0:?}")]
    MissingCollection(CategoryId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_semantic_parameters_independently_of_charset_and_formatting() {
        let expected = "application/vnd.sap.as+xml; charset=utf-8; \
             dataname=com.sap.adt.CreateCorrectionRequest.v1";

        assert!(media_types_match(
            expected,
            "APPLICATION/VND.SAP.AS+XML;dataname=com.sap.adt.CreateCorrectionRequest.v1; \
             charset=UTF-8"
        ));
    }

    #[test]
    fn rejects_different_or_unexpected_semantic_parameters() {
        let legacy = "application/vnd.sap.as+xml; \
             dataname=com.sap.adt.CreateCorrectionRequest";
        let versioned =
            "application/vnd.sap.as+xml; dataname=com.sap.adt.CreateCorrectionRequest.v1";

        assert!(!media_types_match(legacy, versioned));
        assert!(!media_types_match("application/vnd.sap.as+xml", versioned));
    }

    #[test]
    fn matches_version_media_type_parameters_exactly() {
        let quick_fix = "application/vnd.sap.adt.quickfixes.evaluation+xml;version=1.0.0";

        assert!(media_types_match(
            quick_fix,
            "application/vnd.sap.adt.quickfixes.evaluation+xml; version=1.0.0"
        ));
        assert!(!media_types_match(
            quick_fix,
            "application/vnd.sap.adt.quickfixes.evaluation+xml; version=2.0.0"
        ));
    }
}
