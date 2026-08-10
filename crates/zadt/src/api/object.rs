use http::{Method, StatusCode};

use crate::{
    client::{Client, ClientState},
    error::{ObjectError, OperationError, ResponseError},
    models::{AccessMode, LockHandle, SourceCode, SourceUpdateResult},
    objects::{ObjectRef, ObjectType, Source},
    operation::{Operation, OperationResponse, Stateful, Stateless},
    protocol::{AdtRequest, AdtResponse},
    resource::SourceRef,
    vocabulary::{PostAction, media_type, query_parameter},
};

/// Fetches the source code advertised by a [`SourceRef`].
#[derive(Debug)]
pub struct ObjectSourceQuery {
    /// The source resource to fetch.
    pub source: SourceRef,
}

impl<S: ClientState> Operation<S> for ObjectSourceQuery {
    type Response = SourceCode;
    type Kind = Stateless;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::GET, self.source.uri.clone());
        for (name, value) in &self.source.query {
            request.push_query(name, value);
        }
        request.set_accept(media_type::SOURCE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        expect_ok(response.response())?;
        let etag = response.entity_tag();
        let content = String::from_utf8(response.into_body())
            .map_err(ObjectError::InvalidResponseEncoding)?;
        Ok(SourceCode::new(self.source.clone(), content, etag))
    }
}

impl AccessMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Show => "SHOW",
            Self::Modify => "MODIFY",
        }
    }
}

// Every statically identified object supports locking.
impl<T: ObjectType> ObjectRef<T> {
    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        LockRequest::new(self.erase(), access_mode)
    }

    /// Creates an operation that releases this object's lock.
    pub fn unlock(&self, lock_handle: LockHandle) -> Result<UnlockRequest, ObjectError> {
        if self.uri() != lock_handle.object().uri() {
            return Err(ObjectError::LockHandleObjectMismatch {
                expected: self.to_string(),
                actual: lock_handle.object().to_string(),
            });
        }
        Ok(UnlockRequest::new(lock_handle))
    }
}

impl<T: Source> ObjectRef<T> {
    /// Returns the objects conventional source resource.
    pub fn source(&self) -> SourceRef {
        let component = T::SOURCE_COMPONENTS
            .iter()
            .copied()
            .find(|component| component.is_primary())
            .expect("Source object types must advertise one primary source component");
        self.source_from_component(component)
    }
}

/// Locks a repository object within a [`crate::UserSession`].
///
/// The operation sends `POST` with `_action=LOCK` and the configured
/// `accessMode`. The returned [`LockHandle`] must remain in the same user
/// session as subsequent update or unlock operations.
#[derive(Debug)]
pub struct LockRequest {
    /// The repository object to lock.
    pub object: ObjectRef,

    /// Whether the object is locked for display or modification.
    pub access_mode: AccessMode,
}

impl LockRequest {
    pub(crate) fn new(object: ObjectRef, access_mode: AccessMode) -> Self {
        Self {
            object,
            access_mode,
        }
    }
}

impl<S: ClientState> Operation<S> for LockRequest {
    type Response = LockHandle;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::POST, self.object.uri().clone());
        request.push_query(query_parameter::ACTION, PostAction::Lock.as_str());
        request.push_query(query_parameter::ACCESS_MODE, self.access_mode.as_str());
        request.set_accept(media_type::LOCK_RESULT);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        expect_ok(response.response())?;
        Ok(LockHandle::parse(
            self.object.clone(),
            self.access_mode,
            response.user_session(),
            response.body(),
        )?)
    }
}

/// Releases a [`LockHandle`] within its SAP user session.
#[derive(Debug)]
pub struct UnlockRequest {
    /// The lock to release.
    pub lock_handle: LockHandle,
}

impl UnlockRequest {
    pub(crate) fn new(lock_handle: LockHandle) -> Self {
        Self { lock_handle }
    }
}

impl<S: ClientState> Operation<S> for UnlockRequest {
    type Response = ();
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::POST, self.lock_handle.object().uri().clone());
        request.push_query(query_parameter::ACTION, PostAction::Unlock.as_str());
        request.push_query(query_parameter::LOCK_HANDLE, self.lock_handle.handle());
        if let Some(user_session) = self.lock_handle.user_session() {
            request.require_user_session(user_session);
        }
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        expect_ok(response.response())
    }
}

/// Replaces the complete source code of an object.
///
/// This operation is stateful and requires a [`LockHandle`] issued for the
/// object being updated. [`SourceRef::update`] verifies this relationship before
/// constructing the operation.
#[derive(Debug)]
pub struct ObjectSourceUpdate {
    /// The source resource whose complete content will be replaced.
    source: SourceRef,

    /// A modification lock obtained for the source's owning object.
    lock_handle: LockHandle,

    /// The complete replacement source text.
    content: String,

    /// The transport request selected for this update, when recording is required.
    transport_request: Option<String>,
}

