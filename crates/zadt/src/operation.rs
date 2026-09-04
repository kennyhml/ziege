use std::future::Future;

use http::{HeaderMap, StatusCode, header};

use crate::{
    AdtResponse, AdtUri, Client, ClientState, Discovery, EncodeError, EntityTag, OperationError,
    ResponseError,
    compatibility::{MediaTypes, media_types_match},
    user_session::{UserSession, UserSessionId},
};

mod batch;
mod concurrency;
mod encoded;

pub use batch::{BatchError, BatchKey, BatchOperation, BatchResponses, Batched};
pub use concurrency::{ConditionalResult, IfMatch, IfNoneMatch, Locked, PreconditionResult};
pub use encoded::EncodedOperation;

mod private {
    pub trait Sealed {}
}

/// Identifies whether an ADT operation is [`Stateless`] or [`Stateful`].
///
/// Stateless operations do not require a persistent user session. Stateful
/// operations execute within a [`UserSession`] retained across requests.
///
/// For example, updating program source requires a lock acquired and used within
/// the same user session. The session keeps the lock alive until it is released,
/// closed, or expires.
///
/// SAP exposes these user sessions in transaction `SM04`. For HTTP ADT, the
/// session is identified by the `sap-contextid` cookie. It is distinct from the
/// HTTP security session and from the `sap-usercontext` cookie used to select
/// the SAP client and language.
pub trait OperationKind: private::Sealed + Send + Sync {}

pub struct Stateless;
pub struct Stateful;

impl private::Sealed for Stateless {}
impl private::Sealed for Stateful {}
impl OperationKind for Stateless {}
impl OperationKind for Stateful {}

/// Identifies the context required while encoding an operation.
pub trait ResolutionRequirement: private::Sealed + Send + Sync + 'static {
    type Resolver: ?Sized;
}

/// Encoding does not require an ADT discovery document.
#[derive(Clone, Copy, Debug)]
pub struct Independent;

/// Encoding requires capabilities loaded from ADT discovery.
#[derive(Clone, Copy, Debug)]
pub struct RequiresDiscovery;

impl private::Sealed for Independent {}
impl private::Sealed for RequiresDiscovery {}

impl ResolutionRequirement for Independent {
    type Resolver = ();
}

impl ResolutionRequirement for RequiresDiscovery {
    type Resolver = Discovery;
}

/// A typed ADT operation.
///
/// ADT uses HTTP resource semantics, including methods such as `GET`, `POST`,
/// and `PUT`, resource URIs, headers, and representation bodies.
///
/// [`EncodedOperation`] contains the transport-ready request after target
/// resolution. The operation's [`OperationKind`] and resolution requirement
/// determine which [`Execute`] can run it.
///
/// Consumers of the API should construct operations manually only in exceptional
/// cases. In most scenarios, a callable operation can be constructed - or at least
/// partially derived - from an existing context, such as an object reference.
pub trait Operation: Send + Sync {
    type Response: Send;
    type Kind: OperationKind;
    type ResolutionRequirement: ResolutionRequirement;

    /// Resolves and encodes the operation without consulting transport state.
    fn encode(
        &self,
        resolver: &<Self::ResolutionRequirement as ResolutionRequirement>::Resolver,
    ) -> Result<EncodedOperation, EncodeError>;

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

/// Supplies the resolver required to encode an operation.
pub trait Resolves<R: ResolutionRequirement> {
    fn resolver(&self) -> &R::Resolver;
}

/// Initial client can only resolve an independent operation, such as the
/// initial discovery query itself or system logon (not advertised)
impl<S: ClientState> Resolves<Independent> for Client<S> {
    fn resolver(&self) -> &() {
        &()
    }
}

/// Discovery client can resolve operation encodings that rely on discovery
impl Resolves<RequiresDiscovery> for Client<Discovery> {
    fn resolver(&self) -> &Discovery {
        self.discovery()
    }
}

/// An execution context capable of executing operation `O`.
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

/// Local request metadata retained until an operation decodes its response.
///
/// Executors derive this from the final request and attach it to
/// [`OperationResponse`], which wraps the raw [`AdtResponse`].
///
/// This way, the operation decoders gain access to additional execution
/// context which is needed, for example, for the resolved target URI to
/// bind owned resources to as the response usually contains relative links.
#[derive(Clone, Debug)]
pub struct OperationContext {
    target: AdtUri,
}

impl OperationContext {
    pub(crate) fn new(request_target: AdtUri) -> Self {
        Self {
            target: request_target,
        }
    }

