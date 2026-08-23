use std::{any::Any, future::Future, marker::PhantomData, sync::Arc};

use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use thiserror::Error;
use uuid::Uuid;

use super::{
    Advertised, CollectionTarget, EncodedOperation, Operation, OperationContext, OperationKind,
    Resolve, Stateful, Stateless,
};
use crate::{
    AdtRequest, AdtResponse, CategoryId, Client, EncodeError, OperationError, OperationResponse,
    Ready, ResolveError, ResponseError, UserSession, UserSessionId,
};

const BATCH_CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/system/communication/services",
    term: "batch",
};
const BATCH_MEDIA_TYPE: &str = "multipart/mixed";
const APPLICATION_HTTP: &str = "application/http";

/// To be able to have a [`BatchOperation`] stick a bunch of operations
/// into a collection, we must be able to reference them by some common trait.
/// While they all implement [`Operation`], the associated response makes them
/// incompatible. So the response type must also be erased!
type ErasedResponse = Box<dyn Any + Send>;

/// An operation that has both its own type and its associated response erased.
///
/// The response becomes a `void*`-like structure that the rust runtime lets us
/// cast safely by inserting type id checks. This relies on the fact that operations
/// come out in the same order they go in - which the ADT backend guarantees.
trait ErasedOperation: Send + Sync {
    fn decode(&self, response: OperationResponse) -> Result<ErasedResponse, ResponseError>;
}

impl<O> ErasedOperation for O
where
    O: Operation + 'static,
    O::Response: 'static,
{
    fn decode(&self, response: OperationResponse) -> Result<ErasedResponse, ResponseError> {
        <O as Operation>::decode(self, response)
            .map(|response| Box::new(response) as ErasedResponse)
    }
}

struct BatchEntry {
    operation: Box<dyn ErasedOperation>,
    request: AdtRequest,
    context: OperationContext,
}

enum BoundExecutor<'a> {
    Client(&'a Client<Ready>),
    UserSession(&'a UserSession<Ready>),
}

impl BoundExecutor<'_> {
    fn client(&self) -> &Client<Ready> {
        match self {
            Self::Client(client) => client,
            Self::UserSession(session) => session.client(),
        }
    }

    fn validate_user_session(
        &self,
        bound_user_session: Option<UserSessionId>,
    ) -> Result<(), ResolveError> {
        match (self, bound_user_session) {
            (_, None) => Ok(()),
            (Self::UserSession(session), Some(bound)) if bound == session.id() => Ok(()),
            _ => Err(ResolveError::UserSessionMismatch),
        }
    }
}

/// A kind-heterogeneous group of ADT operations executed in one HTTP round trip.
///
/// The batch borrows the [`Ready`] client or user session that created it. Each
/// operation is encoded and resolved when passed to [`BatchOperation::push`].
/// Individual response types remain available through the returned [`BatchKey`].
///
/// Create a batch operation through [`Client::batch`] or [`UserSession::batch`]
/// which binds it to an operation kind `K` and the originating executor.
///
/// ADT executes its subrequests and returns their responses in request order.
/// Batch operations cannot be nested.
///
/// If you have a collection of operations of the same type, in other words,
/// they all share the same response, you may also use the [`Batched`] trait.
///
/// TODO: Implement max batch size and max worker count for parallelism even
/// using batching, sweet middle spot for many operations
pub struct BatchOperation<'a, K: OperationKind> {
    executor: BoundExecutor<'a>,
    identity: Arc<()>,
    entries: Vec<BatchEntry>,
    bound_user_session: Option<UserSessionId>,
    kind: PhantomData<fn() -> K>,
}

impl<'a> BatchOperation<'a, Stateless> {
    pub(crate) fn for_client(client: &'a Client<Ready>) -> Self {
        Self::new(BoundExecutor::Client(client))
    }
}

impl<'a> BatchOperation<'a, Stateful> {
    pub(crate) fn for_user_session(session: &'a UserSession<Ready>) -> Self {
        Self::new(BoundExecutor::UserSession(session))
    }
}

