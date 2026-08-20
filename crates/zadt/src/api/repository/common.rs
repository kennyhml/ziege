use std::{borrow::Cow, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A repository information system facet key.
///
/// SAP defines a common set of keys, exposed as associated constants, but
/// systems may advertise additional facets. Unknown keys are therefore kept
/// intact instead of being rejected by a closed enum.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RepositoryFacet(Cow<'static, str>);

impl RepositoryFacet {
    pub const PACKAGE: Self = Self(Cow::Borrowed("PACKAGE"));
    pub const GROUP: Self = Self(Cow::Borrowed("GROUP"));
    pub const TYPE: Self = Self(Cow::Borrowed("TYPE"));
    pub const OWNER: Self = Self(Cow::Borrowed("OWNER"));
    pub const API_STATE: Self = Self(Cow::Borrowed("API"));
    pub const APPLICATION_COMPONENT: Self = Self(Cow::Borrowed("APPL"));
    pub const FAVORITES: Self = Self(Cow::Borrowed("FAV"));
    pub const CREATED: Self = Self(Cow::Borrowed("CREATED"));
    pub const CREATION_MONTH: Self = Self(Cow::Borrowed("MONTH"));
    pub const CREATION_DATE: Self = Self(Cow::Borrowed("DATE"));
    pub const LANGUAGE: Self = Self(Cow::Borrowed("LANGUAGE"));
    pub const SOURCE_SYSTEM: Self = Self(Cow::Borrowed("SYSTEM"));
    pub const VERSION: Self = Self(Cow::Borrowed("VERSION"));
    pub const DOCUMENTATION: Self = Self(Cow::Borrowed("DOCU"));

    /// Returns the exact facet key used by RIS.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RepositoryFacet {
    fn from(value: &str) -> Self {
        Self(Cow::Owned(value.to_owned()))
    }
}

impl From<String> for RepositoryFacet {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl fmt::Display for RepositoryFacet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for RepositoryFacet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RepositoryFacet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// A filter applied before RIS structures or returns repository objects.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename = "vfs:preselection")]
pub struct RepositoryPreselection {
    #[serde(rename = "@facet")]
    facet: RepositoryFacet,

    #[serde(rename = "vfs:value")]
    values: Vec<String>,
}

impl RepositoryPreselection {
    /// Creates an inclusive filter containing one value.
    pub fn new(facet: impl Into<RepositoryFacet>, value: impl Into<String>) -> Self {
        Self {
            facet: facet.into(),
            values: vec![value.into()],
        }
    }

    /// Selects objects assigned directly to a package, excluding subpackages.
    ///
    /// The leading `..` is RIS protocol syntax and does not denote filesystem
    /// parent traversal.
    pub fn directly_assigned(package: impl Into<String>) -> Self {
        let package = package.into();
        let package = package.strip_prefix("..").unwrap_or(&package);
        Self::new(RepositoryFacet::PACKAGE, format!("..{package}"))
    }

    /// Adds another included value.
    pub fn include(mut self, value: impl Into<String>) -> Self {
        self.values.push(value.into());
        self
    }

    /// Adds an excluded value, represented by RIS with a leading `-`.
    pub fn exclude(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.values.push(if value.starts_with('-') {
            value
        } else {
            format!("-{value}")
        });
        self
    }

    pub fn facet(&self) -> &RepositoryFacet {
        &self.facet
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }
}
