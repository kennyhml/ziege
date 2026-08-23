use std::time::{Duration, Instant};

use async_lock::Mutex;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use secrecy::{ExposeSecret, SecretString};
use uuid::Uuid;

use crate::{
    AdtRequest, AdtResponse, AdtUri, Client, ClientState, OperationError, ResponseError,
    TransportError,
    protocol::{AdtSessionType, CORE_DISCOVERY_PATH},
};

const USER_SESSION_COOKIE: &str = "sap-contextid";
const ICM_NO_SESSION: &[u8] = b"ICMENOSESSION";
const ICM_ERROR_HEADERS: [&str; 2] = ["sap-err-id", "x-sap-icm-err-id"];

/// An opaque local identity used to preserve stateful-operation affinity.
///
/// This is not the SAP `sap-contextid` cookie and contains no server credential.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UserSessionId(Uuid);

impl UserSessionId {
    /// Creates an opaque local identity for one stateful execution context.
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// A long-lived SAP user session for stateful ADT operations.
///
/// SAP calls the stateful ABAP context represented by `sap-contextid` a user
/// session. Active user sessions can be inspected in transaction `SM04`. Do
/// not confuse this with the transports HTTP security session, identified by
/// `SAP_SESSIONID_*`, or the `sap-usercontext` cookie used to select the SAP
/// client and language.
///
/// The session owns a cheap clone of its [`Client`], so it has no borrowing
/// lifetime and can be retained for an entire editing workflow. Client
/// capabilities and the underlying transport remain shared. Requests within
/// one session are serialized, while separate sessions can hold independent
/// `sap-contextid` values.
///
/// A user session can retain locks and other server resources. Call
/// [`UserSession::close`] when the workflow finishes; dropping this value only
/// releases local state and does not notify SAP.
pub struct UserSession<S: ClientState> {
    id: UserSessionId,
    client: Client<S>,
    state: Mutex<UserSessionState>,
}

impl<S> UserSession<S>
where
    S: ClientState,
{
    pub(crate) fn new(client: Client<S>) -> Self {
        Self {
            id: UserSessionId::generate(),
            client,
            state: Mutex::new(UserSessionState::default()),
        }
    }

    pub(crate) fn id(&self) -> UserSessionId {
        self.id
    }

    pub(crate) fn state(&self) -> &Mutex<UserSessionState> {
        &self.state
    }

    /// Returns the client whose capabilities and transport this session uses.
    pub fn client(&self) -> &Client<S> {
        &self.client
    }

    /// Closes this SAP user session and releases its server-side resources.
    ///
    /// If no stateful response established a `sap-contextid`, this returns
    /// without sending a request. Otherwise it performs a safe core-discovery
    /// request carrying the context with `x-sap-adt-sessiontype: stateless`,
    /// leaving the stateful backend session through an existing resource.
    pub async fn close(self) -> Result<(), OperationError> {
        let mut state = self.state.into_inner();
        let Some(cookie) = state.cookie_header()? else {
            return Ok(());
        };
        let target = AdtUri::parse(CORE_DISCOVERY_PATH)
            .expect("the core discovery path must be a valid ADT URI");
        let mut request = AdtRequest::new(http::Method::GET, target);
        request.set_session_type(AdtSessionType::Stateless);
        request.headers_mut().append(header::COOKIE, cookie);
        let response = self.client.transport().send(request).await?;
        if response.status() == StatusCode::OK || is_expired_response(&response) {
            Ok(())
        } else {
            Err(ResponseError::unexpected_status(&response).into())
        }
    }
}

#[derive(Default)]
pub(crate) struct UserSessionState {
    context_id: Option<SecretString>,
    expires_at: Option<Instant>,
    expired: bool,
}

impl UserSessionState {
    pub(crate) fn is_expired(&self) -> bool {
        self.expired
    }

    // Attaches the internal session id cookie to the request headers to be
    // merged by the transport layer later on if needed.
    pub(crate) fn decorate(&mut self, request: &mut AdtRequest) -> Result<(), TransportError> {
        request.set_session_type(AdtSessionType::Stateful);
        if let Some(cookie) = self.cookie_header()? {
            request.headers_mut().append(header::COOKIE, cookie);
        }
        Ok(())
    }

