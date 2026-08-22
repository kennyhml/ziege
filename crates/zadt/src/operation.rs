use std::future::Future;

use async_lock::Mutex;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use secrecy::{ExposeSecret, SecretString};
use uuid::Uuid;

use crate::{
    AdtRequest, AdtResponse, AdtUri, Client, ClientState, EncodeError, EntityTag, OperationError,
    Ready, ResolveError, ResponseError, TransportError, compatibility::media_types_match,
    protocol::CORE_DISCOVERY_PATH,
};

mod batch;
mod encoded;
mod revalidation;
mod target;

pub use batch::{BatchError, BatchKey, BatchOperation, BatchResponses, Batched};
use encoded::EncodedTarget;
pub use encoded::{
    Advertised, AdvertisedCollection, AdvertisedTarget, AdvertisedTemplate, DiscoveryDocument,
    EncodedOperation, OperationTarget, Owned,
};
pub use revalidation::{IfNoneMatch, Revalidation};
use target::resolve_advertised;
pub(crate) use target::{CollectionTarget, TemplateTarget};

const ADT_SESSION_TYPE: &str = "x-sap-adt-sessiontype";
const STATEFUL_SESSION_TYPE: &str = "stateful";
const STATELESS_SESSION_TYPE: &str = "stateless";
const USER_SESSION_COOKIE: &str = "sap-contextid";

mod private {
    pub trait Sealed {}
}

/// Identifies whether an ADT operation is [`Stateless`] or [`Stateful`].
///
/// Stateless operations do not require a persistent ABAP user session. They may
/// still use authentication and an HTTP security session.
///
/// Stateful operations execute within a [`UserSession`] retained across requests.
/// For example, updating a program requires a lock acquired and used within the
/// same user session. The session keeps the lock alive until it is released,
/// closed, or expires.
///
/// SAP exposes these user sessions in transaction `SM04`. For HTTP ADT, the
/// session is identified by the `sap-contextid` cookie. It is distinct from the
/// HTTP security session and from the `sap-usercontext` cookie used to select
/// the SAP client and language.
pub trait OperationKind: private::Sealed + Send + Sync {}

/// An operation that does not require a persistent ABAP user session.
#[derive(Debug)]
pub struct Stateless;

/// An operation that requires a persistent ABAP user session.
#[derive(Debug)]
pub struct Stateful;

impl private::Sealed for Stateless {}
impl private::Sealed for Stateful {}
impl OperationKind for Stateless {}
impl OperationKind for Stateful {}

/// A typed ADT operation.
///
/// ADT uses HTTP resource semantics, including methods such as `GET`, `POST`,
/// and `PUT`, resource URIs, headers, and representation bodies.
///
/// [`EncodedOperation`] represents those semantics before an owned or advertised
/// target is resolved into an [`AdtRequest`]. An HTTP transport sends the resolved
/// request as HTTP, while an RFC transport can map the same fields into SAP's
/// `SADT_REST_REQUEST` structure. It does not tunnel a serialized raw HTTP message.
///
/// The operation's [`OperationKind`] and [`Operation::Target`] determine which
/// [`Execute`] can run it.
///
/// Consumers of the API should construct operations manually only in exceptional
/// cases. In most scenarios, a callable operation can be constructed - or at least
/// partially derived - from an existing context, such as an object reference.
pub trait Operation: Send + Sync {
    type Response: Send;
    type Kind: OperationKind;
    type Target: OperationTarget;

    /// Encodes the operation without consulting client or transport state.
    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError>;

    /// Converts the raw transport response into this operations response type.
    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError>;

    /// Convenient forward of [`Execute::execute`] to the operation itself
    fn execute<E>(
        &self,
        executor: &E,
    ) -> impl Future<Output = Result<Self::Response, OperationError>> + Send
    where
        E: Execute<Self>,
        Self: Sized,
    {
        executor.execute(self)
    }
}