impl<'a, K> BatchOperation<'a, K>
where
    K: OperationKind,
{
    fn new(executor: BoundExecutor<'a>) -> Self {
        Self {
            executor,
            identity: Arc::new(()),
            entries: Vec::new(),
            bound_user_session: None,
            kind: PhantomData,
        }
    }

    /// Returns the number of operations in this batch.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether this batch contains no operations.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolves an operation and returns a typed key for its eventual response.
    ///
    /// See [`BatchKey`] for the magic this performs.
    ///
    /// Encoding and target-resolution failures are returned immediately. The
    /// operation is added only after both steps succeed.
    pub fn push<O>(&mut self, operation: O) -> Result<BatchKey<O::Response>, OperationError>
    where
        O: Operation<Kind = K> + 'static,
        O::Response: 'static,
        Client<Ready>: Resolve<O::Target>,
    {
        let resolved = self.executor.client().resolve(operation.encode()?)?;
        self.executor
            .validate_user_session(resolved.bound_user_session)?;
        let key = BatchKey {
            identity: Arc::clone(&self.identity),
            index: self.entries.len(),
            response: PhantomData::<fn() -> O::Response>,
        };
        if let Some(bound) = resolved.bound_user_session {
            self.bound_user_session = Some(bound);
        }
        self.entries.push(BatchEntry {
            operation: Box::new(operation),
            request: resolved.request,
            context: resolved.context,
        });
        Ok(key)
    }

    fn encode(&self) -> Result<EncodedOperation<Advertised>, EncodeError> {
        if self.entries.is_empty() {
            return Err(BatchError::Empty.into());
        }

        let (content_type, body) =
            encode_batch_body(self.entries.iter().map(|entry| &entry.request));
        let mut request = CollectionTarget::core(BATCH_CATEGORY).operation(Method::POST);
        request.set_accept(BATCH_MEDIA_TYPE);
        request
            .headers_mut()
            .insert(header::CONTENT_TYPE, content_type);
        request.set_body(body);
        if let Some(bound) = self.bound_user_session {
            request.bind_user_session(bound);
        }
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<BatchResponses, ResponseError> {
        response.require_status(StatusCode::ACCEPTED)?;
        let user_session = response.user_session();
        let (response, _) = response.into_parts();

        let boundary = response_boundary(response.headers())?;
        let responses = decode_batch(response.body(), &boundary)?;
        if responses.len() != self.entries.len() {
            return Err(BatchError::ResponseCount {
                expected: self.entries.len(),
                actual: responses.len(),
            }
            .into());
        }

        let slots = self
            .entries
            .iter()
            .zip(responses)
            .map(|(entry, response)| {
                let response = OperationResponse::with_context(response, entry.context.clone());
                let response = match user_session {
                    Some(user_session) => response.in_user_session(user_session),
                    None => response,
                };
                entry.operation.decode(response)
            })
            .map(Some)
            .collect();

        Ok(BatchResponses {
            identity: Arc::clone(&self.identity),
            slots,
        })
    }

    /// Sends this batch through the client or user session that created it.
    ///
    /// The outer batch endpoint is resolved for each execution. This permits a
    /// prepared batch to be reused while retaining stateful session cookies.
    pub async fn execute(&self) -> Result<BatchResponses, OperationError> {
        let encoded = self.encode()?;
        let resolved = self.executor.client().resolve(encoded)?;
        let response = match &self.executor {
            BoundExecutor::Client(client) => client.execute_resolved(resolved).await?,
            BoundExecutor::UserSession(session) => session.execute_resolved(resolved).await?,
        };
        Ok(self.decode(response)?)
    }
}

fn encode_batch_body<'a>(
    requests: impl IntoIterator<Item = &'a AdtRequest>,
) -> (HeaderValue, Vec<u8>) {
    let boundary = format!("batch_{}", Uuid::new_v4());
    let content_type = HeaderValue::from_str(&format!("{BATCH_MEDIA_TYPE}; boundary={boundary}"))
        .expect("a UUID batch boundary is a valid Content-Type parameter");
    let closing_boundary = format!("--{boundary}--");
    let body = requests
        .into_iter()
        .flat_map(|request| request.format_batch_part(&boundary))
        .chain(closing_boundary.bytes())
        .collect();
    (content_type, body)
}

/// A typed reference to one response slot in a batch.
pub struct BatchKey<R> {
    identity: Arc<()>,
    index: usize,
    response: PhantomData<fn() -> R>,
}

impl<R> Clone for BatchKey<R> {
    fn clone(&self) -> Self {
        Self {
            identity: Arc::clone(&self.identity),
            index: self.index,
            response: PhantomData,
        }
    }
}

