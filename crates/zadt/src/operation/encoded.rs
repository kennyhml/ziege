use http::{HeaderMap, HeaderValue, Method, header};

use super::OperationContext;
use crate::{AdtRequest, AdtUri, EntityTag, user_session::UserSessionId};

/// A transport-ready ADT request encoded by an [`crate::Operation`].
///
/// Resolution requirements are enforced before this value is constructed, so
/// its request always contains a concrete target URI. Session affinity remains
/// attached until an executor validates and sends the request.
pub struct EncodedOperation {
    request: AdtRequest,
    bound_user_session: Option<UserSessionId>,
}

impl EncodedOperation {
    fn from_request(request: AdtRequest) -> Self {
        Self {
            request,
            bound_user_session: None,
        }
    }

    /// Creates an encoded operation for a concrete ADT resource URI.
    pub fn new(method: Method, target: AdtUri) -> Self {
        Self::from_request(AdtRequest::new(method, target))
    }

    /// Returns the concrete request target.
    pub fn target(&self) -> &AdtUri {
        self.request.target()
    }

    /// Separates the request, decoder context, and bound user-session identity.
    pub fn into_parts(self) -> (AdtRequest, OperationContext, Option<UserSessionId>) {
        let context = OperationContext::new(self.request.target().clone());
        (self.request, context, self.bound_user_session)
    }

    pub(crate) fn context(&self) -> OperationContext {
        OperationContext::new(self.request.target().clone())
    }

    pub fn push_query(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.request.push_query(name, value);
    }

    pub fn method(&self) -> &Method {
        self.request.method()
    }

    pub fn query(&self) -> &[(String, String)] {
        self.request.query()
    }

    pub fn headers(&self) -> &HeaderMap {
        self.request.headers()
    }

    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.request.headers_mut()
    }

    pub fn set_accept(&mut self, media_type: &'static str) {
        self.request
            .headers_mut()
            .insert(header::ACCEPT, HeaderValue::from_static(media_type));
    }

    pub fn set_accepts(&mut self, media_types: &[&str]) {
        let value = HeaderValue::from_str(&media_types.join(", "))
            .expect("supported media types form a valid non-empty Accept header");
        self.request.headers_mut().insert(header::ACCEPT, value);
    }

    pub fn set_content_type(&mut self, media_type: &'static str) {
        self.request
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
    }

    pub fn set_cache_revalidation(&mut self, if_none_match: Option<&EntityTag>) {
        if let Some(etag) = if_none_match {
            self.request.headers_mut().remove(header::CACHE_CONTROL);
            self.request
                .headers_mut()
                .insert(header::IF_NONE_MATCH, etag.as_header_value().clone());
        } else {
            self.request.headers_mut().remove(header::IF_NONE_MATCH);
            self.request
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        }
    }

    pub fn body(&self) -> &[u8] {
        self.request.body()
    }

    pub fn set_body(&mut self, body: impl Into<Vec<u8>>) {
        self.request.set_body(body);
    }

    pub(crate) fn bind_user_session(&mut self, user_session: UserSessionId) {
        self.bound_user_session = Some(user_session);
    }

    pub(crate) fn bound_user_session(&self) -> Option<UserSessionId> {
        self.bound_user_session
    }
}

impl From<AdtRequest> for EncodedOperation {
    fn from(request: AdtRequest) -> Self {
        Self::from_request(request)
    }
}