/// An execution context capable of running operation `O`.
///
/// `Operation` describes how to build and decode a request, while `Execute`
/// controls how that request is carried out. This separates the operations
/// protocol contract from execution concerns such as target resolution,
/// user-session affinity, session headers, and transport access.
///
/// [`Client<S>`](Client) implements this trait only for [`Stateless`]
/// operations. Consequently, a [`Stateful`] operation cannot execute directly
/// through a client. A [`UserSession`] implements this trait while retaining
/// the required `sap-contextid` and delegating request delivery to its client.
///
/// Callers should use [`Operation::execute`] rather than invoking this directly.
pub trait Execute<O>: Send + Sync
where
    O: Operation,
{
    /// Builds, sends, and decodes one operation within this execution context.
    fn execute(
        &self,
        operation: &O,
    ) -> impl Future<Output = Result<O::Response, OperationError>> + Send;
}

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

/// Local request metadata retained until an operation decodes its response.
///
/// [`Resolve::resolve`] returns this alongside the transport request. Most
/// callers use the built-in executors, which preserve it automatically.
#[derive(Clone, Debug)]
pub struct OperationContext {
    request_target: AdtUri,
}

impl OperationContext {
    pub(crate) fn new(request_target: AdtUri) -> Self {
        Self { request_target }
    }

    /// Returns the target of the originating request.
    pub fn request_target(&self) -> &AdtUri {
        &self.request_target
    }
}

/// A raw ADT response paired with the context of the request that produced it.
///
/// The request target provides the base URI needed to resolve relative links in
/// response representations. Keeping both values together also ensures that a
/// batch decoder can pass each response part the target of its corresponding
/// inner request.
#[derive(Debug)]
pub struct OperationResponse {
    response: AdtResponse,
    context: OperationContext,
    user_session: Option<UserSessionId>,
}

impl OperationResponse {
    /// Pairs a raw response with the target of its originating request.
    pub fn new(response: AdtResponse, request_target: AdtUri) -> Self {
        Self::with_context(response, OperationContext::new(request_target))
    }

    /// Pairs a raw response with context captured from its originating request.
    pub fn with_context(response: AdtResponse, context: OperationContext) -> Self {
        Self {
            response,
            context,
            user_session: None,
        }
    }

    /// Marks the response as executed through one local user-session identity.
    pub fn in_user_session(mut self, user_session: UserSessionId) -> Self {
        self.user_session = Some(user_session);
        self
    }

    /// Returns the local user-session identity that produced this response.
    pub fn user_session(&self) -> Option<UserSessionId> {
        self.user_session
    }

    /// Returns the target of the request that produced this response.
    pub fn request_target(&self) -> &AdtUri {
        self.context.request_target()
    }

    /// Returns the local context captured from the originating request.
    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    /// Returns the raw transport response.
    pub fn response(&self) -> &AdtResponse {
        &self.response
    }

    /// Returns the HTTP-like response status.
    pub fn status(&self) -> StatusCode {
        self.response.status()
    }

    /// Requires the response to have one exact status.
    pub fn require_status(&self, expected: StatusCode) -> Result<(), ResponseError> {
        if self.status() == expected {
            Ok(())
        } else {
            Err(ResponseError::unexpected_status(self.response()))
        }
    }

    /// Requires the response to have a successful status.
    pub fn require_success(&self) -> Result<(), ResponseError> {
        if self.status().is_success() {
            Ok(())
        } else {
            Err(ResponseError::unexpected_status(self.response()))
        }
    }

    /// Returns the response headers.
    pub fn headers(&self) -> &HeaderMap {
        self.response.headers()
    }

    /// Returns the response body.
    pub fn body(&self) -> &[u8] {
        self.response.body()
    }

    /// Returns the response Content-Type header when it contains valid text.
    pub fn content_type(&self) -> Option<&str> {
        self.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
    }

    /// Requires the response Content-Type to match one of the supported media types.
    pub fn require_content_type(&self, supported: &[&str]) -> Result<&str, ResponseError> {
        let content_type =
            self.content_type()
                .ok_or_else(|| ResponseError::MissingContentType {
                    target: self.request_target().clone(),
                })?;
        if supported
            .iter()
            .any(|expected| media_types_match(expected, content_type))
        {
            Ok(content_type)
        } else {
            Err(ResponseError::UnsupportedContentType {
                target: self.request_target().clone(),
                content_type: content_type.to_owned(),
                supported: supported.iter().map(|value| (*value).to_owned()).collect(),
            })
        }
    }