/// Individually decoded responses from a heterogeneous batch.
#[derive(Debug)]
pub struct BatchResponses {
    identity: Arc<()>,
    slots: Vec<Option<Result<ErasedResponse, ResponseError>>>,
}

impl BatchResponses {
    /// Returns the total number of response slots in this batch.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns whether this batch has no response slots.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Takes and downcasts the response associated with `key`.
    ///
    /// A failed subrequest does not prevent other response slots from being
    /// retrieved. Its operation-specific decoding error is returned here.
    pub fn take<R>(&mut self, key: BatchKey<R>) -> Result<R, BatchError>
    where
        R: Send + 'static,
    {
        if !Arc::ptr_eq(&self.identity, &key.identity) {
            return Err(BatchError::ForeignBatch);
        }
        let slot = self
            .slots
            .get_mut(key.index)
            .ok_or(BatchError::MissingResponse { index: key.index })?
            .take()
            .ok_or(BatchError::MissingResponse { index: key.index })?;

        let response = slot.map_err(|source| BatchError::Decode {
            index: key.index,
            source: Box::new(source),
        })?;
        response
            .downcast::<R>()
            .map(|response| *response)
            .map_err(|_| BatchError::TypeMismatch { index: key.index })
    }
}

/// An execution context that can construct and execute a batch of kind `K`.
///
/// This is needed so we can treat both, a [`UserSession`] and [`Client`] as
/// capable of creating a batch for their associated statefulness.
trait CreateBatch<K>: Sync
where
    K: OperationKind,
{
    fn create_batch(&self) -> BatchOperation<'_, K>;
}

impl CreateBatch<Stateless> for Client<Ready> {
    fn create_batch(&self) -> BatchOperation<'_, Stateless> {
        Client::batch(self)
    }
}

impl UserSession<Ready> {
    /// Creates an empty stateful batch bound to this session.
    pub fn batch(&self) -> BatchOperation<'_, Stateful> {
        BatchOperation::for_user_session(self)
    }
}

impl CreateBatch<Stateful> for UserSession<Ready> {
    fn create_batch(&self) -> BatchOperation<'_, Stateful> {
        UserSession::batch(self)
    }
}

/// Allows calling `T.batch()` on any homogenous collection of [`Operation`] `T`
/// that implements [`IntoIterator`] and get back a [`Vec<T::Response>`] where
/// `result[n]` is the response for operation `self[n]`.
///
/// This does not work on arrays of boxed, type erased operations as there is no
/// well defined return type.
pub trait Batched<E> {
    type Response: Send;

    fn batched(
        self,
        executor: &E,
    ) -> impl Future<Output = Result<Vec<Self::Response>, OperationError>> + Send
    where
        Self: Sized;
}

impl<I, E> Batched<E> for I
where
    I: IntoIterator + Send,
    I::Item: Operation + 'static,
    <I::Item as Operation>::Response: 'static,
    E: CreateBatch<<I::Item as Operation>::Kind>,
    Client<Ready>: Resolve<<I::Item as Operation>::Target>,
{
    type Response = <I::Item as Operation>::Response;

    /// This is the main boilerplate this function saves us from writing
    async fn batched(self, executor: &E) -> Result<Vec<Self::Response>, OperationError> {
        let mut batch = executor.create_batch();
        let keys = self
            .into_iter()
            .map(|operation| batch.push(operation))
            .collect::<Result<Vec<_>, _>>()?;

        let mut responses = batch.execute().await?;
        keys.into_iter()
            .map(|key| {
                responses
                    .take(key)
                    .map_err(ResponseError::from)
                    .map_err(OperationError::from)
            })
            .collect()
    }
}

/// An error constructing, parsing, correlating, or retrieving a batch value.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BatchError {
    #[error("a batch must contain at least one operation")]
    Empty,

    #[error("batch response did not include a Content-Type header")]
    MissingContentType,

    #[error("batch response used unsupported Content-Type `{content_type}`")]
    UnsupportedContentType { content_type: String },

    #[error("batch Content-Type `{content_type}` did not include a boundary")]
    MissingBoundary { content_type: String },

    #[error("batch Content-Type contained an invalid boundary")]
    InvalidBoundary,

    #[error("invalid multipart batch response: {reason}")]
    InvalidMultipart { reason: String },

    #[error("invalid multipart batch response part {index}: {reason}")]
    InvalidPart { index: usize, reason: String },

    #[error("batch returned {actual} response parts for {expected} operations")]
    ResponseCount { expected: usize, actual: usize },

    #[error("batch value belongs to a different batch operation")]
    ForeignBatch,

    #[error("batch response slot {index} is missing or was already taken")]
    MissingResponse { index: usize },

    #[error("batch response slot {index} could not be decoded: {source}")]
    Decode {
        index: usize,
        #[source]
        source: Box<ResponseError>,
    },

    #[error("batch response slot {index} did not contain its registered response type")]
    TypeMismatch { index: usize },
}