impl ObjectSourceUpdate {
    fn new(
        source: SourceRef,
        lock_handle: LockHandle,
        content: String,
    ) -> Result<Self, ObjectError> {
        if &source.object != lock_handle.object() {
            return Err(ObjectError::LockHandleObjectMismatch {
                expected: source.object.to_string(),
                actual: lock_handle.object().to_string(),
            });
        }
        if lock_handle.access_mode() != AccessMode::Modify {
            return Err(ObjectError::LockHandleNotModifiable);
        }
        Ok(Self {
            source,
            lock_handle,
            content,
            transport_request: None,
        })
    }

    /// Returns the source resource that will be replaced.
    pub fn source(&self) -> &SourceRef {
        &self.source
    }

    /// Returns the lock authorizing this update.
    pub fn lock_handle(&self) -> &LockHandle {
        &self.lock_handle
    }

    /// Returns the complete replacement source text.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the transport request selected for this update.
    pub fn transport_request(&self) -> Option<&str> {
        self.transport_request.as_deref()
    }

    /// Records this update in the supplied transport request.
    pub fn transport(mut self, transport: impl AsRef<str>) -> Self {
        self.transport_request = Some(transport.as_ref().to_owned());
        self
    }
}

impl<S: ClientState> Operation<S> for ObjectSourceUpdate {
    type Response = SourceUpdateResult;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::PUT, self.source.uri.clone());
        request.push_query(query_parameter::LOCK_HANDLE, self.lock_handle.handle());
        if let Some(transport_request) = &self.transport_request {
            request.push_query(query_parameter::TRANSPORT_REQUEST, transport_request);
        }
        if let Some(user_session) = self.lock_handle.user_session() {
            request.require_user_session(user_session);
        }
        request.set_content_type(media_type::SOURCE_UPDATE);
        request.set_body(self.content.clone());
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if !response.status().is_success() {
            return Err(ResponseError::UnexpectedStatus {
                status: response.status(),
                body: String::from_utf8_lossy(response.body()).into_owned(),
            });
        }
        let etag = response.entity_tag();
        let body = response.into_body();
        let content = (!body.is_empty())
            .then(|| String::from_utf8(body))
            .transpose()
            .map_err(ObjectError::InvalidResponseEncoding)?;
        Ok(SourceUpdateResult::new(self.source.clone(), content, etag))
    }
}

impl LockHandle {
    /// Consumes this handle and creates an operation that removes the lock.
    pub fn remove(self) -> UnlockRequest {
        UnlockRequest::new(self)
    }
}

impl SourceRef {
    /// Creates a stateless query for this source representation.
    pub fn query(&self) -> ObjectSourceQuery {
        ObjectSourceQuery {
            source: self.clone(),
        }
    }

    /// Replaces this source using a modification lock for its owning object.
    pub fn update(
        &self,
        lock_handle: &LockHandle,
        content: impl Into<String>,
    ) -> Result<ObjectSourceUpdate, ObjectError> {
        ObjectSourceUpdate::new(self.clone(), lock_handle.clone(), content.into())
    }
}

