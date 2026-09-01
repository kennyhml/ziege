use std::marker::PhantomData;

use http::{HeaderMap, HeaderValue, Method, header};

use super::target::AdvertisedTarget;
use crate::{AdtUri, EntityTag, user_session::UserSessionId};

mod private {
    pub trait Sealed {}
}

/// Marker type for a valid target of an [`crate::Operation`].
pub trait OperationTarget: private::Sealed + Send + Sync + 'static {}

/// An operation target already owned by the operation.
#[derive(Debug)]
pub struct Owned;

/// An operation target that must be resolved from ADT discovery.
#[derive(Debug)]
pub struct Advertised;

impl private::Sealed for Owned {}
impl private::Sealed for Advertised {}
impl OperationTarget for Owned {}
impl OperationTarget for Advertised {}

/// An encoded request target. This split keeps encoding operation data
/// separate from resolving them against advertised resources, which
/// would couple it to access to a disovery document.
pub(crate) enum EncodedTarget {
    /// A static URI hard-coded into the library such as `/sap/bc/adt/discovery`
    Owned(AdtUri),
    /// A target advertised through the discovery or the core discovery
    Advertised(AdvertisedTarget),
}

/// A request encoded from the internal operation into the HTTP envelope.
///
/// The remaining step is to resolve the contained [`EncodedTarget`] to
/// obtain the request target - which may include query parameters of
/// its own.
pub(crate) struct EncodedRequest {
    pub(crate) method: Method,
    pub(crate) target: EncodedTarget,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

/// Wraps the encoded request with local operation metadata, session affinity
/// and a target typestate to enable compile time target resolver guarantees
/// such that an operation with `T = Advertised` can only be resolved by a
/// [`crate::Client`] in the [`crate::Ready`] state with knowledge of a discovery
/// document.
///
/// [`crate::Operation::encode`] interfaces with this type and returns it on success.
///
/// TODO: This adds some friction because the internal enum and the typestate
/// do not guarantee to stay in sync when an operation is defined incorrectly.
pub struct EncodedOperation<T: OperationTarget> {
    pub(crate) request: EncodedRequest,
    bound_user_session: Option<UserSessionId>,
    target: PhantomData<fn() -> T>,
}

impl EncodedOperation<Owned> {
    /// Creates an operation for an already known ADT resource URI.
    pub fn owned(method: Method, target: AdtUri) -> Self {
        Self::new(method, EncodedTarget::Owned(target))
    }

    /// Returns the concrete target owned by this operation.
    pub fn target(&self) -> &AdtUri {
        let EncodedTarget::Owned(target) = &self.request.target else {
            unreachable!("an owned encoded operation must contain an owned target");
        };
        target
    }
}

impl EncodedOperation<Advertised> {
    /// Creates an operation whose target will be resolved from discovery.
    pub fn advertised(method: Method, target: impl Into<AdvertisedTarget>) -> Self {
        Self::new(method, EncodedTarget::Advertised(target.into()))
    }
}

impl<T: OperationTarget> EncodedOperation<T> {
    fn new(method: Method, target: EncodedTarget) -> Self {
        Self {
            request: EncodedRequest {
                method,
                target,
                query: Vec::new(),
                headers: HeaderMap::new(),
                body: Vec::new(),
            },
            bound_user_session: None,
            target: PhantomData,
        }
    }

    pub(crate) fn into_parts(self) -> (EncodedRequest, Option<UserSessionId>) {
        (self.request, self.bound_user_session)
    }

    pub fn push_query(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.request.query.push((name.into(), value.into()));
    }

    pub fn method(&self) -> &Method {
        &self.request.method
    }

    pub fn query(&self) -> &[(String, String)] {
        &self.request.query
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.request.headers
    }

    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.request.headers
    }

    pub fn set_accept(&mut self, media_type: &'static str) {
        self.request
            .headers
            .insert(header::ACCEPT, HeaderValue::from_static(media_type));
    }

    pub fn set_accepts(&mut self, media_types: &[&str]) {
        let value = HeaderValue::from_str(&media_types.join(", "))
            .expect("supported media types form a valid non-empty Accept header");
        self.request.headers.insert(header::ACCEPT, value);
    }

    pub fn set_content_type(&mut self, media_type: &'static str) {
        self.request
            .headers
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
    }

    pub fn set_cache_revalidation(&mut self, if_none_match: Option<&EntityTag>) {
        if let Some(etag) = if_none_match {
            self.request.headers.remove(header::CACHE_CONTROL);
            self.request
                .headers
                .insert(header::IF_NONE_MATCH, etag.as_header_value().clone());
        } else {
            self.request.headers.remove(header::IF_NONE_MATCH);
            self.request
                .headers
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        }
    }

    pub fn body(&self) -> &[u8] {
        &self.request.body
    }

    pub fn set_body(&mut self, body: impl Into<Vec<u8>>) {
        self.request.body = body.into();
    }

    pub(crate) fn bind_user_session(&mut self, user_session: UserSessionId) {
        self.bound_user_session = Some(user_session);
    }
}
