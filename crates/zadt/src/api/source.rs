use http::{Method, StatusCode};

use crate::{
    client::{Client, ClientState},
    error::{ObjectError, OperationError, ResponseError},
    objects::{Erased, HasSource, ObjectRef},
    operation::{Operation, OperationResponse, Stateful, Stateless},
    protocol::{AdtRequest, AdtResponse, EntityTag},
    resource::SourceRef,
    vocabulary::{media_type, query_parameter},
};

use super::{
    locking::{AccessMode, ObjectLock},
    transports::TransportNumber,
};

/// A fetched source representation and its attached metadata.
#[derive(Debug)]
pub struct SourceCode {
    /// The source resource that was fetched.
    pub reference: SourceRef,

    /// The complete UTF-8 source text.
    pub content: String,

    /// The response entity tag supplied by SAP, when present.
    pub etag: Option<EntityTag>,
}

impl SourceCode {
    pub(crate) fn new(reference: SourceRef, content: String, etag: Option<EntityTag>) -> Self {
        Self {
            reference,
            content,
            etag,
        }
    }
}

/// The canonical source information returned by a successful update.
#[derive(Debug)]
pub struct SourceUpdateResult {
    /// The source resource that was updated.
    pub reference: SourceRef,

    /// Server-confirmed source content when SAP returned a representation body.
    pub content: Option<String>,

    /// The updated entity tag supplied by SAP, when present.
    pub etag: Option<EntityTag>,
}

impl SourceUpdateResult {
    pub(crate) fn new(
        reference: SourceRef,
        content: Option<String>,
        etag: Option<EntityTag>,
    ) -> Self {
        Self {
            reference,
            content,
            etag,
        }
    }
}

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

impl ObjectRef<Erased> {
    /// Resolves the primary source when available.
    pub fn source(&self) -> Option<SourceRef> {
        self.descriptor()
            .and_then(|descriptor| descriptor.source_path())
            .map(|path| SourceRef::from_object_path(self.clone(), path))
    }

    /// Resolves one named secondary source component when available.
    pub fn source_component(&self, name: &str) -> Option<SourceRef> {
        self.descriptor()?
            .source_component_paths()
            .iter()
            .find(|path| path.last() == Some(&name))
            .map(|path| SourceRef::from_object_path(self.clone(), path))
    }
}

impl<T: HasSource> ObjectRef<T> {
    /// Returns the objects conventional source resource.
    pub fn source(&self) -> SourceRef {
        SourceRef::from_object_path(self.erase(), T::SOURCE_PATH)
    }
}

/// Replaces the complete source code of an object.
///
/// This operation is stateful and requires an [`ObjectLock`] issued for the
/// object being updated. [`SourceRef::update`] verifies this relationship before
/// constructing the operation.
#[derive(Debug)]
pub struct ObjectSourceUpdate {
    /// The source resource whose complete content will be replaced.
    source: SourceRef,

    /// A modification lock obtained for the source's owning object.
    object_lock: ObjectLock,

    /// The complete replacement source text.
    content: String,

    /// The transport request selected for this update, when recording is required.
    transport_request: Option<TransportNumber>,
}

impl ObjectSourceUpdate {
    /// Records this update in the supplied transport request.
    ///
    /// This replaces any transport request inherited from the lock.
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }
}

