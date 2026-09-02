//! Optimistic and pessimistic concurrency-control decorators.

use http::{StatusCode, header};

use super::{Operation, OperationResponse};
use crate::{
    EncodeError, EncodedOperation, EntityTag, ObjectError, ObjectLock, ObjectRef, ResponseError,
    Stateful, Stateless, api::locking,
};

/// The outcome of a request using a cache validator such as `If-None-Match`.
#[derive(Clone, Debug)]
pub enum ConditionalResult<T> {
    /// The resource changed and a new representation was returned.
    Modified(T),

    /// The supplied validator still identifies the current representation.
    NotModified { etag: Option<EntityTag> },
}

/// An operation carrying an `If-None-Match` cache validator.
#[derive(Debug)]
pub struct IfNoneMatch<O> {
    inner: O,
    etag: EntityTag,
}

impl<O> IfNoneMatch<O> {
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
    type Target = O::Target;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let mut request = self.inner.encode()?;
        request.set_cache_revalidation(Some(&self.etag));
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(ConditionalResult::NotModified {
                etag: response.entity_tag(),
            });
        }

        self.inner.decode(response).map(ConditionalResult::Modified)
    }
}

impl<T> ConditionalResult<T> {
    /// Borrows the returned representation when the resource was modified.
    pub fn as_modified(&self) -> Option<&T> {
        match self {
            Self::Modified(value) => Some(value),
            Self::NotModified { .. } => None,
        }
    }

    /// Consumes the outcome and returns the representation when modified.
    pub fn into_modified(self) -> Option<T> {
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

/// Outcome of a request using optimistic concurrency control with `If-Match`.
#[derive(Clone, Debug)]
pub enum PreconditionResult<T> {
    /// The precondition succeeded and the operation response was returned.
    Success(T),

    /// The entity tag no longer identifies the current representation.
    Failed { etag: Option<EntityTag> },
}

/// An operation carrying an `If-Match` precondition.
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
    type Target = O::Target;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let mut request = self.inner.encode()?;
        request
            .headers_mut()
            .insert(header::IF_MATCH, self.etag.as_header_value().clone());
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() == StatusCode::PRECONDITION_FAILED {
            return Ok(PreconditionResult::Failed {
                etag: response.entity_tag(),
            });
        }
        self.inner.decode(response).map(PreconditionResult::Success)
    }
}

/// Pessimistic concurrency control decorator via object locks.
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
    type Target = O::Target;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let mut request = self.inner.encode()?;
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