    /// Returns the response entity tag when its header value is valid text.
    pub fn entity_tag(&self) -> Option<EntityTag> {
        self.response.entity_tag()
    }

    /// Consumes the response context and returns its body.
    pub fn into_body(self) -> Vec<u8> {
        self.response.into_body()
    }

    /// Consumes the response context and returns the raw response and request target.
    pub fn into_parts(self) -> (AdtResponse, AdtUri) {
        (self.response, self.context.request_target)
    }
}

/// A transport request paired with local metadata retained for response decoding.
pub struct ResolvedOperation {
    request: AdtRequest,
    context: OperationContext,
    required_user_session: Option<UserSessionId>,
}

impl ResolvedOperation {
    /// Returns the transport-ready ADT request.
    pub fn request(&self) -> &AdtRequest {
        &self.request
    }

    /// Returns the local context to retain while the request is in flight.
    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    /// Returns the user-session identity required by this operation, if any.
    pub fn required_user_session(&self) -> Option<UserSessionId> {
        self.required_user_session
    }

    /// Separates the transport request, response context, and session requirement.
    pub fn into_parts(self) -> (AdtRequest, OperationContext, Option<UserSessionId>) {
        (self.request, self.context, self.required_user_session)
    }
}

/// Resolves an encoded target using an execution context's available state.
pub trait Resolve<T: OperationTarget> {
    fn resolve(&self, operation: EncodedOperation<T>) -> Result<ResolvedOperation, ResolveError>;
}

impl<S: ClientState> Resolve<Owned> for Client<S> {
    fn resolve(
        &self,
        operation: EncodedOperation<Owned>,
    ) -> Result<ResolvedOperation, ResolveError> {
        let operation = operation.request;
        let EncodedTarget::Owned(target) = operation.target else {
            unreachable!("an owned encoded operation must contain an owned target");
        };
        let context = OperationContext::new(target.clone());
        let request = AdtRequest::from_parts(
            operation.method,
            target,
            operation.query,
            operation.headers,
            operation.body,
        );
        Ok(ResolvedOperation {
            request,
            context,
            required_user_session: operation.required_user_session,
        })
    }
}

impl Resolve<Advertised> for Client<Ready> {
    fn resolve(
        &self,
        operation: EncodedOperation<Advertised>,
    ) -> Result<ResolvedOperation, ResolveError> {
        let operation = operation.request;
        let EncodedTarget::Advertised(target) = operation.target else {
            unreachable!("an advertised encoded operation must contain an advertised target");
        };
        let resolved = resolve_advertised(self, target)?;
        let mut query = resolved.query;
        query.extend(operation.query);
        let context = OperationContext::new(resolved.target.clone());
        let request = AdtRequest::from_parts(
            operation.method,
            resolved.target,
            query,
            operation.headers,
            operation.body,
        );
        Ok(ResolvedOperation {
            request,
            context,
            required_user_session: operation.required_user_session,
        })
    }
}

/// A long-lived SAP user session for stateful ADT operations.
///
/// SAP calls the stateful ABAP context represented by `sap-contextid` a user
/// session. Active user sessions can be inspected in transaction `SM04`. Do
/// not confuse this with the transports HTTP security session, identified by
/// `SAP_SESSIONID_*`, or the `sap-usercontext` client/language cookie.
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

// Execution of a stateless request
impl<S, O> Execute<O> for Client<S>
where
    S: ClientState,
    O: Operation<Kind = Stateless>,
    Client<S>: Resolve<O::Target>,
{
    async fn execute(&self, operation: &O) -> Result<O::Response, OperationError> {
        let resolved = self.resolve(operation.encode()?)?;
        let response = self.execute_resolved(resolved).await?;
        Ok(operation.decode(response)?)
    }
}

