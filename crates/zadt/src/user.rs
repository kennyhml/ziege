use std::{
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An SAP user identity.
///
/// A user is scoped to the SAP system against which an operation is executed.
/// It carries no credentials or client connection.
#[derive(Clone, Debug)]
pub struct User {
    name: String,
    display_name: Option<String>,
}

impl User {
    /// Creates a user from its backend name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: None,
        }
    }

    pub(crate) fn with_display_name(
        name: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let display_name = display_name.into();
        Self {
            name: name.into(),
            display_name: (!display_name.is_empty()).then_some(display_name),
        }
    }

    /// Returns the backend user name.
    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// Returns the display name loaded from the system user directory, when available.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Consumes this value and returns the backend user name.
    pub fn into_inner(self) -> String {
        self.name
    }
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for User {}

impl Hash for User {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl AsRef<str> for User {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for User {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<String> for User {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl From<&str> for User {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<&User> for User {
    fn from(user: &User) -> Self {
        user.clone()
    }
}

impl Serialize for User {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for User {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::new)
    }
}
