use std::{error::Error as StdError, fmt};

use http::StatusCode;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    AdtResponse, AdtUriError, BatchError, CategoryId, CompatibilityError, GlobalWorkbenchType,
    resource::AdtLinkError,
};

#[cfg(feature = "reqwest")]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReqwestTransportBuildError {
    #[error("required client field `{0}` was not provided")]
    MissingField(&'static str),

    #[error("invalid SAP destination: {0}")]
    InvalidDestination(#[from] url::ParseError),

    #[error("SAP destination must use HTTP or HTTPS")]
    UnsupportedScheme,

    #[error("SAP destination must not contain credentials, a query, or a fragment")]
    InvalidDestinationComponents,

    #[cfg(feature = "reqwest")]
    #[error("could not construct the HTTP client: {0}")]
    HttpClient(#[from] reqwest::Error),
}

#[cfg(feature = "reqwest")]
impl From<derive_builder::UninitializedFieldError> for ReqwestTransportBuildError {
    fn from(error: derive_builder::UninitializedFieldError) -> Self {
        Self::MissingField(error.field_name())
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DiscoveryError {
    #[error("invalid discovery XML: {0}")]
    Xml(#[from] serde_xml_rs::Error),
}

/// An error decoding or validating the HTTP session established during logon.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LogonError {
    #[error("invalid HTTP session-information XML: {0}")]
    InvalidResponse(#[from] serde_xml_rs::Error),

    #[error("HTTP session response did not include a Content-Type header")]
    MissingContentType,

    #[error("HTTP session response did not include a representation body")]
    MissingResponseBody,

    #[error("unsupported HTTP session response Content-Type `{content_type}`")]
    UnsupportedContentType { content_type: String },

    #[error("HTTP session response did not advertise a logoff resource")]
    MissingLogoffLink,

    #[error("HTTP session response did not advertise a cleanup resource")]
    MissingCleanupLink,

    #[error("system-information link did not advertise a Content-Type")]
    MissingSystemInformationContentType,

    #[error("invalid HTTP session link `{href}` for relation `{relation}`")]
    InvalidLink { relation: String, href: String },

    #[error("HTTP session response advertised inactivityTimeout more than once")]
    DuplicateInactivityTimeout,

    #[error("invalid HTTP session inactivity timeout `{value}`: {source}")]
    InvalidInactivityTimeout {
        value: String,
        source: std::num::ParseIntError,
    },
}

/// An error in a generic ADT object operation or representation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ObjectError {
    #[error(transparent)]
    Compatibility(#[from] CompatibilityError),

    #[error("could not construct the object resource URI: {0}")]
    InvalidTarget(#[from] AdtUriError),

    #[error("invalid object representation: {0}")]
    InvalidResponse(#[source] serde_xml_rs::Error),

    #[error("could not serialize object update request: {0}")]
    InvalidRequest(#[source] serde_xml_rs::Error),

    #[error("invalid object properties JSON: {0}")]
    InvalidPropertiesJson(#[source] serde_json::Error),

    #[error("object link `{href}` could not be resolved: {source}")]
    InvalidLink { href: String, source: AdtUriError },

    #[error("object response did not advertise the required `{relation}` relation")]
    MissingRelation { relation: &'static str },

    #[error("object response advertised source component `{component}` more than once")]
    DuplicateSourceComponent { component: String },

    #[error("object type `{object_type}` does not support {capability}")]
    UnsupportedCapability {
        object_type: GlobalWorkbenchType,
        capability: &'static str,
    },

    #[error("object type `{object_type}` is not modeled by ZADT")]
    UnsupportedObjectType { object_type: GlobalWorkbenchType },

    #[error("unsupported object version `{version}`")]
    UnsupportedObjectVersion { version: String },

    #[error("unsupported Data Element type kind `{kind}`")]
    UnsupportedDataElementTypeKind { kind: String },

    #[error("unsupported Data Element documentation status `{status}`")]
    UnsupportedDataElementDocumentationStatus { status: String },

    #[error(
        "object `{relation}` reference `{declared}` disagrees with advertised relation `{advertised}`"
    )]
    RelationMismatch {
        relation: &'static str,
        declared: String,
        advertised: String,
    },

    #[error("expected object type `{expected}`, but the response advertised `{actual}`")]
    UnexpectedObjectType {
        expected: GlobalWorkbenchType,
        actual: GlobalWorkbenchType,
    },

    #[error("expected repository object type `{expected}`, but RIS advertised `{actual}`")]
    UnexpectedRepositoryObjectType {
        expected: GlobalWorkbenchType,
        actual: GlobalWorkbenchType,
    },

    #[error("expected compact object type `{expected}`, but the response advertised `{actual}`")]
    UnexpectedCompactObjectType {
        expected: &'static str,
        actual: String,
    },

    #[error("object reference is missing required field `{field}`")]
    IncompleteObjectReference { field: &'static str },

    #[error("the `{relation}` operation template was not advertised")]
    MissingTemplate { relation: &'static str },

    #[error("the operation template does not support `{parameter}`")]
    UnsupportedTemplateParameter { parameter: &'static str },

    #[error("invalid operation template `{template}`: {reason}")]
    InvalidTemplate { template: String, reason: String },

    #[error("operation template expanded to invalid target `{target}`: {source}")]
    InvalidExpandedTarget { target: String, source: AdtUriError },

    #[error("invalid object lock response: {0}")]
    InvalidLockResponse(#[from] serde_xml_rs::Error),

    #[error("object lock response did not contain a lock handle")]
    MissingLockHandle,

    #[error("object response was not valid UTF-8: {0}")]
    InvalidResponseEncoding(#[from] std::string::FromUtf8Error),

    #[error("lock for `{actual}` cannot be used with object `{expected}`")]
    ObjectLockMismatch { expected: String, actual: String },

    #[error("updating an object requires a modification lock")]
    ObjectLockNotModifiable,
}

/// An error encoding or decoding repository information system data.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RepositoryError {
    #[error("could not serialize repository information request: {0}")]
    InvalidRequest(#[source] serde_xml_rs::Error),

    #[error("invalid repository information response: {0}")]
    InvalidResponse(#[source] serde_xml_rs::Error),

    #[error("repository object `{name}` advertised invalid URI `{uri}`: {source}")]
    InvalidObjectUri {
        name: String,
        uri: String,
        source: AdtUriError,
    },

    #[error("repository folder `{name}` advertised invalid URI `{uri}`: {source}")]
    InvalidFolderUri {
        name: String,
        uri: String,
        source: AdtUriError,
    },
}

/// An error encoding or decoding Change and Transport System data.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CtsError {
    #[error("could not serialize CTS transport check request: {0}")]
    InvalidTransportCheckRequest(#[source] serde_xml_rs::Error),

    #[error("could not serialize CTS transport creation request: {0}")]
    InvalidTransportCreationRequest(#[source] serde_xml_rs::Error),

    #[error("invalid CTS transport response: {0}")]
    InvalidTransportResponse(#[source] serde_xml_rs::Error),

    #[error("CTS transport creation response was not valid UTF-8: {0}")]
    InvalidTransportCreationResponseEncoding(#[source] std::str::Utf8Error),

    #[error("CTS transport creation response did not contain a transport number")]
    MissingTransportCreationResponse,

    #[error("CTS transport check response was empty")]
    MissingTransportCheckResponse,

    #[error("invalid CTS transport creation reference `{reference}`")]
    InvalidTransportCreationReference { reference: String },
}

impl From<AdtLinkError> for ObjectError {
    fn from(error: AdtLinkError) -> Self {
        let (href, source) = error.into_parts();
        Self::InvalidLink { href, source }
    }
}

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

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ResponseError {
    #[error("ADT returned HTTP status {status}: {exception}")]
    BackendException {
        status: StatusCode,
        exception: Box<AdtException>,
    },

    #[error("ADT returned unexpected HTTP status {status}: {body}")]
    UnexpectedStatus { status: StatusCode, body: String },

    #[error("ADT returned 304 Not Modified without an If-None-Match validator")]
    UnexpectedNotModified,

    #[error("ADT response for collection {category:?} did not include a Content-Type header")]
    MissingContentType { category: CategoryId },

    #[error(
        "ADT response for collection {category:?} used unsupported Content-Type `{content_type}`; supported media types: {supported:?}"
    )]
    UnsupportedContentType {
        category: CategoryId,
        content_type: String,
        supported: Vec<String>,
    },

    #[error(transparent)]
    Batch(#[from] BatchError),

    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error(transparent)]
    Logon(#[from] LogonError),

    #[error(transparent)]
    Object(#[from] ObjectError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error(transparent)]
    Cts(#[from] CtsError),

    #[error("could not serialize object properties as JSON: {0}")]
    JsonSerialization(#[from] serde_json::Error),
}

impl ResponseError {
    pub(crate) fn unexpected_status(response: &AdtResponse) -> Self {
        let status = response.status();
        match AdtException::parse(response.body()) {
            Ok(exception) => Self::BackendException {
                status,
                exception: Box::new(exception),
            },
            Err(_) => Self::UnexpectedStatus {
                status,
                body: String::from_utf8_lossy(response.body()).into_owned(),
            },
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OperationError {
    #[error("operation uses a lock acquired by another user session")]
    UserSessionMismatch,

    #[error(transparent)]
    Compatibility(#[from] CompatibilityError),

    #[error(transparent)]
    Batch(#[from] BatchError),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Object(#[from] ObjectError),

    #[error(transparent)]
    Response(#[from] ResponseError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error(transparent)]
    Cts(#[from] CtsError),
}

/// An error produced while carrying a request through a transport.
#[derive(Debug)]
pub struct TransportError {
    source: Box<dyn StdError + Send + Sync>,
}

impl TransportError {
    pub fn new(error: impl StdError + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(error),
        }
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl StdError for TransportError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
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