// Execution within a retained user session. Stateless operations can opt into
// the session when they need affinity with an existing stateful workflow.
impl<S, O> Execute<O> for UserSession<S>
where
    S: ClientState,
    O: Operation,
    Client<S>: Resolve<O::Target>,
{
    async fn execute(&self, operation: &O) -> Result<O::Response, OperationError> {
        let resolved = self.client.resolve(operation.encode()?)?;
        let response = self.execute_resolved(resolved).await?;
        Ok(operation.decode(response)?)
    }
}

impl<S> Client<S>
where
    S: ClientState,
{
    pub(crate) async fn execute_resolved(
        &self,
        resolved: ResolvedOperation,
    ) -> Result<OperationResponse, OperationError> {
        if resolved.required_user_session.is_some() {
            return Err(ResolveError::UserSessionMismatch.into());
        }
        let response = self.transport().send(resolved.request).await?;
        Ok(OperationResponse::with_context(response, resolved.context))
    }
}

impl<S> UserSession<S>
where
    S: ClientState,
{
    pub(crate) async fn execute_resolved(
        &self,
        mut resolved: ResolvedOperation,
    ) -> Result<OperationResponse, OperationError> {
        let mut session = self.state.lock().await;
        if resolved
            .required_user_session
            .is_some_and(|required| required != self.id)
        {
            return Err(ResolveError::UserSessionMismatch.into());
        }
        session.decorate(&mut resolved.request)?;
        let response = self.client.transport().send(resolved.request).await?;
        session.update(response.headers());
        Ok(OperationResponse::with_context(response, resolved.context).in_user_session(self.id))
    }
}

#[derive(Default)]
struct UserSessionState {
    context_id: Option<SecretString>,
}

impl UserSessionState {
    // Attaches the internal session id cookie to the request headers to be
    // merged by the transport layer later on if needed.
    fn decorate(&self, request: &mut AdtRequest) -> Result<(), TransportError> {
        request.headers_mut().insert(
            ADT_SESSION_TYPE,
            HeaderValue::from_static(STATEFUL_SESSION_TYPE),
        );
        if let Some(cookie) = self.cookie_header()? {
            request.headers_mut().append(header::COOKIE, cookie);
        }
        Ok(())
    }

    fn cookie_header(&self) -> Result<Option<HeaderValue>, TransportError> {
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
    fn update(&mut self, headers: &HeaderMap) {
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
                    .is_some_and(|duration| duration.whole_seconds() <= 0);
            self.context_id =
                (!expired).then(|| SecretString::from(cookie.value_trimmed().to_owned()));
        }
    }
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
        let state = self.state.into_inner();
        let Some(cookie) = state.cookie_header()? else {
            return Ok(());
        };
        let target = AdtUri::parse(CORE_DISCOVERY_PATH)
            .expect("the core discovery path must be a valid ADT URI");
        let mut request = AdtRequest::new(http::Method::GET, target);
        request.headers_mut().insert(
            ADT_SESSION_TYPE,
            HeaderValue::from_static(STATELESS_SESSION_TYPE),
        );
        request.headers_mut().append(header::COOKIE, cookie);
        let response = self.client.transport().send(request).await?;
        if response.status() == http::StatusCode::OK {
            Ok(())
        } else {
            Err(ResponseError::unexpected_status(&response).into())
        }
    }
}