fn expect_ok(response: &AdtResponse) -> Result<(), ResponseError> {
    if response.status() == StatusCode::OK {
        Ok(())
    } else {
        Err(ResponseError::UnexpectedStatus {
            status: response.status(),
            body: String::from_utf8_lossy(response.body()).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, header};

    use crate::{Class, ClassSourceComponent, Initial, ObjectRef, Program, Transport};

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn program() -> ObjectRef<Program> {
        ObjectRef::<Program>::for_test(
            "ZPROGRAM",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/zprogram").unwrap(),
        )
    }

    #[test]
    fn object_operations_do_not_require_discovery() {
        fn accepts_initial<O: Operation<crate::Initial>>() {}

        accepts_initial::<ObjectSourceQuery>();
        accepts_initial::<LockRequest>();
        accepts_initial::<UnlockRequest>();
        accepts_initial::<ObjectSourceUpdate>();
    }

    #[test]
    fn derives_the_conventional_source_from_a_program_reference() {
        let program = program();

        assert_eq!(
            program.source().uri.as_str(),
            "/sap/bc/adt/programs/programs/zprogram/source/main"
        );
    }

    #[test]
    fn derives_class_source_component_resources() {
        let class = ObjectRef::<Class>::for_test(
            "ZCL_EXAMPLE",
            crate::AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );

        for (component, suffix) in [
            (ClassSourceComponent::Main, "source/main"),
            (ClassSourceComponent::Definitions, "includes/definitions"),
            (
                ClassSourceComponent::Implementations,
                "includes/implementations",
            ),
            (ClassSourceComponent::Macros, "includes/macros"),
            (ClassSourceComponent::TestClasses, "includes/testclasses"),
            (ClassSourceComponent::LocalTypes, "includes/localtypes"),
        ] {
            let source = class.component_source(component);
            assert_eq!(
                source.uri.as_str(),
                format!("/sap/bc/adt/oo/classes/zcl_example/{suffix}")
            );
            assert_eq!(source.object, class.erase());
        }
    }

    #[test]
    fn one_class_lock_can_update_multiple_source_components() {
        let class = ObjectRef::<Class>::for_test(
            "ZCL_EXAMPLE",
            crate::AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );
        let lock_handle = LockHandle::for_test(class.erase(), AccessMode::Modify);
        let client = Client::new(UnusedTransport);

        for component in [
            ClassSourceComponent::Definitions,
            ClassSourceComponent::Implementations,
        ] {
            let source = class.component_source(component);
            let update = source.update(&lock_handle, "source").unwrap();
            let request =
                <ObjectSourceUpdate as Operation<Initial>>::request(&update, &client).unwrap();

            assert_eq!(request.target(), &source.uri);
            assert_eq!(
                request.query(),
                [("lockHandle".to_owned(), "LOCK-HANDLE".to_owned())]
            );
        }
    }

    #[test]
    fn source_update_rejects_a_lock_for_another_object() {
        let first = ObjectRef::<Program>::for_test(
            "ZFIRST",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/ZFIRST").unwrap(),
        );
        let second = ObjectRef::<Program>::for_test(
            "ZSECOND",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/ZSECOND").unwrap(),
        );
        let lock_handle = LockHandle::for_test(first.erase(), AccessMode::Modify);

        let error = second
            .source()
            .update(&lock_handle, "REPORT zsecond.")
            .unwrap_err();

        assert!(matches!(
            error,
            ObjectError::LockHandleObjectMismatch { .. }
        ));
    }

    #[test]
    fn source_update_requires_a_modification_lock() {
        let program = program();
        let lock_handle = LockHandle::for_test(program.erase(), AccessMode::Show);

        let error = program
            .source()
            .update(&lock_handle, "REPORT zprogram.")
            .unwrap_err();

        assert!(matches!(error, ObjectError::LockHandleNotModifiable));
    }

    #[test]
    fn source_update_uses_only_lock_and_transport_write_parameters() {
        let program = program();
        let mut source = program.source();
        source
            .query
            .push(("version".to_owned(), "inactive".to_owned()));
        let lock_handle = LockHandle::for_test(program.erase(), AccessMode::Modify);
        let update = source
            .update(&lock_handle, "REPORT zprogram.")
            .unwrap()
            .transport("A4HK900001");

        let request = <ObjectSourceUpdate as Operation<Initial>>::request(
            &update,
            &Client::new(UnusedTransport),
        )
        .unwrap();

        assert_eq!(request.method(), Method::PUT);
        assert_eq!(request.target(), &source.uri);
        assert_eq!(
            request.query(),
            [
                ("lockHandle".to_owned(), "LOCK-HANDLE".to_owned()),
                ("corrNr".to_owned(), "A4HK900001".to_owned()),
            ]
        );
        assert_eq!(request.body(), b"REPORT zprogram.");
    }

    #[test]
    fn source_update_decodes_canonical_content_and_etag() {
        let program = program();
        let source = program.source();
        let lock_handle = LockHandle::for_test(program.erase(), AccessMode::Modify);
        let update = source.update(&lock_handle, "REPORT zprogram.").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::ETAG, HeaderValue::from_static("source-etag-2"));

        let result = <ObjectSourceUpdate as Operation<Initial>>::decode(
            &update,
            OperationResponse::new(
                AdtResponse::new(StatusCode::OK, headers, b"REPORT zprogram.\n".to_vec()),
                source.uri.clone(),
            ),
        )
        .unwrap();

        assert_eq!(result.reference, source);
        assert_eq!(result.content.as_deref(), Some("REPORT zprogram.\n"));
        assert_eq!(result.etag.as_deref(), Some("source-etag-2"));
    }

    #[test]
    fn source_update_accepts_an_empty_success_response() {
        let program = program();
        let source = program.source();
        let lock_handle = LockHandle::for_test(program.erase(), AccessMode::Modify);
        let update = source.update(&lock_handle, "REPORT zprogram.").unwrap();

        let result = <ObjectSourceUpdate as Operation<Initial>>::decode(
            &update,
            OperationResponse::new(
                AdtResponse::new(StatusCode::NO_CONTENT, HeaderMap::new(), Vec::new()),
                source.uri,
            ),
        )
        .unwrap();

        assert_eq!(result.content, None);
        assert_eq!(result.etag, None);
    }

    #[tokio::test]
    async fn source_update_rejects_another_user_session_before_transport() {
        let program = program();
        let lock_handle = LockHandle::for_test(program.erase(), AccessMode::Modify);
        let update = program
            .source()
            .update(&lock_handle, "REPORT zprogram.")
            .unwrap();
        let session = Client::new(UnusedTransport).create_user_session();

        let error = update.execute(&session).await.unwrap_err();

        assert!(matches!(error, OperationError::UserSessionMismatch));
    }

    #[test]
    fn object_rejects_another_objects_lock_for_unlock() {
        let first = ObjectRef::<Program>::for_test(
            "ZFIRST",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/ZFIRST").unwrap(),
        );
        let second = ObjectRef::<Program>::for_test(
            "ZSECOND",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/ZSECOND").unwrap(),
        );
        let lock_handle = LockHandle::for_test(first.erase(), AccessMode::Modify);

        let error = second.unlock(lock_handle).unwrap_err();

        assert!(matches!(
            error,
            ObjectError::LockHandleObjectMismatch { .. }
        ));
    }
}
