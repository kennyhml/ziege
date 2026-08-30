use thiserror::Error;

use crate::CategoryId;

struct ParsedMediaType<'a> {
    essence: &'a str,
    parameters: Vec<(&'a str, &'a str)>,
}

fn parse_media_type(value: &str) -> Option<ParsedMediaType<'_>> {
    let mut parts = value.split(';');
    let essence = parts.next()?.trim();
    let (media_type, media_subtype) = essence.split_once('/')?;
    if media_type.is_empty()
        || media_subtype.is_empty()
        || media_subtype.contains('/')
        || media_type.contains('*') && media_type != "*"
        || media_subtype.contains('*') && media_subtype != "*"
        || media_type == "*" && media_subtype != "*"
    {
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

fn parameter_value(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn parameter_values_match(expected: &str, candidate: &str) -> bool {
    parameter_value(expected) == parameter_value(candidate)
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
                            && parameter_values_match(expected_value, candidate_value)
                    })
            })
}

pub(crate) fn matching_media_type(
    supported: &'static [&'static str],
    candidate: &str,
) -> Option<&'static str> {
    supported
        .iter()
        .copied()
        .find(|expected| media_types_match(expected, candidate))
}

fn media_range_accepts(range: &str, media_type: &str) -> bool {
    let (Some(range), Some(media_type)) = (parse_media_type(range), parse_media_type(media_type))
    else {
        return false;
    };
    let Some((range_type, range_subtype)) = range.essence.split_once('/') else {
        return false;
    };
    let Some((media_type_type, media_type_subtype)) = media_type.essence.split_once('/') else {
        return false;
    };
    if media_type_type == "*" || media_type_subtype == "*" {
        return false;
    }
    if range_type != "*" && !range_type.eq_ignore_ascii_case(media_type_type) {
        return false;
    }
    if range_subtype != "*" && !range_subtype.eq_ignore_ascii_case(media_type_subtype) {
        return false;
    }
    range
        .parameters
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("q"))
        .all(|(range_name, range_value)| {
            if range_name.eq_ignore_ascii_case("charset") {
                // ToXml emits UTF-8 bytes when the local media type omits a charset.
                let media_type_charset = media_type
                    .parameters
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("charset"))
                    .map(|(_, value)| parameter_value(value))
                    .unwrap_or("utf-8");
                return parameter_value(range_value).eq_ignore_ascii_case(media_type_charset);
            }
            media_type
                .parameters
                .iter()
                .any(|(media_type_name, media_type_value)| {
                    range_name.eq_ignore_ascii_case(media_type_name)
                        && parameter_values_match(range_value, media_type_value)
                })
        })
}

pub(crate) fn select_accepted_media_type(
    preferred: &'static [&'static str],
    accepted: &[String],
) -> Option<&'static str> {
    preferred.iter().copied().find(|media_type| {
        accepted
            .iter()
            .any(|range| media_range_accepts(range, media_type))
    })
}

/// An error establishing protocol compatibility with an ADT backend.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompatibilityError {
    /// Central discovery did not advertise a required collection.
    #[error("ADT discovery did not advertise collection {0:?}")]
    MissingCollection(CategoryId),

    /// A collection does not accept any locally supported request representation.
    #[error(
        "ADT collection accepts none of the supported media types; supported: {supported:?}, accepted: {accepted:?}"
    )]
    NoCompatibleMediaType {
        supported: Vec<String>,
        accepted: Vec<String>,
    },
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

    #[test]
    fn selects_preferred_media_type_accepted_by_a_discovery_range() {
        let preferred = &["application/example.v3+xml", "application/example.v2+xml"];

        assert_eq!(
            select_accepted_media_type(preferred, &["application/example.v2+xml".to_owned()]),
            Some("application/example.v2+xml")
        );
        assert_eq!(
            select_accepted_media_type(preferred, &["application/*".to_owned()]),
            Some("application/example.v3+xml")
        );
        assert_eq!(select_accepted_media_type(preferred, &[]), None);
    }

    #[test]
    fn accepted_media_ranges_require_their_semantic_parameters() {
        let preferred = &["application/example+xml;version=2; charset=utf-8"];

        assert_eq!(
            select_accepted_media_type(
                preferred,
                &["application/example+xml; version=2; q=0.5".to_owned()]
            ),
            Some(preferred[0])
        );
        assert_eq!(
            select_accepted_media_type(
                preferred,
                &["application/example+xml; version=1".to_owned()]
            ),
            None
        );
        assert_eq!(
            select_accepted_media_type(
                preferred,
                &["application/example+xml; version=\"2\"".to_owned()]
            ),
            Some(preferred[0])
        );
    }

    #[test]
    fn ignores_atompub_quality_parameters_and_rejects_invalid_ranges() {
        let preferred = &["application/example+xml"];

        assert_eq!(
            select_accepted_media_type(preferred, &["application/example+xml; q=0".to_owned()]),
            Some(preferred[0])
        );
        assert_eq!(
            select_accepted_media_type(preferred, &["*/xml".to_owned()]),
            None
        );
    }

    #[test]
    fn accepted_media_ranges_honor_charset_constraints() {
        let preferred = &["application/example+xml"];

        assert_eq!(
            select_accepted_media_type(
                preferred,
                &["application/example+xml; charset=UTF-8".to_owned()]
            ),
            Some(preferred[0])
        );
        assert_eq!(
            select_accepted_media_type(
                preferred,
                &["application/example+xml; charset=UTF-16".to_owned()]
            ),
            None
        );
    }
}