impl UserSession<Ready> {
    /// Creates an empty stateful batch bound to this session.
    pub fn batch(&self) -> BatchOperation<'_, Stateful> {
        BatchOperation::for_user_session(self)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex as StdMutex},
    };

    use async_trait::async_trait;
    use http::{HeaderMap, Method, StatusCode};

    use super::*;
    use crate::{AdtUri, Transport};

    struct StatefulProbe;

    struct StatelessProbe;

    impl Operation for StatefulProbe {
        type Response = AdtUri;
        type Kind = Stateful;
        type Target = Owned;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            Ok(EncodedOperation::owned(
                Method::GET,
                AdtUri::parse("/sap/bc/adt/stateful-probe").unwrap(),
            ))
        }

        fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
            response.require_status(StatusCode::OK)?;
            Ok(response.request_target().clone())
        }
    }

    impl Operation for StatelessProbe {
        type Response = AdtUri;
        type Kind = Stateless;
        type Target = Owned;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            Ok(EncodedOperation::owned(
                Method::GET,
                AdtUri::parse("/sap/bc/adt/stateless-probe").unwrap(),
            ))
        }

        fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
            response.require_status(StatusCode::OK)?;
            Ok(response.request_target().clone())
        }
    }

    struct ContextFixtureTransport {
        requests: Arc<StdMutex<Vec<HeaderMap>>>,
        responses: StdMutex<VecDeque<AdtResponse>>,
    }

    #[async_trait]
    impl Transport for ContextFixtureTransport {
        async fn send(&self, request: AdtRequest) -> Result<AdtResponse, TransportError> {
            self.requests
                .lock()
                .unwrap()
                .push(request.headers().clone());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| TransportError::new(std::io::Error::other("no fixture response")))
        }
    }

    #[test]
    fn operation_response_retains_raw_response_and_request_target() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ETAG, HeaderValue::from_static("response-etag"));
        let target = AdtUri::parse("/sap/bc/adt/test/resource").unwrap();
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, b"response".to_vec()),
            target.clone(),
        );

        assert_eq!(response.request_target(), &target);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), b"response");
        assert_eq!(response.entity_tag().as_deref(), Some("response-etag"));

        let (raw, request_target) = response.into_parts();
        assert_eq!(request_target, target);
        assert_eq!(raw.body(), b"response");
    }

    #[test]
    fn operation_response_validates_content_type_against_its_request_target() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=UTF-8"),
        );
        let target = AdtUri::parse("/sap/bc/adt/test/resource").unwrap();
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, Vec::new()),
            target.clone(),
        );

        assert_eq!(
            response.require_content_type(&["application/xml"]).unwrap(),
            "application/xml; charset=UTF-8"
        );
        assert!(matches!(
            response.require_content_type(&["application/json"]),
            Err(ResponseError::UnsupportedContentType {
                target: error_target,
                ..
            }) if error_target == target
        ));
    }

    #[test]
    fn operation_response_reports_its_target_when_content_type_is_missing() {
        let target = AdtUri::parse("/sap/bc/adt/test/resource").unwrap();
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, HeaderMap::new(), Vec::new()),
            target.clone(),
        );

        assert!(matches!(
            response.require_content_type(&["application/xml"]),
            Err(ResponseError::MissingContentType {
                target: error_target
            }) if error_target == target
        ));
    }

    #[tokio::test]
    async fn user_session_is_owned_and_reuses_its_context_id() {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let mut context_headers = HeaderMap::new();
        context_headers.insert(
            header::SET_COOKIE,
            HeaderValue::from_static("sap-contextid=context-1; Path=/sap/bc/adt"),
        );
        let transport = ContextFixtureTransport {
            requests: Arc::clone(&requests),
            responses: StdMutex::new(VecDeque::from([
                AdtResponse::new(StatusCode::OK, context_headers, Vec::new()),
                AdtResponse::new(StatusCode::OK, HeaderMap::new(), Vec::new()),
            ])),
        };
        let session = Client::new(transport).create_user_session();

        fn assert_static<T: 'static>(_value: &T) {}
        assert_static(&session);
        let first_target = StatefulProbe.execute(&session).await.unwrap();
        let second_target = StatelessProbe.execute(&session).await.unwrap();

        assert_eq!(first_target.as_str(), "/sap/bc/adt/stateful-probe");
        assert_eq!(second_target.as_str(), "/sap/bc/adt/stateless-probe");

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].get(ADT_SESSION_TYPE).unwrap(),
            STATEFUL_SESSION_TYPE
        );
        assert!(!requests[0].contains_key(header::COOKIE));
        assert_eq!(
            requests[1].get(header::COOKIE).unwrap(),
            "sap-contextid=context-1"
        );
    }
}