const BINARY: &str = "binary";
const CRLF: &[u8] = b"\r\n";
const MAX_PART_HEADERS: usize = 128;

fn response_boundary(headers: &HeaderMap) -> Result<String, BatchError> {
    let value = headers
        .get(header::CONTENT_TYPE)
        .ok_or(BatchError::MissingContentType)?;
    let content_type = value
        .to_str()
        .map_err(|_| BatchError::UnsupportedContentType {
            content_type: String::from_utf8_lossy(value.as_bytes()).into_owned(),
        })?;
    let mut fields = content_type.split(';');
    let media_type = fields.next().unwrap_or_default().trim();
    if !media_type.eq_ignore_ascii_case(BATCH_MEDIA_TYPE) {
        return Err(BatchError::UnsupportedContentType {
            content_type: content_type.to_owned(),
        });
    }

    let boundary = fields.find_map(|field| {
        let (name, value) = field.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("boundary")
            .then(|| value.trim())
    });
    let Some(boundary) = boundary else {
        return Err(BatchError::MissingBoundary {
            content_type: content_type.to_owned(),
        });
    };
    let boundary = if boundary.starts_with('"') && boundary.ends_with('"') && boundary.len() >= 2 {
        &boundary[1..boundary.len() - 1]
    } else {
        boundary
    };
    if boundary.is_empty() || boundary.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        return Err(BatchError::InvalidBoundary);
    }
    Ok(boundary.to_owned())
}

fn decode_batch(body: &[u8], boundary: &str) -> Result<Vec<AdtResponse>, BatchError> {
    multipart_parts(body, boundary)?
        .into_iter()
        .enumerate()
        .map(|(index, part)| decode_part(index, part))
        .collect()
}

fn multipart_parts<'a>(body: &'a [u8], boundary: &str) -> Result<Vec<&'a [u8]>, BatchError> {
    let marker = format!("--{boundary}").into_bytes();
    let closing_marker = [marker.as_slice(), b"--"].concat();
    let mut parts = Vec::new();
    let mut part_start = None;
    let mut line_start = 0;

    loop {
        let line_end = find_bytes(&body[line_start..], CRLF)
            .map(|offset| line_start + offset)
            .unwrap_or(body.len());
        let next_line = (line_end < body.len()).then_some(line_end + CRLF.len());
        let line = &body[line_start..line_end];

        if line == marker {
            if let Some(start) = part_start {
                parts.push(&body[start..line_start]);
            }
            part_start = Some(next_line.ok_or_else(|| BatchError::InvalidMultipart {
                reason: "opening boundary was not followed by CRLF".to_owned(),
            })?);
        } else if line == closing_marker {
            let Some(start) = part_start else {
                return Err(BatchError::InvalidMultipart {
                    reason: "closing boundary appeared before an opening boundary".to_owned(),
                });
            };
            if start < line_start {
                parts.push(&body[start..line_start]);
            }
            return Ok(parts);
        }

        let Some(next_line) = next_line else {
            break;
        };
        line_start = next_line;
    }

    Err(BatchError::InvalidMultipart {
        reason: if part_start.is_some() {
            "closing boundary was not found".to_owned()
        } else {
            "opening boundary was not found".to_owned()
        },
    })
}

