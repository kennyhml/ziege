use crate::CategoryId;
use std::{iter::Copied, ops::Index, slice};
use thiserror::Error;

/// An ordered set of locally supported media types.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MediaTypes {
    values: &'static [&'static str],
}

impl MediaTypes {
    /// Creates an ordered set of supported media types.
    pub const fn new(values: &'static [&'static str]) -> Self {
        Self { values }
    }

    /// Returns whether the set contains no media types.
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of media types in this set.
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns the media types in preference order.
    pub const fn as_slice(&self) -> &'static [&'static str] {
        self.values
    }

    /// Returns the media type at `index`, when present.
    pub fn get(&self, index: usize) -> Option<&'static str> {
        self.as_slice().get(index).copied()
    }

    /// Iterates over the media types in preference order.
    pub fn iter(&self) -> Copied<slice::Iter<'static, &'static str>> {
        self.as_slice().iter().copied()
    }

    /// Returns whether this set contains the exact media type.
    pub fn contains(&self, media_type: &str) -> bool {
        self.as_slice().contains(&media_type)
    }

    /// Finds the canonical supported value matching a concrete media type.
    pub fn matching(&self, candidate: &str) -> Option<&'static str> {
        self.as_slice()
            .iter()
            .copied()
            .find(|expected| media_types_match(expected, candidate))
    }

    /// Selects the preferred value accepted by at least one advertised range.
    pub fn select_compatible<I, M>(&self, accepted: I) -> Result<&'static str, CompatibilityError>
    where
        I: IntoIterator<Item = M>,
        M: AsRef<str>,
    {
        let accepted = accepted.into_iter().collect::<Vec<_>>();
        if let Some(media_type) = self.as_slice().iter().copied().find(|media_type| {
            accepted
                .iter()
                .any(|range| media_range_accepts(range.as_ref(), media_type))
        }) {
            return Ok(media_type);
        }

        Err(CompatibilityError::NoCompatibleMediaType {
            supported: self
                .as_slice()
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            accepted: accepted
                .into_iter()
                .map(|value| value.as_ref().to_owned())
                .collect(),
        })
    }
}

impl Index<usize> for MediaTypes {
    type Output = &'static str;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl IntoIterator for MediaTypes {
    type Item = &'static str;
    type IntoIter = Copied<slice::Iter<'static, &'static str>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

struct ParsedMediaType<'a> {
    essence: &'a str,
    parameters: Vec<(&'a str, &'a str)>,
}

impl<'a> ParsedMediaType<'a> {
    fn parse(value: &'a str) -> Option<Self> {
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

        Some(Self {
            essence,
            parameters,
        })
    }
}

fn strip_suffix_and_prefix(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn parameter_values_match(expected: &str, candidate: &str) -> bool {
    strip_suffix_and_prefix(expected) == strip_suffix_and_prefix(candidate)
}

pub(crate) fn media_types_match(expected: &str, candidate: &str) -> bool {
    let (Some(expected), Some(candidate)) = (
        ParsedMediaType::parse(expected),
        ParsedMediaType::parse(candidate),
    ) else {
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

fn media_range_accepts(range: &str, media_type: &str) -> bool {
    let (Some(range), Some(media_type)) = (
        ParsedMediaType::parse(range),
        ParsedMediaType::parse(media_type),
    ) else {
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
                    .map(|(_, value)| strip_suffix_and_prefix(value))
                    .unwrap_or("utf-8");
                return strip_suffix_and_prefix(range_value)
                    .eq_ignore_ascii_case(media_type_charset);
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
    fn returns_the_canonical_supported_media_type() {
        const SUPPORTED: &[&str] = &[
            "application/example+xml;version=2",
            "application/example+xml;version=1",
        ];
        let media_types = MediaTypes::new(SUPPORTED);

        assert_eq!(
            media_types.matching("application/example+xml; version=2; charset=UTF-8"),
            Some(SUPPORTED[0])
        );
        assert_eq!(
            media_types.matching("application/example+xml; version=3"),
            None
        );
    }

    #[test]
    fn selects_preferred_media_type_accepted_by_a_discovery_range() {
        const PREFERRED: &[&str] = &["application/example.v3+xml", "application/example.v2+xml"];
        let preferred = MediaTypes::new(PREFERRED);

        assert_eq!(
            preferred
                .select_compatible(["application/example.v2+xml"])
                .unwrap(),
            "application/example.v2+xml"
        );
        assert_eq!(
            preferred.select_compatible(["application/*"]).unwrap(),
            "application/example.v3+xml"
        );
        assert!(matches!(
            preferred.select_compatible(std::iter::empty::<&str>()),
            Err(CompatibilityError::NoCompatibleMediaType {
                supported,
                accepted,
            }) if supported == PREFERRED && accepted.is_empty()
        ));
    }

    #[test]
    fn incompatible_media_types_retain_both_sides_for_diagnostics() {
        const PREFERRED: &[&str] = &["application/example.v2+xml"];
        let error = MediaTypes::new(PREFERRED)
            .select_compatible(["application/example.v1+xml", "text/plain"])
            .unwrap_err();

        assert!(matches!(
            error,
            CompatibilityError::NoCompatibleMediaType {
                supported,
                accepted,
            } if supported == PREFERRED
                && accepted == ["application/example.v1+xml", "text/plain"]
        ));
    }

    #[test]
    fn accepted_media_ranges_require_their_semantic_parameters() {
        const PREFERRED: &[&str] = &["application/example+xml;version=2; charset=utf-8"];
        let preferred = MediaTypes::new(PREFERRED);

        assert_eq!(
            preferred
                .select_compatible(["application/example+xml; version=2; q=0.5"])
                .unwrap(),
            PREFERRED[0]
        );
        assert!(
            preferred
                .select_compatible(["application/example+xml; version=1"])
                .is_err()
        );
        assert_eq!(
            preferred
                .select_compatible(["application/example+xml; version=\"2\""])
                .unwrap(),
            PREFERRED[0]
        );
    }

    #[test]
    fn ignores_atompub_quality_parameters_and_rejects_invalid_ranges() {
        const PREFERRED: &[&str] = &["application/example+xml"];
        let preferred = MediaTypes::new(PREFERRED);

        assert_eq!(
            preferred
                .select_compatible(["application/example+xml; q=0"])
                .unwrap(),
            PREFERRED[0]
        );
        assert!(preferred.select_compatible(["*/xml"]).is_err());
    }

    #[test]
    fn accepted_media_ranges_honor_charset_constraints() {
        const PREFERRED: &[&str] = &["application/example+xml"];
        let preferred = MediaTypes::new(PREFERRED);

        assert_eq!(
            preferred
                .select_compatible(["application/example+xml; charset=UTF-8"])
                .unwrap(),
            PREFERRED[0]
        );
        assert!(
            preferred
                .select_compatible(["application/example+xml; charset=UTF-16"])
                .is_err()
        );
    }
}
