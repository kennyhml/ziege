use std::{fmt, ops::Deref, str::FromStr};

use http::{
    HeaderMap, HeaderValue, Method, StatusCode,
    header::{self, InvalidHeaderValue},
};

use crate::AdtUri;

pub(crate) const TEXT_PLAIN_MEDIA_TYPE: &str = "text/plain";
pub(crate) const CORE_DISCOVERY_PATH: &str = "/sap/bc/adt/core/discovery";

/// Actions accepted through ADT's `_action` query parameter.
///
/// Values come from `IF_ADT_REST_POST_ACTION`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PostAction {
    Check,
    Activate,
    Lock,
    Unlock,
    Find,
}

impl PostAction {
    pub(crate) const QUERY_PARAMETER: &'static str = "_action";

    /// Returns the exact value expected by ADT.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Check => "CHECK",
            Self::Activate => "ACTIVATE",
            Self::Lock => "LOCK",
            Self::Unlock => "UNLOCK",
            Self::Find => "FIND",
        }
    }
}

/// A transport agnostic request to an ADT resource.
///
/// Different transports preserve the HTTP-like method, target, query,
/// headers, and body semantics. They do not need to tunnel a serialized raw
/// HTTP message.
///
/// For instance, Eclipse still uses RFC connections for on premise systems
/// that simply wrap the HTTP payload. This can be observed in the ABAP
/// communcation Log.
#[derive(Debug)]
pub struct AdtRequest {
    method: Method,
    target: AdtUri,
    query: Vec<(String, String)>,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl AdtRequest {
    pub fn new(method: Method, target: AdtUri) -> Self {
        Self::from_parts(method, target, Vec::new(), HeaderMap::new(), Vec::new())
    }

    /// Creates a resolved request from its transport-neutral components.
    pub fn from_parts(
        method: Method,
        target: AdtUri,
        query: Vec<(String, String)>,
        headers: HeaderMap,
        body: Vec<u8>,
    ) -> Self {
        Self {
            method,
            target,
            query,
            headers,
            body,
        }
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn target(&self) -> &AdtUri {
        &self.target
    }

    pub fn query(&self) -> &[(String, String)] {
        &self.query
    }

    pub fn push_query(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.query.push((name.into(), value.into()));
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Sets the media type accepted for the response.
    pub fn set_accept(&mut self, media_type: &'static str) {
        self.headers
            .insert(header::ACCEPT, HeaderValue::from_static(media_type));
    }

    /// Sets all response media types accepted by the caller.
    pub fn set_accepts(&mut self, media_types: &[&str]) {
        let value = HeaderValue::from_str(&media_types.join(", "))
            .expect("supported media types form a valid non-empty Accept header");
        self.headers.insert(header::ACCEPT, value);
    }

    /// Sets the media type of the request body.
    pub fn set_content_type(&mut self, media_type: &'static str) {
        self.headers
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
    }

    /// Configures cache revalidation with an ETag or an unconditional refresh.
    pub fn set_cache_revalidation(&mut self, if_none_match: Option<&EntityTag>) {
        if let Some(etag) = if_none_match {
            self.headers.remove(header::CACHE_CONTROL);
            self.headers
                .insert(header::IF_NONE_MATCH, etag.as_header_value().clone());
        } else {
            self.headers.remove(header::IF_NONE_MATCH);
            self.headers
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        }
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn set_body(&mut self, body: impl Into<Vec<u8>>) {
        self.body = body.into();
    }

    /// Formats this requests content as a batch part of a `multipart/mixed`
    /// payload. The boundary must be provided by the caller to make that
    /// responsibility explicit.
    pub(crate) fn format_batch_part(&self, boundary: &str) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        output.extend_from_slice(b"Content-Type: application/http\r\n");
        output.extend_from_slice(b"content-transfer-encoding: binary\r\n\r\n");
        output.extend_from_slice(self.method.as_str().as_bytes());
        output.push(b' ');
        output.extend_from_slice(self.target.as_str().as_bytes());
        if !self.query.is_empty() {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for (name, value) in &self.query {
                serializer.append_pair(name, value);
            }
            output.push(b'?');
            output.extend_from_slice(serializer.finish().as_bytes());
        }
        output.extend_from_slice(b" HTTP/1.1\r\n");

        for name in self.headers.keys() {
            for value in self.headers.get_all(name) {
                output.extend_from_slice(name.as_str().as_bytes());
                output.push(b':');
                output.extend_from_slice(value.as_bytes());
                output.extend_from_slice(b"\r\n");
            }
        }
        output.extend_from_slice(b"\r\n");

        if !self.body.is_empty() {
            output.extend_from_slice(&self.body);
            output.extend_from_slice(b"\r\n");
        }

        output
    }

    /// Consumes the request and returns its transport-level components.
    pub fn into_parts(self) -> (Method, AdtUri, Vec<(String, String)>, HeaderMap, Vec<u8>) {
        (
            self.method,
            self.target,
            self.query,
            self.headers,
            self.body,
        )
    }
}

/// A raw response returned by an ADT transport.
#[derive(Debug)]
pub struct AdtResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl AdtResponse {
    pub fn new(status: StatusCode, headers: HeaderMap, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns the response entity tag when its header value is valid text.
    pub fn entity_tag(&self) -> Option<EntityTag> {
        self.headers
            .get(header::ETAG)
            .and_then(EntityTag::from_header_value)
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

/// An entity tag validated for use as an HTTP header value.
///
/// This guarantees header safety but does not enforce the complete HTTP ETag
/// grammar, preserving the unquoted values emitted by some SAP systems.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntityTag(HeaderValue);

impl EntityTag {
    /// Creates an entity tag from a static header value.
    pub fn from_static(value: &'static str) -> Self {
        Self(HeaderValue::from_static(value))
    }

    /// Returns the entity tag as text.
    pub fn as_str(&self) -> &str {
        self.0
            .to_str()
            .expect("an EntityTag always contains visible header text")
    }

    /// Returns the validated HTTP header value.
    pub fn as_header_value(&self) -> &HeaderValue {
        &self.0
    }

    fn from_header_value(value: &HeaderValue) -> Option<Self> {
        value.to_str().ok()?;
        Some(Self(value.clone()))
    }
}

impl FromStr for EntityTag {
    type Err = InvalidHeaderValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        HeaderValue::from_str(value).map(Self)
    }
}

impl TryFrom<String> for EntityTag {
    type Error = InvalidHeaderValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for EntityTag {
    type Error = InvalidHeaderValue;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl serde::Serialize for EntityTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for EntityTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

impl Deref for EntityTag {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for EntityTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for EntityTag {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<EntityTag> for str {
    fn eq(&self, other: &EntityTag) -> bool {
        self == other.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_tags_validate_header_safety_at_construction() {
        assert_eq!(
            EntityTag::try_from("safe-etag").unwrap().as_str(),
            "safe-etag"
        );
        assert!(EntityTag::try_from("etag\r\ninjected: value").is_err());
    }

    #[test]
    fn response_exposes_its_entity_tag() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ETAG, HeaderValue::from_static("response-etag"));
        let response = AdtResponse::new(StatusCode::OK, headers, Vec::new());

        assert_eq!(response.entity_tag().as_deref(), Some("response-etag"));
    }

    #[test]
    fn post_actions_match_if_adt_rest_post_action() {
        assert_eq!(PostAction::Check.as_str(), "CHECK");
        assert_eq!(PostAction::Activate.as_str(), "ACTIVATE");
        assert_eq!(PostAction::Lock.as_str(), "LOCK");
        assert_eq!(PostAction::Unlock.as_str(), "UNLOCK");
        assert_eq!(PostAction::Find.as_str(), "FIND");
    }
}
