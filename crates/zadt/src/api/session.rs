use std::{
    fmt,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use http::{HeaderValue, Method, StatusCode, header};
use serde::Deserialize;
use url::Url;

use crate::{
    AdtRequest, Client, ClientState, LogonError, Operation, OperationError, OperationResponse,
    ResponseError, Stateless,
    compatibility::media_types_match,
    target::HTTP_SESSIONS,
    vocabulary::{
        CANCEL_ON_CLOSE_HEADER, LOAD_BALANCER_HEADER, PURPOSE_HEADER, SECURITY_SESSION_HEADER,
    },
};

const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";

pub(crate) fn parse_session_information(body: &[u8]) -> Result<SessionInformation, LogonError> {
    let raw: RawSession = serde_xml_rs::from_reader(body)?;
    SessionInformation::from_raw(raw)
}

/// Information advertised for an authenticated ADT HTTP security session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionInformation {
    /// The resource used to log off the current HTTP security session.
    pub logoff_uri: SessionUri,

    /// The resource used to delete the corresponding security session.
    pub cleanup_uri: SessionUri,

    /// Optional information about the authenticated SAP system and user.
    pub system_information: Option<SystemInformationLink>,

    /// The backend-advertised inactivity timeout, when positive.
    pub inactivity_timeout: Option<Duration>,
}

impl SessionInformation {
    const LOGOFF_RELATION: &str = "http://www.sap.com/adt/categories/core/http/sessions/logoff";
    const CLEANUP_RELATION: &str =
        "http://www.sap.com/adt/categories/core/http/sessions/securitysession";
    const SYSTEM_INFORMATION_RELATION: &str =
        "http://www.sap.com/adt/categories/core/http/system/systeminformation";
    const INACTIVITY_TIMEOUT_PROPERTY: &str = "inactivityTimeout";

    fn from_raw(raw: RawSession) -> Result<Self, LogonError> {
        let logoff =
            find_link(&raw.links, Self::LOGOFF_RELATION).ok_or(LogonError::MissingLogoffLink)?;
        let cleanup =
            find_link(&raw.links, Self::CLEANUP_RELATION).ok_or(LogonError::MissingCleanupLink)?;
        let system_information = find_link(&raw.links, Self::SYSTEM_INFORMATION_RELATION)
            .map(|link| -> Result<SystemInformationLink, LogonError> {
                let media_type = link
                    .media_type
                    .clone()
                    .filter(|value| !value.is_empty())
                    .ok_or(LogonError::MissingSystemInformationContentType)?;
                Ok(SystemInformationLink {
                    target: SessionUri::parse(Self::SYSTEM_INFORMATION_RELATION, &link.href)?,
                    media_type,
                })
            })
            .transpose()?;

        let mut inactivity_timeout = None;
        let mut inactivity_timeout_seen = false;
        if let Some(properties) = raw.properties {
            for property in properties.values {
                if property.name != Self::INACTIVITY_TIMEOUT_PROPERTY {
                    continue;
                }
                if inactivity_timeout_seen {
                    return Err(LogonError::DuplicateInactivityTimeout);
                }
                inactivity_timeout_seen = true;
                let value = property.value.trim();
                let seconds = value.parse::<i64>().map_err(|source| {
                    LogonError::InvalidInactivityTimeout {
                        value: value.to_owned(),
                        source,
                    }
                })?;
                inactivity_timeout = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
            }
        }

        Ok(Self {
            logoff_uri: SessionUri::parse(Self::LOGOFF_RELATION, &logoff.href)?,
            cleanup_uri: SessionUri::parse(Self::CLEANUP_RELATION, &cleanup.href)?,
            system_information,
            inactivity_timeout,
        })
    }
}

/// A validated same-destination URI advertised by the HTTP session resource.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionUri(String);

impl SessionUri {
    const SESSION_LINK_ORIGIN: &str = "https://adt.invalid/";

