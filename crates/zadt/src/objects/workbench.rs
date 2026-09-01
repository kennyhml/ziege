use core::fmt;
use std::{borrow::Cow, str::FromStr};

use serde::{Deserialize, Serialize};

/// An exact global ABAP Workbench type registered with ADT.
///
/// # Background
///
/// Common values combine an R3TR object-directory type and an internal subtype,
/// such as `PROG/P` or `CLAS/OC`. The ADT registry also contains compact values
/// such as `AUTH`, lowercase values such as `amdp`, and identifiers with more
/// than one slash. The value is therefore treated as one opaque protocol token.
#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct GlobalWorkbenchType(Cow<'static, str>);

impl GlobalWorkbenchType {
    /// Creates a static global Workbench type.
    pub const fn new(value: &'static str) -> Self {
        assert!(!value.is_empty(), "global Workbench type must not be empty");
        assert!(value.is_ascii(), "global Workbench type must be ASCII");
        let bytes = value.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            assert!(
                bytes[index] > b' ' && bytes[index] != 0x7f,
                "global Workbench type must not contain whitespace or control characters"
            );
            index += 1;
        }
        Self(Cow::Borrowed(value))
    }

    /// Returns the exact identifier registered with ADT.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GlobalWorkbenchType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl serde::Serialize for GlobalWorkbenchType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

/// An error parsing an ADT global Workbench type such as `PROG/I` or `AUTH`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid global Workbench type `{value}`: {reason}")]
pub struct InvalidWorkbenchType {
    value: String,
    reason: &'static str,
}

impl FromStr for GlobalWorkbenchType {
    type Err = InvalidWorkbenchType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = |reason| InvalidWorkbenchType {
            value: value.to_owned(),
            reason,
        };
        if value.is_empty() {
            return Err(invalid("value is empty"));
        }
        if !value.is_ascii() {
            return Err(invalid("value must be ASCII"));
        }
        if value.bytes().any(|byte| byte <= b' ' || byte == 0x7f) {
            return Err(invalid("value contains whitespace or control characters"));
        }
        Ok(Self(Cow::Owned(value.to_owned())))
    }
}

impl TryFrom<String> for GlobalWorkbenchType {
    type Error = InvalidWorkbenchType;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

/// Version of a repository object.
///
/// These values are the public URI vocabulary for `IF_ADT_URI_QUERY_PARAMETERS`.
/// SAP maps them internally to one-character ABAP Workbench `R3STATE` values.
///
/// # References
/// - `IF_ADT_URI_QUERY_PARAMETERS` defines `CO_VERSION` and its external values
/// - `CL_ADT_UTILITY->GET_WB_VERSION` maps it to Workbench `R3STATE` values
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WorkbenchVersion {
    /// The persistent active object (R3STATE `A`).
    Active,
    /// An inactive object awaiting activation (R3STATE `I`).
    Inactive,
    /// Uses the current users inactive version when available (R3STATE `_`).
    WorkingArea,
    /// A newly created object (R3STATE `N`).
    New,
    /// An object for which only part of the content is active (R3STATE `P`).
    PartlyActive,
}

impl WorkbenchVersion {
    pub(crate) const QUERY_PARAMETER: &'static str = "version";

    /// Returns the exact value used by ADT URI query parameters.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::WorkingArea => "workingArea",
            Self::New => "new",
            Self::PartlyActive => "partlyActive",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "inactive" => Some(Self::Inactive),
            "workingArea" => Some(Self::WorkingArea),
            "new" => Some(Self::New),
            "partlyActive" => Some(Self::PartlyActive),
            _ => None,
        }
    }
}

impl fmt::Display for WorkbenchVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for WorkbenchVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WorkbenchVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| {
            serde::de::Error::custom(format!("unsupported object version `{value}`"))
        })
    }
}

/// ABAP language version advertised for repository objects.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AbapLanguageVersion {
    /// Standard ABAP.
    Standard,
    /// Standard ABAP as represented by loaded source-object properties.
    StandardX,
    /// ABAP for Key Users.
    KeyUser,
    /// ABAP for Cloud Development.
    CloudDevelopment,
    /// A backend-specific language-version value.
    Other(String),
}

