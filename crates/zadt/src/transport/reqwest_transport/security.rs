use std::time::{Duration, Instant};

use async_lock::Mutex;
use http::{HeaderMap, HeaderValue, Method, StatusCode};

use super::connection::HttpConnection;
use crate::{
    AdtRequest, AdtResponse, AdtUri, Logon, Operation, OperationResponse, SessionInformation,
    TransportError,
    api::session::{HTTP_SESSIONS_PATH, SECURITY_SESSION_HEADER},
    protocol::{AdtSessionType, CORE_DISCOVERY_PATH},
};

pub(super) const CSRF_TOKEN_HEADER: &str = "x-csrf-token";
pub(super) const CSRF_FETCH: &str = "Fetch";

#[derive(Default)]
pub(super) struct HttpSecuritySession {
    state: Mutex<SecuritySessionState>,
}

impl HttpSecuritySession {
    /// Prepares the execution of the given request by ensuring all
    /// prerequisites are met, such as a valid CSRF token for mutating
    /// requests.
    ///
    /// The preparation either runs proactively because a session timeout
    /// has been determined due to the server provided session timeout duration,
    /// or because the internal state has previously been marked as reconnect
    /// required by another failing request.
    pub(super) async fn prepare(
        &self,
        connection: &HttpConnection,
        request: &AdtRequest,
    ) -> Result<PreparedSecurity, TransportError> {
        let request_kind = SecurityRequestKind::classify(request);
        let csrf_required = request_kind.requires_csrf_token(request.method());

        let mut state = self.state.lock().await;

        // Make sure not to issue reconnects for initial logon request
        if request_kind.allows_reconnect() && state.requires_reconnect() {
            self.reconnect(connection, &mut state).await?;
        }

        if csrf_required && state.csrf_token.is_none() {
            let mut response = self.fetch_csrf_token(connection).await?;
            if response.status() == StatusCode::UNAUTHORIZED {
                self.reconnect(connection, &mut state).await?;
                response = self.fetch_csrf_token(connection).await?;
            }
            let token = csrf_token_from_response(&response)?;
            state.csrf_token = Some(token);
            state.last_activity = Some(Instant::now());
        }

        let mut header_overrides = HeaderMap::new();
        if csrf_required {
            header_overrides.insert(
                CSRF_TOKEN_HEADER,
                state
                    .csrf_token
                    .clone()
                    .expect("a required CSRF token was fetched"),
            );
        }
        Ok(PreparedSecurity {
            epoch: state.epoch,
            request_kind,
            header_overrides,
        })
    }

    /// Updates the internal security session state from a server response.
    ///
    /// This includes remembering the last session activity for proactive timeout
    /// detection and setting the session information / CSRF tokin from the headers
    /// or body.
    pub(super) async fn observe(&self, prepared: &PreparedSecurity, response: &AdtResponse) {
        let information = if prepared.request_kind == SecurityRequestKind::SessionBootstrap
            && response.status() == StatusCode::OK
        {
            SessionInformation::from_xml(response.body()).ok()
        } else {
            None
        };
        let mut state = self.state.lock().await;
        if state.epoch != prepared.epoch {
            return;
        }
        if response.status() != StatusCode::UNAUTHORIZED {
            state.last_activity = Some(Instant::now());
        }
        let token = valid_csrf_token(response.headers().get(CSRF_TOKEN_HEADER));
        if let Some(information) = information {
            state.epoch = state.epoch.wrapping_add(1);
            state.establish(information);
        }
        if let Some(token) = token {
            state.csrf_token = Some(token);
        }
    }

    /// Invalidates the CSRF token if the response is a 403 with an
    /// attached `x-csrf-token: required` header
    pub(super) async fn invalidate_csrf(
        &self,
        prepared: &PreparedSecurity,
        response: &AdtResponse,
    ) -> bool {
        if prepared.csrf_token().is_none() || !is_csrf_required(response) {
            return false;
        }

        let mut state = self.state.lock().await;
        if state.epoch == prepared.epoch
            && prepared
                .csrf_token()
                .is_some_and(|rejected| state.csrf_token.as_ref() == Some(rejected))
        {
            state.csrf_token = None;
        }
        true
    }

    /// Invalidates the security componont if the response is a 401
    pub(super) async fn invalidate_unauthorized(
        &self,
        prepared: &PreparedSecurity,
        response: &AdtResponse,
    ) -> bool {
        if response.status() != StatusCode::UNAUTHORIZED || !prepared.allows_reconnect() {
            return false;
        }
        let mut state = self.state.lock().await;
        if state.epoch == prepared.epoch {
            state.reconnect_required = true;
        }
        true
    }

    /// Uses the [`Logon`] operation with a `preflight_logon` purpose to
    /// issue a session reconnect.
    ///
    /// This flow must bypass the regular execution flow of [`Transport::send`]
    /// as that would cause a deadlock.
    async fn reconnect(
        &self,
        connection: &HttpConnection,
        state: &mut SecuritySessionState,
    ) -> Result<(), TransportError> {
        // Reset previous state completely
        connection.reset_cookies();
        state.reset();

        // Reuse an encoded logon operation with private dispatch.
        let operation = Logon::default().as_preflight();
        let encoded = operation.encode(&()).map_err(TransportError::new)?;
        let (request, context, _) = encoded.into_parts();

        let response = connection.send(&request, &HeaderMap::new()).await?;
        let information = operation
            .decode(OperationResponse::with_context(response, context))
            .map_err(TransportError::new)?;

        state.epoch = state.epoch.wrapping_add(1);
        state.establish(information);
        Ok(())
    }