    fn parse(relation: &str, href: &str) -> Result<Self, LogonError> {
        let base = Url::parse(Self::SESSION_LINK_ORIGIN).expect("the session-link origin is valid");
        let resolved = base.join(href).map_err(|_| LogonError::InvalidLink {
            relation: relation.to_owned(),
            href: href.to_owned(),
        })?;
        if href.is_empty()
            || href.trim() != href
            || href.chars().any(char::is_control)
            || href.contains('\\')
            || href.starts_with("//")
            || resolved.origin() != base.origin()
            || !resolved.path().starts_with("/sap/")
            || resolved.fragment().is_some()
        {
            return Err(LogonError::InvalidLink {
                relation: relation.to_owned(),
                href: href.to_owned(),
            });
        }

        let mut value = resolved.path().to_owned();
        if let Some(query) = resolved.query() {
            value.push('?');
            value.push_str(query);
        }
        Ok(Self(value))
    }

    /// Returns the destination-relative URI.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The optional system-information resource advertised during logon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemInformationLink {
    /// The same-destination system-information target.
    pub target: SessionUri,

    /// The representation media type expected from the target.
    pub media_type: String,
}

fn find_link<'a>(links: &'a [RawLink], relation: &str) -> Option<&'a RawLink> {
    links.iter().rev().find(|link| link.relation == relation)
}

#[derive(Debug, Deserialize)]
#[serde(rename = "http:session")]
struct RawSession {
    #[serde(rename = "atom:link", default)]
    links: Vec<RawLink>,

    #[serde(rename = "http:properties")]
    properties: Option<RawProperties>,
}

#[derive(Debug, Deserialize)]
struct RawLink {
    #[serde(rename = "@href")]
    href: String,

    #[serde(rename = "@rel")]
    relation: String,

    #[serde(rename = "@type")]
    media_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawProperties {
    #[serde(rename = "http:property", default)]
    values: Vec<RawProperty>,
}

#[derive(Debug, Deserialize)]
struct RawProperty {
    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "#text", default)]
    value: String,
}

/// Establishes an authenticated ADT HTTP security session.
///
/// This HTTP-specific operation returns the advertised [`SessionInformation`]
/// without changing client typestate.
#[derive(Clone, Copy, Debug, Default)]
pub struct Logon;

impl<S: ClientState> Operation<S> for Logon {
    type Response = SessionInformation;
    type Kind = Stateless;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = HTTP_SESSIONS.request(Method::GET);
        request.push_query("_", cache_buster());
        request.set_accept(SESSION_MEDIA_TYPE);

        // TODO: These should probably be statically typed enums
        request
            .headers_mut()
            .insert(SECURITY_SESSION_HEADER, HeaderValue::from_static("create"));
        request
            .headers_mut()
            .insert(PURPOSE_HEADER, HeaderValue::from_static("logon"));
        request
            .headers_mut()
            .insert(LOAD_BALANCER_HEADER, HeaderValue::from_static("fetch"));
        request
            .headers_mut()
            .insert(CANCEL_ON_CLOSE_HEADER, HeaderValue::from_static("true"));
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() != StatusCode::OK {
            return Err(ResponseError::unexpected_status(response.response()));
        }
        if response.body().is_empty() {
            return Err(LogonError::MissingResponseBody.into());
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .ok_or(LogonError::MissingContentType)?;
        if !media_types_match(SESSION_MEDIA_TYPE, content_type) {
            return Err(LogonError::UnsupportedContentType {
                content_type: content_type.to_owned(),
            }
            .into());
        }
        parse_session_information(response.body()).map_err(Into::into)
    }
}

fn cache_buster() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_XML: &[u8] = include_bytes!("../../tests/fixtures/http-session-v3.xml");

    #[test]
    fn parses_v3_session_information() {
        let session = parse_session_information(SESSION_XML).unwrap();

        assert_eq!(session.logoff_uri.as_str(), "/sap/public/bc/icf/logoff");
        assert_eq!(
            session.cleanup_uri.as_str(),
            "/sap/bc/adt/core/http/sessions/security-context"
        );
        assert_eq!(session.inactivity_timeout, Some(Duration::from_secs(3600)));
        let system_information = session.system_information.as_ref().unwrap();
        assert_eq!(
            system_information.target.as_str(),
            "/sap/bc/adt/core/http/systeminformation"
        );
        assert_eq!(
            system_information.media_type,
            "application/vnd.sap.adt.core.http.systeminformation.v1+json"
        );
    }
}