impl AbapLanguageVersion {
    /// Returns the fixed value used by ADT.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Standard => " ",
            Self::StandardX => "X",
            Self::KeyUser => "2",
            Self::CloudDevelopment => "5",
            Self::Other(value) => value,
        }
    }
}

impl From<String> for AbapLanguageVersion {
    fn from(value: String) -> Self {
        match value.as_str() {
            " " => Self::Standard,
            "X" => Self::StandardX,
            "2" => Self::KeyUser,
            "5" => Self::CloudDevelopment,
            _ => Self::Other(value),
        }
    }
}

impl From<&str> for AbapLanguageVersion {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Serialize for AbapLanguageVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AbapLanguageVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::from)
    }
}

impl fmt::Display for AbapLanguageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class, DataElement, Include, ObjectType, Program};

    #[test]
    fn preserves_exact_static_global_workbench_types() {
        let object_type = GlobalWorkbenchType::new("ABCD/XYZ");

        assert_eq!(object_type.as_str(), "ABCD/XYZ");
        assert_eq!(object_type.to_string(), "ABCD/XYZ");
        assert_eq!(Program::WORKBENCH_TYPE.to_string(), "PROG/P");
        assert_eq!(Include::WORKBENCH_TYPE.to_string(), "PROG/I");
        assert_eq!(Class::WORKBENCH_TYPE.to_string(), "CLAS/OC");
        assert_eq!(DataElement::WORKBENCH_TYPE.to_string(), "DTEL/DE");
    }

    #[test]
    fn parses_the_opaque_adt_registry_vocabulary() {
        for value in [
            "CLAS/OM",
            "AUTH",
            "DEFAULT",
            "/RQ",
            "amdp",
            "CLAS/OCN/definitions",
        ] {
            let object_type: GlobalWorkbenchType = value.parse().unwrap();

            assert_eq!(object_type.as_str(), value);
            assert_eq!(object_type.to_string(), value);
        }
    }

    #[test]
    fn rejects_invalid_global_workbench_type_responses() {
        for value in ["", "CLAS OC", "CLAS\nOC", "ÄUTH"] {
            assert!(
                value.parse::<GlobalWorkbenchType>().is_err(),
                "accepted {value}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "global Workbench type must not contain whitespace")]
    fn static_global_workbench_type_rejects_whitespace() {
        GlobalWorkbenchType::new("CLAS OC");
    }

    #[test]
    fn uses_the_adt_query_parameter_vocabulary() {
        for (version, value) in [
            (WorkbenchVersion::Active, "active"),
            (WorkbenchVersion::Inactive, "inactive"),
            (WorkbenchVersion::WorkingArea, "workingArea"),
            (WorkbenchVersion::New, "new"),
            (WorkbenchVersion::PartlyActive, "partlyActive"),
        ] {
            assert_eq!(version.as_str(), value);
            assert_eq!(WorkbenchVersion::parse(value), Some(version));
            assert_eq!(
                serde_json::from_value::<WorkbenchVersion>(value.into()).unwrap(),
                version
            );
        }
        assert!(serde_json::from_str::<WorkbenchVersion>("\"future\"").is_err());
    }

    #[test]
    fn uses_the_adt_fixed_values() {
        for (version, value) in [
            (AbapLanguageVersion::Standard, " "),
            (AbapLanguageVersion::StandardX, "X"),
            (AbapLanguageVersion::KeyUser, "2"),
            (AbapLanguageVersion::CloudDevelopment, "5"),
            (AbapLanguageVersion::Other("future".to_owned()), "future"),
        ] {
            assert_eq!(version.as_str(), value);
            assert_eq!(serde_json::to_value(&version).unwrap(), value);
            assert_eq!(
                serde_json::from_value::<AbapLanguageVersion>(value.into()).unwrap(),
                version
            );
        }
    }
}