    /// Returns the target of the originating request.
    pub fn request_target(&self) -> &AdtUri {
        &self.target
    }
}

/// A raw ADT response decorated with local context of the producing request.
///
/// Keeping both values together also ensures that a batch decoder can pass
/// each response part the target of its corresponding inner request. This
/// prevents a logical error where the operation context of the batch request
/// itself would be passed to the inner responses.
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

    /// Returns the HTTP-like response status.
    pub fn status(&self) -> StatusCode {
        self.response.status()
    }

    /// Requires the response to have one exact status.
    pub fn require_status(&self, expected: StatusCode) -> Result<(), ResponseError> {
        if self.status() == expected {
            Ok(())
        } else {
            Err(ResponseError::unexpected_status(&self.response))
        }
    }

    /// Requires the response to have a successful status.
    pub fn require_success(&self) -> Result<(), ResponseError> {
        if self.status().is_success() {
            Ok(())
        } else {
            Err(ResponseError::unexpected_status(&self.response))
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

        if supported.iter().any(|v| media_types_match(v, content_type)) {
            Ok(content_type)
        } else {
            Err(ResponseError::UnsupportedContentType {
                target: self.request_target().clone(),
                content_type: content_type.to_owned(),
                supported: supported.iter().map(|v| (*v).to_owned()).collect(),
            })
        }
    }

    pub(crate) fn require_supported_media_type(
        &self,
        supported: MediaTypes,
    ) -> Result<&'static str, ResponseError> {
        let content_type =
            self.content_type()
                .ok_or_else(|| ResponseError::MissingContentType {
                    target: self.request_target().clone(),
                })?;

        supported
            .matching(content_type)
            .ok_or_else(|| ResponseError::UnsupportedContentType {
                target: self.request_target().clone(),
                content_type: content_type.to_owned(),
                supported: supported
                    .as_slice()
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            })
    }

    /// Returns the response entity tag when its header value is valid text.
    pub fn etag(&self) -> Option<EntityTag> {
        self.response.etag()
    }

    /// Consumes the response context and returns its body.
    pub fn into_body(self) -> Vec<u8> {
        self.response.into_body()
    }

    /// Consumes the response context and returns the raw response and request target.
    pub fn into_parts(self) -> (AdtResponse, AdtUri) {
        (self.response, self.context.target)
    }
}

// Execution of a stateless request
impl<S, O> Execute<O> for Client<S>
where
    S: ClientState,
    O: Operation<Kind = Stateless>,
    Client<S>: Resolves<O::ResolutionRequirement>,
{
    async fn execute(&self, operation: &O) -> Result<O::Response, OperationError> {
        let encoded = operation.encode(self.resolver())?;
        let response = self.execute_encoded(encoded).await?;
        Ok(operation.decode(response)?)
    }
}

// Execution of a stateful request
impl<S, O> Execute<O> for UserSession<S>
where
    S: ClientState,
    O: Operation,
    Client<S>: Resolves<O::ResolutionRequirement>,
{
    async fn execute(&self, operation: &O) -> Result<O::Response, OperationError> {
        let encoded = operation.encode(self.client().resolver())?;
        let response = self.execute_encoded(encoded).await?;
        Ok(operation.decode(response)?)
    }
}