    async fn fetch_csrf_token(
        &self,
        connection: &HttpConnection,
    ) -> Result<AdtResponse, TransportError> {
        let mut request = AdtRequest::new(
            Method::GET,
            AdtUri::parse(CORE_DISCOVERY_PATH)
                .expect("the core discovery path must be a valid ADT URI"),
        );
        request
            .headers_mut()
            .insert(CSRF_TOKEN_HEADER, HeaderValue::from_static(CSRF_FETCH));
        request.set_session_type(AdtSessionType::Stateless);
        connection.send(&request, &HeaderMap::new()).await
    }

    #[cfg(test)]
    pub(super) async fn set_inactive(&self, elapsed: Duration, timeout: Duration) {
        let mut state = self.state.lock().await;
        state.last_activity = Some(Instant::now() - elapsed);
        state.inactivity_timeout = Some(timeout);
    }
}

pub(super) struct PreparedSecurity {
    pub(super) epoch: u64,
    request_kind: SecurityRequestKind,
    header_overrides: HeaderMap,
}

impl PreparedSecurity {
    pub(super) fn headers(&self) -> &HeaderMap {
        &self.header_overrides
    }

    pub(super) fn allows_reconnect(&self) -> bool {
        self.request_kind.allows_reconnect()
    }

    fn csrf_token(&self) -> Option<&HeaderValue> {
        self.header_overrides.get(CSRF_TOKEN_HEADER)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SecurityRequestKind {
    Application,
    SessionBootstrap,
}

impl SecurityRequestKind {
    fn classify(request: &AdtRequest) -> Self {
        if is_security_session_request(request) {
            Self::SessionBootstrap
        } else {
            Self::Application
        }
    }

    fn allows_reconnect(self) -> bool {
        self == Self::Application
    }

    fn requires_csrf_token(self, method: &Method) -> bool {
        self == Self::Application
            && !matches!(
                *method,
                Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
            )
    }
}

#[derive(Default)]
struct SecuritySessionState {
    epoch: u64,
    csrf_token: Option<HeaderValue>,
    last_activity: Option<Instant>,
    inactivity_timeout: Option<Duration>,
    reconnect_required: bool,
}

impl SecuritySessionState {
    fn establish(&mut self, information: SessionInformation) {
        self.csrf_token = None;
        self.last_activity = Some(Instant::now());
        self.inactivity_timeout = information.inactivity_timeout;
        self.reconnect_required = false;
    }

    fn reset(&mut self) {
        self.csrf_token = None;
        self.last_activity = None;
        self.inactivity_timeout = None;
        self.reconnect_required = true;
    }

    fn requires_reconnect(&self) -> bool {
        self.reconnect_required
            || self
                .last_activity
                .zip(self.inactivity_timeout)
                .is_some_and(|(last_activity, timeout)| last_activity.elapsed() >= timeout)
    }
}

#[derive(Debug, thiserror::Error)]
enum CsrfTokenError {
    #[error("CSRF token request returned unexpected HTTP status {0}")]
    UnexpectedStatus(StatusCode),

    #[error("CSRF token response did not include x-csrf-token")]
    MissingToken,

    #[error("CSRF token response included invalid x-csrf-token value `{0}`")]
    InvalidToken(String),
}

fn csrf_token_from_response(response: &AdtResponse) -> Result<HeaderValue, TransportError> {
    if response.status() != StatusCode::OK {
        return Err(TransportError::new(CsrfTokenError::UnexpectedStatus(
            response.status(),
        )));
    }
    let token = response
        .headers()
        .get(CSRF_TOKEN_HEADER)
        .cloned()
        .ok_or_else(|| TransportError::new(CsrfTokenError::MissingToken))?;
    valid_csrf_token(Some(&token)).ok_or_else(|| {
        TransportError::new(CsrfTokenError::InvalidToken(
            token.to_str().unwrap_or("<non-text>").to_owned(),
        ))
    })
}

fn valid_csrf_token(token: Option<&HeaderValue>) -> Option<HeaderValue> {
    let token = token?;
    let value = token.to_str().ok()?;
    (!value.is_empty()
        && !value.eq_ignore_ascii_case("required")
        && !value.eq_ignore_ascii_case(CSRF_FETCH))
    .then(|| token.clone())
}

pub(super) fn is_csrf_required(response: &AdtResponse) -> bool {
    response.status() == StatusCode::FORBIDDEN
        && response
            .headers()
            .get(CSRF_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("required"))
}

pub(super) fn is_security_session_request(request: &AdtRequest) -> bool {
    request.target().as_str() == HTTP_SESSIONS_PATH
        && request
            .headers()
            .get(SECURITY_SESSION_HEADER)
            .is_some_and(|value| value == "create")
}