fn decode_part(index: usize, part: &[u8]) -> Result<AdtResponse, BatchError> {
    let (embedded_offset, meta_headers) = decode_mime_headers(index, part)?;
    let content_type = meta_headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim);
    if !content_type.is_some_and(|value| value.eq_ignore_ascii_case(APPLICATION_HTTP)) {
        return Err(invalid_part(
            index,
            "MIME Content-Type must be application/http",
        ));
    }
    if let Some(encoding) = meta_headers
        .get("content-transfer-encoding")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        && !encoding.eq_ignore_ascii_case(BINARY)
    {
        return Err(invalid_part(
            index,
            "MIME content-transfer-encoding must be binary",
        ));
    }

    let embedded_response = &part[embedded_offset..];
    let mut raw_headers = [httparse::EMPTY_HEADER; MAX_PART_HEADERS];
    let mut parsed = httparse::Response::new(&mut raw_headers);
    let response_offset = match parsed
        .parse(embedded_response)
        .map_err(|error| invalid_part(index, &format!("invalid embedded HTTP response: {error}")))?
    {
        httparse::Status::Complete(offset) => offset,
        httparse::Status::Partial => {
            return Err(invalid_part(index, "incomplete embedded HTTP response"));
        }
    };
    let status = parsed
        .code
        .and_then(|status| StatusCode::from_u16(status).ok())
        .ok_or_else(|| invalid_part(index, "invalid embedded HTTP status code"))?;
    let headers = decode_headers(index, parsed.headers)?;

    let mut response_body = &embedded_response[response_offset..];
    if let Some(body) = response_body.strip_suffix(CRLF) {
        response_body = body;
    }
    Ok(AdtResponse::new(status, headers, response_body.to_vec()))
}

fn decode_mime_headers(index: usize, part: &[u8]) -> Result<(usize, HeaderMap), BatchError> {
    let mut raw_headers = [httparse::EMPTY_HEADER; MAX_PART_HEADERS];
    let (offset, raw_headers) = match httparse::parse_headers(part, &mut raw_headers)
        .map_err(|error| invalid_part(index, &format!("invalid MIME headers: {error}")))?
    {
        httparse::Status::Complete(parsed) => parsed,
        httparse::Status::Partial => return Err(invalid_part(index, "incomplete MIME headers")),
    };
    Ok((offset, decode_headers(index, raw_headers)?))
}

