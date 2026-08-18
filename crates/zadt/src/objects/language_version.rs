use serde::{Deserialize, Serialize};

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

impl std::fmt::Display for AbapLanguageVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
