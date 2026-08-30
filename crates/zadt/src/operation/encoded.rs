use std::marker::PhantomData;

use http::{HeaderMap, HeaderValue, Method, header};

use crate::{AdtUri, CategoryId, EntityTag, user_session::UserSessionId};

mod private {
    pub trait Sealed {}
}

/// Identifies how an encoded operation obtains its request target.
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

/// Selects the discovery document containing an advertised resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryDocument {
    Central,
    Core,
}

/// A collection locator resolved from an ADT discovery document during execution.
#[derive(Debug)]
pub struct AdvertisedCollection {
    pub(crate) document: DiscoveryDocument,
    pub(crate) category: CategoryId,
    pub(crate) suffix: Vec<String>,
    pub(crate) accepted_media_types: &'static [&'static str],
}

impl AdvertisedCollection {
    /// Selects a collection from central ADT discovery.
    pub fn new(category: CategoryId) -> Self {
        Self::in_document(DiscoveryDocument::Central, category)
    }

    /// Selects a collection from core ADT discovery.
    pub fn core(category: CategoryId) -> Self {
        Self::in_document(DiscoveryDocument::Core, category)
    }

    fn in_document(document: DiscoveryDocument, category: CategoryId) -> Self {
        Self {
            document,
            category,
            suffix: Vec::new(),
            accepted_media_types: &[],
        }
    }

    /// Appends one safely encoded segment to a collection target.
    pub fn push_segment(&mut self, segment: impl Into<String>) {
        self.suffix.push(segment.into());
    }

    pub(crate) fn require_accepted_media_types(&mut self, media_types: &'static [&'static str]) {
        self.accepted_media_types = media_types;
    }
}

/// A URI-template locator resolved from an ADT discovery document during execution.
#[derive(Debug)]
pub struct AdvertisedTemplate {
    pub(crate) document: DiscoveryDocument,
    pub(crate) category: CategoryId,
    pub(crate) relation: &'static str,
    pub(crate) variables: Vec<(String, String)>,
    pub(crate) required_variables: Vec<&'static str>,
    pub(crate) supported_variables: Vec<&'static str>,
    pub(crate) required_query_parameters: Vec<&'static str>,
}

impl AdvertisedTemplate {
    /// Selects a URI template from a central-discovery collection.
    pub fn new(category: CategoryId, relation: &'static str) -> Self {
        Self {
            document: DiscoveryDocument::Central,
            category,
            relation,
            variables: Vec::new(),
            required_variables: Vec::new(),
            supported_variables: Vec::new(),
            required_query_parameters: Vec::new(),
        }
    }

    /// Supplies one string variable for URI-template expansion.
    pub fn push_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.push((name.into(), value.into()));
    }

    /// Requires the advertised template to declare one variable.
    pub fn require_variable(&mut self, variable: &'static str) {
        self.required_variables.push(variable);
    }

    /// Requires the advertised template to support one optional variable.
    pub fn require_supported_variable(&mut self, variable: &'static str) {
        self.supported_variables.push(variable);
    }

    /// Requires template expansion to produce one query parameter.
    pub fn require_query_parameter(&mut self, parameter: &'static str) {
        self.required_query_parameters.push(parameter);
    }
}

/// An advertised collection or URI-template target.
#[derive(Debug)]
pub enum AdvertisedTarget {
    Collection(AdvertisedCollection),
    Template(AdvertisedTemplate),
}

impl From<AdvertisedCollection> for AdvertisedTarget {
    fn from(target: AdvertisedCollection) -> Self {
        Self::Collection(target)
    }
}

impl From<AdvertisedTemplate> for AdvertisedTarget {
    fn from(target: AdvertisedTemplate) -> Self {
        Self::Template(target)
    }
}

pub(crate) enum EncodedTarget {
    Owned(AdtUri),
    Advertised(AdvertisedTarget),
}

pub(crate) struct EncodedRequest {
    pub(crate) method: Method,
    pub(crate) target: EncodedTarget,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

/// A transport-neutral operation plan whose target has not necessarily been resolved.
///
/// The encoded request contains only protocol data. Local execution metadata,
/// such as user-session affinity, is retained separately by this plan.
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
