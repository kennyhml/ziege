//! Optimistic and pessimistic concurrency-control decorators.
use super::{Operation, OperationResponse, ResolutionRequirement};
use crate::{
    EncodeError, EncodedOperation, EntityTag, ObjectError, ObjectLock, ObjectRef, ResponseError,
    Stateful, Stateless, api::locking,
};
use http::{StatusCode, header};

/// The outcome of a request using a cache validator such as `If-None-Match`.
///
/// If the E-Tag we sent along it not the current version of the resource on
/// the server, the new version is contained in [`Self::Modified`]. Otherwise,
/// the version we have it still up-to-date and an optional server E-Tag is
/// returned for validation in [`Self::NotModified`].
///
/// See the [MDN documentation for `If-None-Match`][mdn-if-none-match].
///
/// [mdn-if-none-match]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/If-None-Match
#[derive(Clone, Debug)]
pub enum ConditionalResult<T> {
    /// The resource changed and a new representation was returned.
    Modified(T),
    /// The supplied validator still identifies the current representation.
    NotModified { etag: Option<EntityTag> },
}

impl<T> ConditionalResult<T> {
    /// Borrows the returned representation when the resource was modified.
    pub fn as_modified(&self) -> Option<&T> {
        match self {
            Self::Modified(value) => Some(value),
            Self::NotModified { .. } => None,
        }
    }

    /// Returns the ETag supplied with a not-modified response.
    pub fn not_modified_etag(&self) -> Option<&str> {
        match self {
            Self::Modified(_) => None,
            Self::NotModified { etag } => etag.as_deref(),
        }
    }
}

/// An operation carrying an `If-None-Match` cache validator.
///
/// The operation response is wrapped in a [`ConditionalResult`] and
/// decoded locally. The decorator is lightweight and only adds the
/// E-Tag header and response status code inspection.
///
/// See the result documentation for more detail.
#[derive(Debug)]
pub struct IfNoneMatch<O> {
    inner: O,
    etag: EntityTag,
}

impl<O> IfNoneMatch<O> {
    /// Internal constructor to construct a conditional query.
    /// Crate users currently should not interface with this method.
    pub(crate) fn new(inner: O, etag: EntityTag) -> Self {
        Self { inner, etag }
    }
}

impl<O> Operation for IfNoneMatch<O>
where
    O: Operation,
{
    type Response = ConditionalResult<O::Response>;
    type Kind = O::Kind;
    type ResolutionRequirement = O::ResolutionRequirement;

    fn encode(
        &self,
        resolver: &<Self::ResolutionRequirement as ResolutionRequirement>::Resolver,
    ) -> Result<EncodedOperation, EncodeError> {
        let mut request = self.inner.encode(resolver)?;
        request.set_cache_revalidation(Some(&self.etag));
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(ConditionalResult::NotModified {
                etag: response.etag(),
            });
        }
        self.inner.decode(response).map(ConditionalResult::Modified)
    }
}

/// The outcome of a request using optimistic concurrency control with `If-Match`.
///
/// If the E-Tag we sent along it not the current version of the resource on
/// the server, the operation is rejected by the server, typically because it
/// attempts to mutate a resource that has since been mutated by another client.
///
/// In that case, the servers current version is returned in [`Self::Failed`] for
/// validation and error detail, this E-Tag should not be used to bypass the
/// cache-control! If the E-Tag is still up-to-date, the response passes through
/// the [`Self::Success`] variant.
///
/// See the [MDN documentation for `If-Match`][mdn-if-match].
///
/// [mdn-if-match]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/If-Match
#[derive(Clone, Debug)]
pub enum PreconditionResult<T> {
    /// The precondition succeeded and the operation response was returned.
    Success(T),

    /// The entity tag no longer identifies the current representation.
    Failed { etag: Option<EntityTag> },
}

/// An operation carrying an `If-Match` precondition.
///
/// The operation response is wrapped in a [`PreconditionResult`] and
/// decoded locally. The decorator is lightweight and only adds the
/// E-Tag header and response status code inspection.
///
/// See the result documentation for more detail.
#[derive(Debug)]
pub struct IfMatch<O> {
    inner: O,
    etag: EntityTag,
}

impl<O> IfMatch<O> {
    pub(crate) fn new(inner: O, etag: EntityTag) -> Self {
        Self { inner, etag }
    }

    pub(crate) fn map_inner(mut self, map: impl FnOnce(&mut O)) -> Self {
        map(&mut self.inner);
        self
    }
}

impl<O> Operation for IfMatch<O>
where
    O: Operation,
{
    type Kind = O::Kind;
    type Response = PreconditionResult<O::Response>;
    type ResolutionRequirement = O::ResolutionRequirement;

    fn encode(
        &self,
        resolver: &<Self::ResolutionRequirement as ResolutionRequirement>::Resolver,
    ) -> Result<EncodedOperation, EncodeError> {
        let mut request = self.inner.encode(resolver)?;
        request
            .headers_mut()
            .insert(header::IF_MATCH, self.etag.as_header_value().clone());
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() == StatusCode::PRECONDITION_FAILED {
            return Ok(PreconditionResult::Failed {
                etag: response.etag(),
            });
        }
        self.inner.decode(response).map(PreconditionResult::Success)
    }
}

/// Pessimistic concurrency control decorator via an object lock.
///
/// This turns the operation into a [`Stateful`] operation unconditionally,
/// because object locks are only valid in the [`crate::UserSession`] they were
/// created in.
///
/// The decorator validates that the passed lock is valid for modifications
/// on the passed reference during construction. This is merely an internal
/// helper so that validation cannot be omitted. It is still possible to validate
/// the wrong object against the real operation target if the call site is
/// not cautious.
///
/// See [`IfMatch`] as alternative for optimistic concurrency control.
#[derive(Debug)]
pub struct Locked<O> {
    inner: O,
    lock: ObjectLock,
}

impl<O> Locked<O> {
    pub(crate) fn try_new<T>(
        inner: O,
        lock: ObjectLock,
        target: &ObjectRef<T>,
    ) -> Result<Self, ObjectError> {
        lock.validate_modification_for(target)?;

        Ok(Self { inner, lock })
    }

    pub(crate) fn map_inner(mut self, map: impl FnOnce(&mut O)) -> Self {
        map(&mut self.inner);
        self
    }
}

impl<O> Operation for Locked<O>
where
    O: Operation<Kind = Stateless>,
{
    type Kind = Stateful;
    type Response = O::Response;
    type ResolutionRequirement = O::ResolutionRequirement;

    fn encode(
        &self,
        resolver: &<Self::ResolutionRequirement as ResolutionRequirement>::Resolver,
    ) -> Result<EncodedOperation, EncodeError> {
        let mut request = self.inner.encode(resolver)?;
        request.push_query(locking::LOCK_HANDLE_QUERY, self.lock.handle());
        if let Some(user_session) = self.lock.user_session() {
            request.bind_user_session(user_session);
        }
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        self.inner.decode(response)
    }
}
