use core::fmt;
use std::{borrow::Cow, str::FromStr};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Class, Include, ObjectType, Program};

    #[test]
    fn preserves_exact_static_global_workbench_types() {
        let object_type = GlobalWorkbenchType::new("ABCD/XYZ");

        assert_eq!(object_type.as_str(), "ABCD/XYZ");
        assert_eq!(object_type.to_string(), "ABCD/XYZ");
        assert_eq!(Program::WORKBENCH_TYPE.to_string(), "PROG/P");
        assert_eq!(Include::WORKBENCH_TYPE.to_string(), "PROG/I");
        assert_eq!(Class::WORKBENCH_TYPE.to_string(), "CLAS/OC");
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
    fn serializes_opaque_workbench_types_without_normalizing_them() {
        for value in ["AUTH", "/RQ", "amdp", "CLAS/OCN/definitions"] {
            let object_type: GlobalWorkbenchType = value.parse().unwrap();
            let json = serde_json::to_string(&object_type).unwrap();
            let decoded: GlobalWorkbenchType = serde_json::from_str(&json).unwrap();

            assert_eq!(json, format!("\"{value}\""));
            assert_eq!(decoded, object_type);
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
}
