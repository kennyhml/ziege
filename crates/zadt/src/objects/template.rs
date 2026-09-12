use serde::{Deserialize, Serialize};

/// An `abapsource:template` for class and interface copying or source generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTemplate {
    /// The existing object name or ADT template implementation name.
    #[serde(rename = "@abapsource:name")]
    pub name: String,

    #[serde(
        rename = "abapsource:property",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub properties: Vec<SourceTemplateProperty>,
}

impl SourceTemplate {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: Vec::new(),
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties
            .push(SourceTemplateProperty::new(key, value));
        self
    }
}

/// One parameter passed to an ADT source template.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTemplateProperty {
    #[serde(rename = "@abapsource:key")]
    pub key: String,

    #[serde(rename = "#text", default)]
    pub value: String,
}

impl SourceTemplateProperty {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// An `adtcore:adtTemplate` used by generic ADT object creation handlers.
///
/// This uses SADT_OBJECT wire names, distinct from the `abapsource:template`
/// representation used by class source templates. Property-only templates may
/// omit the name.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectTemplate {
    #[serde(rename = "@adtcore:name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(
        rename = "adtcore:adtProperty",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub properties: Vec<ObjectTemplateProperty>,
}

impl ObjectTemplate {
    /// Creates a named object template.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            properties: Vec::new(),
        }
    }

    /// Adds a handler-specific template property.
    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push(ObjectTemplateProperty {
            key: key.into(),
            value: value.into(),
        });
        self
    }
}

/// One parameter for an ADT object template, with its value stored as element text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectTemplateProperty {
    #[serde(rename = "@adtcore:key")]
    pub key: String,

    #[serde(rename = "#text", default)]
    pub value: String,
}