impl<S> Client<S>
where
    S: ClientState,
{
    /// Passes the fully resolved request onto the transport dispatcher for execution.
    ///
    /// Because this only handles stateless requests, the presence of a user session
    /// is considered an error as it can cause unexpected behavior on the backend.
    pub(crate) async fn execute_encoded(
        &self,
        encoded: EncodedOperation,
    ) -> Result<OperationResponse, OperationError> {
        let (request, context, bound_user_session) = encoded.into_parts();
        if bound_user_session.is_some() {
            return Err(OperationError::UserSessionMismatch);
        }

        let response = self.transport().send(request).await?;
        Ok(OperationResponse::with_context(response, context))
    }
}

impl<S> UserSession<S>
where
    S: ClientState,
{
    /// Passes the fully resolved request onto the transport dispatcher for execution.
    ///
    /// Because this handles stateful requests, additonal precautions surrounding
    /// the related user sessions must be taken, such as checking for session
    /// expiry, mismatched session ids and expiration responses.
    pub(crate) async fn execute_encoded(
        &self,
        encoded: EncodedOperation,
    ) -> Result<OperationResponse, OperationError> {
        let (mut request, context, bound_user_session) = encoded.into_parts();
        if bound_user_session.is_some_and(|s| s != self.id()) {
            return Err(OperationError::UserSessionMismatch);
        }

        let target = context.request_target().clone();
        let mut session = self.state().lock().await;

        // We might be able to infer expiration based on what the server told us
        // about the max session timeout
        session.expire_if_elapsed();
        if session.is_expired() {
            return Err(ResponseError::UserSessionExpired { target }.into());
        }

        // Mount the user session context on the request and update it based
        // on the response. This may expire the user session after the call.
        session.decorate(&mut request)?;
        let response = self.client().transport().send(request).await?;
        session.update(&response);

        if session.is_expired() {
            return Err(ResponseError::UserSessionExpired { target }.into());
        }
        Ok(OperationResponse::with_context(response, context).in_user_session(self.id()))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex as StdMutex},
    };

    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, Method, StatusCode};

    use super::*;
    use crate::{
        AdtRequest, AdtUri, Transport, TransportError,
        protocol::{ADT_SESSION_TYPE_HEADER, AdtSessionType},
    };

    struct StatefulProbe;

    struct StatelessProbe;

    impl Operation for StatefulProbe {
        type Response = AdtUri;
        type Kind = Stateful;
        type ResolutionRequirement = Independent;

        fn encode(&self, _resolver: &()) -> Result<EncodedOperation, EncodeError> {
            Ok(EncodedOperation::new(
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
        type ResolutionRequirement = Independent;

        fn encode(&self, _resolver: &()) -> Result<EncodedOperation, EncodeError> {
            Ok(EncodedOperation::new(
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
        assert_eq!(response.etag().as_deref(), Some("response-etag"));

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
            requests[0].get(ADT_SESSION_TYPE_HEADER).unwrap(),
            AdtSessionType::Stateful.as_str()
        );
        assert!(!requests[0].contains_key(header::COOKIE));
        assert_eq!(
            requests[1].get(header::COOKIE).unwrap(),
            "sap-contextid=context-1"
        );
    }

    #[tokio::test]
    async fn expired_user_session_fails_without_replaying_or_reopening() {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let transport = ContextFixtureTransport {
            requests: Arc::clone(&requests),
            responses: StdMutex::new(VecDeque::from([AdtResponse::new(
                StatusCode::BAD_REQUEST,
                HeaderMap::new(),
                b"ICMENOSESSION".to_vec(),
            )])),
        };
        let session = Client::new(transport).create_user_session();

        let first = StatefulProbe.execute(&session).await.unwrap_err();
        let second = StatefulProbe.execute(&session).await.unwrap_err();

        assert!(matches!(
            first,
            OperationError::Response(ResponseError::UserSessionExpired { .. })
        ));
        assert!(matches!(
            second,
            OperationError::Response(ResponseError::UserSessionExpired { .. })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}
