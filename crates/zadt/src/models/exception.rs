use std::fmt;

use serde::Deserialize;

/// A structured exception representation returned by an ADT resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdtException {
    pub namespace: String,
    pub exception_type: String,
    pub message: String,
    pub localized_message: Option<String>,
    pub properties: Vec<AdtExceptionProperty>,
}

impl AdtException {
    /// Parses an ADT communication-framework exception response.
    pub fn parse(body: &[u8]) -> Result<Self, serde_xml_rs::Error> {
        let raw: RawAdtException = serde_xml_rs::from_reader(body)?;
        Ok(Self {
            namespace: raw.namespace.id,
            exception_type: raw.exception_type.id,
            message: raw.message,
            localized_message: raw.localized_message,
            properties: raw
                .properties
                .entries
                .into_iter()
                .map(|entry| AdtExceptionProperty {
                    key: entry.key,
                    value: entry.value,
                })
                .collect(),
        })
    }

    /// Returns the first property with the supplied key.
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|property| property.key == key)
            .map(|property| property.value.as_str())
    }
}

impl fmt::Display for AdtException {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.localized_message
            .as_deref()
            .unwrap_or(&self.message)
            .fmt(formatter)
    }
}

/// One ordered property attached to an [`AdtException`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdtExceptionProperty {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename = "exception")]
struct RawAdtException {
    namespace: RawExceptionId,
    #[serde(rename = "type")]
    exception_type: RawExceptionId,
    message: String,
    #[serde(rename = "localizedMessage", default)]
    localized_message: Option<String>,
    #[serde(default)]
    properties: RawExceptionProperties,
}

#[derive(Deserialize)]
struct RawExceptionId {
    #[serde(rename = "@id")]
    id: String,
}

#[derive(Default, Deserialize)]
struct RawExceptionProperties {
    #[serde(rename = "entry", default)]
    entries: Vec<RawExceptionProperty>,
}

#[derive(Deserialize)]
struct RawExceptionProperty {
    #[serde(rename = "@key")]
    key: String,
    #[serde(rename = "#text")]
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK_CONFLICT: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
        <exc:exception xmlns:exc="http://www.sap.com/abapxml/types/communicationframework">
            <namespace id="com.sap.adt"/>
            <type id="ExceptionResourceLockConflict"/>
            <message lang="EN">Object is already locked</message>
            <localizedMessage lang="EN">Object is already locked in request A4HK900125</localizedMessage>
            <properties>
                <entry key="T100KEY-ID">CTS_WBO_API</entry>
                <entry key="T100KEY-NO">019</entry>
                <entry key="T100KEY-V3">A4HK900125</entry>
                <entry key="LONGTEXT">&lt;HTML&gt;&lt;BODY&gt;Release the request.&lt;/BODY&gt;&lt;/HTML&gt;</entry>
            </properties>
        </exc:exception>"#;

    #[test]
    fn parses_structured_adt_exceptions_and_decodes_properties() {
        let exception = AdtException::parse(LOCK_CONFLICT).unwrap();

        assert_eq!(exception.namespace, "com.sap.adt");
        assert_eq!(exception.exception_type, "ExceptionResourceLockConflict");
        assert_eq!(
            exception.localized_message.as_deref(),
            Some("Object is already locked in request A4HK900125")
        );
        assert_eq!(exception.property("T100KEY-ID"), Some("CTS_WBO_API"));
        assert_eq!(exception.property("T100KEY-NO"), Some("019"));
        assert_eq!(exception.property("T100KEY-V3"), Some("A4HK900125"));
        assert_eq!(
            exception.property("LONGTEXT"),
            Some("<HTML><BODY>Release the request.</BODY></HTML>")
        );
        assert_eq!(exception.property("missing"), None);
    }
}