    fn cookie_header(&mut self) -> Result<Option<HeaderValue>, TransportError> {
        self.expire_if_elapsed();
        self.context_id
            .as_ref()
            .map(|context_id| {
                HeaderValue::from_str(&format!(
                    "{USER_SESSION_COOKIE}={}",
                    context_id.expose_secret()
                ))
                .map_err(TransportError::new)
            })
            .transpose()
    }

    // Updates the session id based on the response. This may mean discarding the session
    // if it has expired, or setting / renewing it.
    pub(crate) fn update(&mut self, headers: &HeaderMap) {
        for header in headers.get_all(header::SET_COOKIE) {
            let Some(cookie) = header
                .to_str()
                .ok()
                .and_then(|value| cookie::Cookie::parse(value.to_owned()).ok())
                .filter(|cookie| cookie.name().eq_ignore_ascii_case(USER_SESSION_COOKIE))
            else {
                continue;
            };

            let expired = cookie.value_trimmed().is_empty()
                || cookie
                    .max_age()
                    .is_some_and(|duration| duration.whole_seconds() <= 0)
                || cookie
                    .expires_datetime()
                    .is_some_and(|expires| expires <= cookie::time::OffsetDateTime::now_utc());
            if expired {
                if self.context_id.take().is_some() {
                    self.expired = true;
                }
                self.expires_at = None;
            } else {
                self.context_id = Some(SecretString::from(cookie.value_trimmed().to_owned()));
                self.expires_at = context_cookie_lifetime(&cookie)
                    .and_then(|lifetime| Instant::now().checked_add(lifetime));
            }
        }
    }

    pub(crate) fn expire(&mut self) {
        self.context_id = None;
        self.expires_at = None;
        self.expired = true;
    }

    pub(crate) fn expire_if_elapsed(&mut self) {
        if self
            .expires_at
            .is_some_and(|expires_at| Instant::now() >= expires_at)
        {
            self.expire();
        }
    }
}

pub(crate) fn is_expired_response(response: &AdtResponse) -> bool {
    response.status() == StatusCode::BAD_REQUEST
        && (ICM_ERROR_HEADERS.iter().any(|name| {
            response
                .headers()
                .get_all(*name)
                .iter()
                .any(|value| value.as_bytes().eq_ignore_ascii_case(ICM_NO_SESSION))
        }) || response
            .body()
            .windows(ICM_NO_SESSION.len())
            .any(|window| window.eq_ignore_ascii_case(ICM_NO_SESSION)))
}

fn context_cookie_lifetime(cookie: &cookie::Cookie<'_>) -> Option<Duration> {
    if let Some(max_age) = cookie.max_age() {
        return (max_age.whole_seconds() > 0)
            .then(|| Duration::from_secs(max_age.whole_seconds() as u64));
    }
    let expires = cookie.expires_datetime()?;
    let seconds =
        expires.unix_timestamp() - cookie::time::OffsetDateTime::now_utc().unix_timestamp();
    (seconds > 0).then(|| Duration::from_secs(seconds as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_session_context_expires_locally() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SET_COOKIE,
            HeaderValue::from_static("sap-contextid=context-1; Max-Age=60; Path=/sap/bc/adt"),
        );
        let mut state = UserSessionState::default();
        state.update(&headers);
        state.expires_at = Some(Instant::now());

        assert!(state.cookie_header().unwrap().is_none());
        assert!(state.expired);
    }

    #[test]
    fn expired_session_response_recognizes_sap_error_headers() {
        for name in ICM_ERROR_HEADERS {
            let mut headers = HeaderMap::new();
            headers.insert(name, HeaderValue::from_static("icmenosession"));
            let response = AdtResponse::new(StatusCode::BAD_REQUEST, headers, Vec::new());

            assert!(is_expired_response(&response));
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            ICM_ERROR_HEADERS[0],
            HeaderValue::from_static("ICMENOSESSION"),
        );
        let response = AdtResponse::new(StatusCode::OK, headers, Vec::new());

        assert!(!is_expired_response(&response));
    }
}