fn decode_headers(
    index: usize,
    raw_headers: &[httparse::Header<'_>],
) -> Result<HeaderMap, BatchError> {
    let mut headers = HeaderMap::new();
    for raw_header in raw_headers {
        let name = HeaderName::from_bytes(raw_header.name.as_bytes())
            .map_err(|_| invalid_part(index, "invalid header name"))?;
        let value = HeaderValue::from_bytes(raw_header.value)
            .map_err(|_| invalid_part(index, "invalid header value"))?;
        headers.append(name, value);
    }
    Ok(headers)
}

fn invalid_part(index: usize, reason: &str) -> BatchError {
    BatchError::InvalidPart {
        index,
        reason: reason.to_owned(),
    }
}

fn find_bytes(value: &[u8], needle: &[u8]) -> Option<usize> {
    value
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex as StdMutex},
    };

    use async_trait::async_trait;

    use super::*;
    use crate::{
        AdtUri, Owned, Ready, ResolveError, Stateful, Stateless, Transport, TransportError,
        api::discovery::parse_capabilities,
        protocol::{ADT_SESSION_TYPE_HEADER, AdtSessionType},
    };

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");
    const CORE_DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/core-discovery.xml");
    const RESPONSE_BOUNDARY: &str = "batch_00112233445566778899AABBCCDDEEFF";

    struct TextOperation;

    impl Operation for TextOperation {
        type Response = String;
        type Kind = Stateless;
        type Target = Owned;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            let mut request = EncodedOperation::owned(
                Method::GET,
                AdtUri::parse("/sap/bc/adt/test/text").unwrap(),
            );
            request.push_query("name", "hello world");
            request
                .headers_mut()
                .insert(header::ACCEPT, HeaderValue::from_static("text/plain"));
            Ok(request)
        }

        fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
            assert_eq!(response.request_target().as_str(), "/sap/bc/adt/test/text");
            expect_ok(response).map(|body| String::from_utf8_lossy(&body).into_owned())
        }
    }

    struct CountOperation;

    impl Operation for CountOperation {
        type Response = usize;
        type Kind = Stateless;
        type Target = Owned;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            Ok(EncodedOperation::owned(
                Method::GET,
                AdtUri::parse("/sap/bc/adt/test/count").unwrap(),
            ))
        }

        fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
            assert_eq!(response.request_target().as_str(), "/sap/bc/adt/test/count");
            let body = expect_ok(response)?;
            Ok(String::from_utf8_lossy(&body).parse().unwrap())
        }
    }

    struct MissingAdvertisedOperation;

    impl Operation for MissingAdvertisedOperation {
        type Response = ();
        type Kind = Stateless;
        type Target = Advertised;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            Ok(CollectionTarget::new(CategoryId {
                scheme: "https://example.test/categories",
                term: "missing",
            })
            .operation(Method::GET))
        }

        fn decode(&self, _response: OperationResponse) -> Result<Self::Response, ResponseError> {
            Ok(())
        }
    }

    struct StatefulTextOperation;

    impl Operation for StatefulTextOperation {
        type Response = String;
        type Kind = Stateful;
        type Target = Owned;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            Ok(EncodedOperation::owned(
                Method::GET,
                AdtUri::parse("/sap/bc/adt/test/stateful").unwrap(),
            ))
        }

        fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
            expect_ok(response).map(|body| String::from_utf8_lossy(&body).into_owned())
        }
    }

    struct SessionBoundOperation(UserSessionId);

    impl Operation for SessionBoundOperation {
        type Response = ();
        type Kind = Stateful;
        type Target = Owned;

        fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
            let mut request = EncodedOperation::owned(
                Method::PUT,
                AdtUri::parse("/sap/bc/adt/test/session-bound").unwrap(),
            );
            request.bind_user_session(self.0);
            Ok(request)
        }

        fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
            expect_ok(response).map(|_| ())
        }
    }

    fn expect_ok(response: OperationResponse) -> Result<Vec<u8>, ResponseError> {
        response.require_status(StatusCode::OK)?;
        Ok(response.into_body())
    }

    struct FixtureTransport {
        requests: Arc<StdMutex<Vec<AdtRequest>>>,
        responses: StdMutex<VecDeque<AdtResponse>>,
    }

    #[async_trait]
    impl Transport for FixtureTransport {
        async fn send(&self, request: AdtRequest) -> Result<AdtResponse, TransportError> {
            self.requests.lock().unwrap().push(request);
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| TransportError::new(std::io::Error::other("no fixture response")))
        }
    }

    fn fixture_response(parts: &[(&str, StatusCode)]) -> AdtResponse {
        fixture_response_with_headers(parts, HeaderMap::new())
    }

    fn fixture_response_with_headers(
        parts: &[(&str, StatusCode)],
        mut headers: HeaderMap,
    ) -> AdtResponse {
        let mut body = Vec::new();
        for (content, status) in parts {
            body.extend_from_slice(format!("--{RESPONSE_BOUNDARY}\r\n").as_bytes());
            body.extend_from_slice(b"content-type: application/http\r\n");
            body.extend_from_slice(b"content-transfer-encoding: binary\r\n\r\n");
            body.extend_from_slice(
                format!(
                    "HTTP/1.1 {} fixture\r\nContent-Type: text/plain\r\n\r\n",
                    status.as_u16()
                )
                .as_bytes(),
            );
            body.extend_from_slice(content.as_bytes());
            body.extend_from_slice(CRLF);
        }
        body.extend_from_slice(format!("--{RESPONSE_BOUNDARY}--\r\n").as_bytes());

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/mixed; boundary={RESPONSE_BOUNDARY}"))
                .unwrap(),
        );
        AdtResponse::new(StatusCode::ACCEPTED, headers, body)
    }

    fn fixture_client(
        responses: Vec<AdtResponse>,
    ) -> (Client<Ready>, Arc<StdMutex<Vec<AdtRequest>>>) {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let transport = FixtureTransport {
            requests: Arc::clone(&requests),
            responses: StdMutex::new(responses.into()),
        };
        let client = Client::new(transport).with_capabilities(
            parse_capabilities(DISCOVERY_XML).unwrap(),
            parse_capabilities(CORE_DISCOVERY_XML).unwrap(),
        );
        (client, requests)
    }

    #[tokio::test]
    async fn executes_and_decodes_heterogeneous_operations() {
        let (client, requests) = fixture_client(vec![fixture_response(&[
            ("hello", StatusCode::OK),
            ("42", StatusCode::OK),
        ])]);
        let mut batch = client.batch();
        let text = batch.push(TextOperation).unwrap();
        let count = batch.push(CountOperation).unwrap();

        let mut responses = batch.execute().await.unwrap();

        assert_eq!(responses.len(), 2);
        assert_eq!(responses.take(text).unwrap(), "hello");
        assert_eq!(responses.take(count).unwrap(), 42);

        let requests = requests.lock().unwrap();
        let request = &requests[0];
        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.target().as_str(), "/sap/bc/adt/communication/batch");
        assert_eq!(
            request.headers().get(header::ACCEPT).unwrap(),
            BATCH_MEDIA_TYPE
        );
        let boundary = response_boundary(request.headers()).unwrap();
        assert!(boundary.starts_with("batch_"));
        let body = String::from_utf8(request.body().to_vec()).unwrap();
        assert!(body.contains(&format!("--{boundary}\r\nContent-Type: application/http")));
        assert!(body.contains("GET /sap/bc/adt/test/text?name=hello+world HTTP/1.1\r\n"));
        assert!(body.contains("accept:text/plain\r\n\r\n"));
        assert!(body.contains("GET /sap/bc/adt/test/count HTTP/1.1\r\n\r\n"));
        assert!(body.ends_with(&format!("--{boundary}--")));
    }

    #[test]
    fn push_resolves_an_operation_before_transport() {
        let (client, requests) = fixture_client(Vec::new());
        let mut batch = client.batch();

        let Err(error) = batch.push(MissingAdvertisedOperation) else {
            panic!("operation with a missing advertised target was accepted");
        };

        assert!(matches!(
            error,
            OperationError::Resolve(ResolveError::Compatibility(
                crate::CompatibilityError::MissingCollection(_)
            ))
        ));
        assert!(batch.is_empty());
        assert!(requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn executes_homogeneous_operations_through_the_client() {
        let (client, _) = fixture_client(vec![fixture_response(&[
            ("first", StatusCode::OK),
            ("second", StatusCode::OK),
        ])]);

        let responses = [TextOperation, TextOperation]
            .batched(&client)
            .await
            .unwrap();

        assert_eq!(responses, ["first", "second"]);
    }

    #[test]
    fn encodes_the_sap_application_http_contract() {
        let mut get = AdtRequest::new(Method::GET, AdtUri::parse("/sap/bc/adt/test/read").unwrap());
        get.push_query("name", "hello world");
        get.headers_mut()
            .insert(header::ACCEPT, HeaderValue::from_static("text/plain"));
        let mut post = AdtRequest::new(
            Method::POST,
            AdtUri::parse("/sap/bc/adt/test/write").unwrap(),
        );
        post.set_content_type("application/xml");
        post.set_body(b"<value/>".to_vec());

        let encoded = [get, post]
            .iter()
            .flat_map(|request| request.format_batch_part("batch_test"))
            .chain(b"--batch_test--".iter().copied())
            .collect::<Vec<_>>();

        assert_eq!(
            encoded,
            b"--batch_test\r\n\
Content-Type: application/http\r\n\
content-transfer-encoding: binary\r\n\r\n\
GET /sap/bc/adt/test/read?name=hello+world HTTP/1.1\r\n\
accept:text/plain\r\n\r\n\
--batch_test\r\n\
Content-Type: application/http\r\n\
content-transfer-encoding: binary\r\n\r\n\
POST /sap/bc/adt/test/write HTTP/1.1\r\n\
content-type:application/xml\r\n\r\n\
<value/>\r\n\
--batch_test--"
        );
    }

    #[tokio::test]
    async fn rejects_an_empty_batch_before_transport() {
        let (client, requests) = fixture_client(Vec::new());
        let batch = client.batch();

        let Err(error) = batch.execute().await else {
            panic!("empty batch succeeded");
        };

        assert!(matches!(
            error,
            OperationError::Encode(EncodeError::Batch(BatchError::Empty))
        ));
        assert!(requests.lock().unwrap().is_empty());
    }

    #[test]
    fn rejects_an_operation_bound_to_a_different_user_session_during_push() {
        let (client, _) = fixture_client(Vec::new());
        let session = client.create_user_session();
        let mut batch = session.batch();

        let Err(error) = batch.push(SessionBoundOperation(UserSessionId::generate())) else {
            panic!("operation bound to a different user session was accepted");
        };

        assert!(matches!(
            error,
            OperationError::Resolve(ResolveError::UserSessionMismatch)
        ));
    }

    #[tokio::test]
    async fn keeps_subrequest_failures_in_their_typed_slots() {
        let (client, _) = fixture_client(vec![fixture_response(&[
            ("missing", StatusCode::NOT_FOUND),
            ("7", StatusCode::OK),
        ])]);
        let mut batch = client.batch();
        let failed = batch.push(TextOperation).unwrap();
        let successful = batch.push(CountOperation).unwrap();

        let mut responses = batch.execute().await.unwrap();

        assert!(matches!(
            responses.take(failed),
            Err(BatchError::Decode { index: 0, .. })
        ));
        assert_eq!(responses.take(successful).unwrap(), 7);
    }

    #[tokio::test]
    async fn rejects_wrong_response_count() {
        let (client, _) = fixture_client(vec![fixture_response(&[("hello", StatusCode::OK)])]);
        let mut batch = client.batch();
        batch.push(TextOperation).unwrap();
        batch.push(CountOperation).unwrap();

        let Err(error) = batch.execute().await else {
            panic!("batch with a missing response part succeeded");
        };

        assert!(matches!(
            error,
            OperationError::Response(ResponseError::Batch(BatchError::ResponseCount {
                expected: 2,
                actual: 1,
            }))
        ));
    }

    #[tokio::test]
    async fn rejects_response_keys_from_another_batch() {
        let (client, _) = fixture_client(vec![fixture_response(&[("hello", StatusCode::OK)])]);
        let mut batch = client.batch();
        let text = batch.push(TextOperation).unwrap();
        let mut other = client.batch();
        let foreign = other.push(TextOperation).unwrap();
        let mut responses = batch.execute().await.unwrap();

        assert!(matches!(
            responses.take(foreign),
            Err(BatchError::ForeignBatch)
        ));
        assert_eq!(responses.take(text).unwrap(), "hello");
    }

    #[tokio::test]
    async fn stateful_batch_uses_outer_user_session() {
        let mut first_headers = HeaderMap::new();
        first_headers.insert(
            header::SET_COOKIE,
            HeaderValue::from_static("sap-contextid=batch-context; Path=/sap/bc/adt"),
        );
        let first_response =
            fixture_response_with_headers(&[("first", StatusCode::OK)], first_headers);
        let second_response = fixture_response(&[("second", StatusCode::OK)]);
        let (client, requests) = fixture_client(vec![first_response, second_response]);
        let session = client.create_user_session();
        let mut batch = session.batch();
        let first = batch.push(StatefulTextOperation).unwrap();

        let mut responses = batch.execute().await.unwrap();
        assert_eq!(responses.take(first).unwrap(), "first");
        batch.execute().await.unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(
            requests[0].headers().get(ADT_SESSION_TYPE_HEADER).unwrap(),
            AdtSessionType::Stateful.as_str()
        );
        assert!(!requests[0].headers().contains_key(header::COOKIE));
        assert_eq!(
            requests[1].headers().get(header::COOKIE).unwrap(),
            "sap-contextid=batch-context"
        );
        assert!(!String::from_utf8_lossy(requests[1].body()).contains("sap-contextid"));
    }

    #[tokio::test]
    async fn executes_homogeneous_operations_through_a_user_session() {
        let (client, requests) = fixture_client(vec![fixture_response(&[
            ("first", StatusCode::OK),
            ("second", StatusCode::OK),
        ])]);
        let session = client.create_user_session();

        let responses = vec![StatefulTextOperation, StatefulTextOperation]
            .batched(&session)
            .await
            .unwrap();

        assert_eq!(responses, ["first", "second"]);
        assert_eq!(
            requests.lock().unwrap()[0]
                .headers()
                .get(ADT_SESSION_TYPE_HEADER)
                .unwrap(),
            AdtSessionType::Stateful.as_str()
        );
    }

    #[test]
    fn parses_quoted_boundary_and_preserves_body_line_ending() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("Multipart/Mixed; charset=utf-8; BOUNDARY=\"batch_test\""),
        );
        let boundary = response_boundary(&headers).unwrap();
        let body = b"--batch_test\r\nContent-Type: application/http\r\n\r\nHTTP/1.1 200 OK\r\nX-Test: one\r\nX-Test: two\r\n\r\nline\r\n\r\n--batch_test--\r\n";

        let responses = decode_batch(body, &boundary).unwrap();

        assert_eq!(responses[0].body(), b"line\r\n");
        assert_eq!(responses[0].headers().get_all("x-test").iter().count(), 2);
    }

    #[test]
    fn rejects_a_multipart_response_without_a_boundary() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/mixed"),
        );

        assert!(matches!(
            response_boundary(&headers),
            Err(BatchError::MissingBoundary { .. })
        ));
    }
}