impl<S: ClientState> Operation<S> for ObjectSourceUpdate {
    type Response = SourceUpdateResult;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::PUT, self.source.uri.clone());
        request.push_query(query_parameter::LOCK_HANDLE, self.object_lock.handle());
        if let Some(transport_request) = &self.transport_request {
            request.push_query(
                query_parameter::TRANSPORT_REQUEST,
                transport_request.as_str(),
            );
        }
        if let Some(user_session) = self.object_lock.user_session() {
            request.require_user_session(user_session);
        }
        request.set_content_type(media_type::SOURCE_UPDATE);
        request.set_body(self.content.clone());
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if !response.status().is_success() {
            return Err(ResponseError::unexpected_status(response.response()));
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

impl SourceRef {
    /// Creates a stateless query for this source representation.
    pub fn query(&self) -> ObjectSourceQuery {
        ObjectSourceQuery {
            source: self.clone(),
        }
    }

    /// Replaces this source using a modification lock for its owning object.
    ///
    /// The update automatically uses the transport request attached to the lock,
    /// when SAP supplied one.
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        content: impl Into<String>,
    ) -> Result<ObjectSourceUpdate, ObjectError> {
        if &self.object != object_lock.object() {
            return Err(ObjectError::ObjectLockMismatch {
                expected: self.object.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        if object_lock.access_mode() != AccessMode::Modify {
            return Err(ObjectError::ObjectLockNotModifiable);
        }
        Ok(ObjectSourceUpdate {
            source: self.clone(),
            object_lock: object_lock.clone(),
            content: content.into(),
            transport_request: object_lock.transport_request().cloned(),
        })
    }
}

fn expect_ok(response: &AdtResponse) -> Result<(), ResponseError> {
    if response.status() == StatusCode::OK {
        Ok(())
    } else {
        Err(ResponseError::unexpected_status(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, header};

    use crate::{
        AdtResponse, AdtUri, Class, ClassSourceComponent, Initial, ObjectRef, Program, Transport,
    };

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
            AdtUri::parse("/sap/bc/adt/programs/programs/zprogram").unwrap(),
        )
    }

    #[test]
    fn source_operations_do_not_require_discovery() {
        fn accepts_initial<O: Operation<Initial>>() {}

        accepts_initial::<ObjectSourceQuery>();
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
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );

        let main_source = class.source();
        assert_eq!(
            main_source.uri.as_str(),
            "/sap/bc/adt/oo/classes/zcl_example/source/main"
        );
        assert_eq!(main_source.object, class.erase());

        for (component, suffix) in [
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
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );
        let object_lock = ObjectLock::for_test(class.erase(), AccessMode::Modify);
        let client = Client::new(UnusedTransport);

        for component in [
            ClassSourceComponent::Definitions,
            ClassSourceComponent::Implementations,
        ] {
            let source = class.component_source(component);
            let update = source.update(&object_lock, "source").unwrap();
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
            AdtUri::parse("/sap/bc/adt/programs/programs/ZFIRST").unwrap(),
        );
        let second = ObjectRef::<Program>::for_test(
            "ZSECOND",
            AdtUri::parse("/sap/bc/adt/programs/programs/ZSECOND").unwrap(),
        );
        let object_lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);

        let error = second
            .source()
            .update(&object_lock, "REPORT zsecond.")
            .unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockMismatch { .. }));
    }

    #[test]
    fn source_update_requires_a_modification_lock() {
        let program = program();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Show);

        let error = program
            .source()
            .update(&object_lock, "REPORT zprogram.")
            .unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockNotModifiable));
    }

    #[test]
    fn source_update_inherits_the_locks_transport_request() {
        let program = program();
        let mut source = program.source();
        source
            .query
            .push(("version".to_owned(), "inactive".to_owned()));
        let object_lock =
            ObjectLock::for_test_with_transport(program.erase(), AccessMode::Modify, "A4HK900001");
        let update = source.update(&object_lock, "REPORT zprogram.").unwrap();

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
    fn source_update_accepts_an_explicit_transport_request() {
        let program = program();
        let source = program.source();

        for object_lock in [
            ObjectLock::for_test(program.erase(), AccessMode::Modify),
            ObjectLock::for_test_with_transport(program.erase(), AccessMode::Modify, "A4HK900001"),
        ] {
            let update = source
                .update(&object_lock, "REPORT zprogram.")
                .unwrap()
                .transport("A4HK900002");
            let request = <ObjectSourceUpdate as Operation<Initial>>::request(
                &update,
                &Client::new(UnusedTransport),
            )
            .unwrap();

            assert_eq!(
                request.query(),
                [
                    ("lockHandle".to_owned(), "LOCK-HANDLE".to_owned()),
                    ("corrNr".to_owned(), "A4HK900002".to_owned()),
                ]
            );
        }
    }

    #[test]
    fn source_update_decodes_canonical_content_and_etag() {
        let program = program();
        let source = program.source();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = source.update(&object_lock, "REPORT zprogram.").unwrap();
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
    fn source_update_decodes_structured_backend_exceptions() {
        let program = program();
        let source = program.source();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = source.update(&object_lock, "REPORT zprogram.").unwrap();
        let body =
            br#"<exc:exception xmlns:exc="http://www.sap.com/abapxml/types/communicationframework">
            <namespace id="com.sap.adt"/>
            <type id="ExceptionResourceLockConflict"/>
            <message lang="EN">Object is already locked</message>
            <localizedMessage lang="EN">Object is locked in request A4HK900125</localizedMessage>
            <properties>
                <entry key="T100KEY-V3">A4HK900125</entry>
            </properties>
        </exc:exception>"#;

        let error = <ObjectSourceUpdate as Operation<Initial>>::decode(
            &update,
            OperationResponse::new(
                AdtResponse::new(StatusCode::CONFLICT, HeaderMap::new(), body.to_vec()),
                source.uri,
            ),
        )
        .unwrap_err();

        let ResponseError::BackendException { status, exception } = error else {
            panic!("expected a structured backend exception");
        };
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(exception.exception_type, "ExceptionResourceLockConflict");
        assert_eq!(exception.property("T100KEY-V3"), Some("A4HK900125"));
    }

    #[test]
    fn source_update_accepts_an_empty_success_response() {
        let program = program();
        let source = program.source();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = source.update(&object_lock, "REPORT zprogram.").unwrap();

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
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = program
            .source()
            .update(&object_lock, "REPORT zprogram.")
            .unwrap();
        let session = Client::new(UnusedTransport).create_user_session();

        let error = update.execute(&session).await.unwrap_err();

        assert!(matches!(error, OperationError::UserSessionMismatch));
    }
}
