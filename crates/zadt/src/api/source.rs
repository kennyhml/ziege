use http::{Method, StatusCode};

use crate::{
    AdtRequest, IfMatch, Locked, ObjectRef, SnapshotResources,
    error::{EncodeError, ObjectError, ResponseError},
    objects::{ObjectSnapshot, Source, SourceComponents},
    operation::{EncodedOperation, Independent, Operation, OperationResponse, Stateless},
    protocol::{EntityTag, TEXT_PLAIN_MEDIA_TYPE},
    resource::SourceRef,
};

use super::{
    locking::ObjectLock,
    transports::{TRANSPORT_REQUEST_QUERY, TransportNumber},
};

/// Fetches the source code advertised by a [`SourceRef`].
///
/// The reference to the source already contains all information
/// needed to make the request, as the media type can be assumed
/// as `text/plain`.
///
/// Source queries support if-none-match handling and return an
/// etag in the response headers.
#[derive(Debug)]
pub struct SourceQuery {
    /// The source resource to fetch.
    pub source: SourceRef,
}

impl Operation for SourceQuery {
    type Response = SourceCode;
    type Kind = Stateless;
    type ResolutionRequirement = Independent;

    fn encode(&self, _: &()) -> Result<EncodedOperation, EncodeError> {
        let mut request = AdtRequest::new(Method::GET, self.source.uri.clone());
        for (name, value) in &self.source.query {
            request.push_query(name, value);
        }
        request.set_accept(TEXT_PLAIN_MEDIA_TYPE);
        Ok(EncodedOperation::from(request))
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[TEXT_PLAIN_MEDIA_TYPE])?;

        let etag = response.etag();
        let content = String::from_utf8(response.into_body())
            .map_err(ObjectError::InvalidResponseEncoding)?;
        Ok(SourceCode::new(self.source.clone(), content, etag))
    }
}

impl<T: Source> ObjectSnapshot<T> {
    /// Resolves the primary source advertised by this loaded object.
    pub fn source(&self) -> Result<SourceRef, ObjectError> {
        map_source_ref(self.reference(), self.resources(), "main")?
            .ok_or(ObjectError::MissingRelation { relation: "source" })
    }
}

impl<T: SourceComponents> ObjectSnapshot<T> {
    /// Resolves one named source component supported by this loaded object family.
    pub fn source_component(
        &self,
        name: impl AsRef<str>,
    ) -> Result<Option<SourceRef>, ObjectError> {
        map_source_ref(self.reference(), self.resources(), name.as_ref())
    }
}

impl ObjectSnapshot<()> {
    /// Resolves the primary source advertised by this runtime-typed object.
    pub fn source(&self) -> Result<SourceRef, ObjectError> {
        if !self.reference().require_descriptor()?.supports_source() {
            return Err(self.reference().unsupported_capability("source"));
        }

        map_source_ref(self.reference(), self.resources(), "main")?
            .ok_or(ObjectError::MissingRelation { relation: "source" })
    }

    /// Resolves one named source component supported by this runtime-typed object.
    pub fn source_component(
        &self,
        name: impl AsRef<str>,
    ) -> Result<Option<SourceRef>, ObjectError> {
        let descriptor = self.reference().require_descriptor()?;
        if !descriptor.supports_source_components() {
            return Err(self.reference().unsupported_capability("source components"));
        }
        map_source_ref(self.reference(), self.resources(), name.as_ref())
    }
}

/// Replaces the complete source code of an object.
///
/// Construct through [`SourceRef::update_if_match`] for optimistic concurrency
/// or [`SourceRef::update_with_lock`] for a stateful, lock-based update.
#[derive(Debug)]
pub struct SourceUpdate {
    /// The source resource whose complete content will be replaced.
    source: SourceRef,

    /// The complete replacement source text.
    content: String,

    /// The transport request selected for this update, when recording is required.
    transport_request: Option<TransportNumber>,
}

impl SourceUpdate {
    const MEDIA_TYPE: &str = "text/plain; charset=utf-8";

    /// Records this update in the supplied transport request.
    ///
    /// This replaces any transport request inherited from the lock.
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }
}

impl Operation for SourceUpdate {
    type Response = SourceUpdateResult;
    type Kind = Stateless;
    type ResolutionRequirement = Independent;

    fn encode(&self, _: &()) -> Result<EncodedOperation, EncodeError> {
        let mut request = AdtRequest::new(Method::PUT, self.source.uri.clone());
        if let Some(transport_request) = &self.transport_request {
            request.push_query(TRANSPORT_REQUEST_QUERY, transport_request.as_str());
        }
        request.set_content_type(Self::MEDIA_TYPE);
        request.set_body(self.content.clone());
        Ok(EncodedOperation::from(request))
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        let etag = response.etag();
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
    pub fn query(&self) -> SourceQuery {
        SourceQuery {
            source: self.clone(),
        }
    }

    /// Creates a stateless update guarded by the entity tag from this reference.
    ///
    /// Construction fails when the reference has no entity tag. A failed HTTP
    /// precondition is represented by [`crate::PreconditionResult::Failed`].
    /// Uses the tag stored on this reference. Query and update response tags
    /// remain separate and do not automatically replace it.
    pub fn update_if_match(
        &self,
        content: impl Into<String>,
    ) -> Result<IfMatch<SourceUpdate>, ObjectError> {
        let etag = self.etag.clone().ok_or(ObjectError::MissingEntityTag)?;
        self.update(content)
            .map(|operation| IfMatch::new(operation, etag))
    }

    /// Creates a stateful update guarded by a persistent modification lock.
    ///
    /// The lock must belong to this object and permit modifications. Its user
    /// session and transport request are retained by the returned operation.
    pub fn update_with_lock(
        &self,
        content: impl Into<String>,
        lock: ObjectLock,
    ) -> Result<Locked<SourceUpdate>, ObjectError> {
        let mut update = self.update(content)?;
        update.transport_request = lock.transport_request().cloned();
        Locked::try_new(update, lock, &self.object)
    }

    /// Constructs the source update shared by both concurrency modes.
    fn update(&self, content: impl Into<String>) -> Result<SourceUpdate, ObjectError> {
        Ok(SourceUpdate {
            source: self.clone(),
            content: content.into(),
            transport_request: None,
        })
    }
}

impl IfMatch<SourceUpdate> {
    /// Records this update in the supplied transport request.
    pub fn transport(self, transport: impl Into<TransportNumber>) -> Self {
        self.map_inner(|update| update.transport_request = Some(transport.into()))
    }
}

impl Locked<SourceUpdate> {
    /// Overrides the transport request inherited from the lock.
    pub fn transport(self, transport: impl Into<TransportNumber>) -> Self {
        self.map_inner(|update| update.transport_request = Some(transport.into()))
    }
}

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

/// Internal helper method to look up the [`crate::SnapshotResource`] associated
/// with the source component and map it into a [`SourceRef`].
fn map_source_ref<T>(
    reference: &ObjectRef<T>,
    resources: SnapshotResources<'_>,
    name: &str,
) -> Result<Option<SourceRef>, ObjectError> {
    resources
        .source(name)
        .map(|resource| {
            SourceRef::from_href(
                reference.erase(),
                resource.href(),
                resource.etag().map(str::to_owned),
            )
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use crate::{
        AccessMode, AdtRequest, AdtResponse, AdtUri, Class, Client, ObjectKey, ObjectRef,
        OperationError, Program, Transport,
    };

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn program() -> ObjectRef<Program> {
        ObjectRef::new(
            ObjectKey::<Program>::new("ZPROGRAM"),
            AdtUri::parse("/sap/bc/adt/programs/programs/zprogram").unwrap(),
        )
    }

    fn source_ref<T>(object: &ObjectRef<T>, uri: &str) -> SourceRef {
        SourceRef::new(object.erase(), AdtUri::parse(uri).unwrap())
    }

    fn program_source() -> SourceRef {
        source_ref(
            &program(),
            "/sap/bc/adt/programs/programs/zprogram/source/main",
        )
    }

    #[test]
    fn source_operations_do_not_require_discovery() {
        fn accepts_stateless<
            O: Operation<Kind = Stateless, ResolutionRequirement = Independent>,
        >() {
        }
        fn accepts_stateful<
            O: Operation<Kind = crate::Stateful, ResolutionRequirement = Independent>,
        >() {
        }

        accepts_stateless::<SourceQuery>();
        accepts_stateless::<IfMatch<SourceUpdate>>();
        accepts_stateful::<Locked<SourceUpdate>>();
    }

    #[test]
    fn optimistic_source_update_requires_a_source_etag() {
        assert!(matches!(
            program_source().update_if_match("REPORT zprogram."),
            Err(ObjectError::MissingEntityTag)
        ));
        assert!(matches!(
            SourceRef::from_href(
                program().erase(),
                "source/main",
                Some("invalid\r\ntag".into())
            ),
            Err(ObjectError::InvalidEntityTag(_))
        ));
    }

    #[test]
    fn optimistic_source_update_encodes_source_validator_and_transport() {
        let source = SourceRef::from_href(
            program().erase(),
            "source/main?version=inactive",
            Some("\"source-1\"".into()),
        )
        .unwrap();
        for transport in [None, Some("A4HK900001")] {
            let mut update = source.update_if_match("REPORT zprogram.").unwrap();
            if let Some(transport) = transport {
                update = update.transport(transport);
            }
            let request = update.encode(&()).unwrap();
            assert_eq!(request.method(), Method::PUT);
            assert_eq!(request.target(), &source.uri);
            assert_eq!(request.headers()[header::IF_MATCH], "\"source-1\"");
            assert_eq!(
                request.headers()[header::CONTENT_TYPE],
                "text/plain; charset=utf-8"
            );
            assert_eq!(request.body(), b"REPORT zprogram.");
            let expected: Vec<_> = transport
                .into_iter()
                .map(|value| ("corrNr".to_owned(), value.to_owned()))
                .collect();
            // Read-version parameters and lock handles do not belong to this PUT.
            assert_eq!(request.query(), expected);
        }
    }

    #[test]
    fn optimistic_source_update_decodes_success_and_conflicts() {
        let mut source = program_source();
        source.etag = Some(EntityTag::from_static("source-1"));
        let update = source.update_if_match("REPORT zprogram.").unwrap();
        for (status, body, expected_content) in [
            (
                StatusCode::OK,
                "REPORT zprogram.\n",
                Some("REPORT zprogram.\n"),
            ),
            (StatusCode::NO_CONTENT, "", None),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::ETAG, HeaderValue::from_static("source-2"));
            let result = update
                .decode(OperationResponse::new(
                    AdtResponse::new(status, headers, body.as_bytes().to_vec()),
                    source.uri.clone(),
                ))
                .unwrap();
            let crate::PreconditionResult::Success(saved) = result else {
                panic!("expected successful source update");
            };
            assert_eq!(saved.content.as_deref(), expected_content);
            assert_eq!(saved.etag.as_deref(), Some("source-2"));
            assert_eq!(saved.reference, source);
        }
        for etag in [None, Some("source-3")] {
            let mut headers = HeaderMap::new();
            if let Some(etag) = etag {
                headers.insert(header::ETAG, etag.parse().unwrap());
            }
            let result = update
                .decode(OperationResponse::new(
                    AdtResponse::new(StatusCode::PRECONDITION_FAILED, headers, vec![0xff]),
                    source.uri.clone(),
                ))
                .unwrap();
            let crate::PreconditionResult::Failed { etag: actual } = result else {
                panic!("expected failed source precondition");
            };
            assert_eq!(actual.as_deref(), etag);
        }
        assert_eq!(source.etag.as_deref(), Some("source-1"));
    }

    #[test]
    fn source_query_requires_plain_text_content() {
        let query = program_source().query();
        let target = query.source.uri.clone();
        let missing = OperationResponse::new(
            AdtResponse::new(
                StatusCode::OK,
                HeaderMap::new(),
                b"REPORT zprogram.".to_vec(),
            ),
            target.clone(),
        );
        assert!(matches!(
            query.decode(missing),
            Err(ResponseError::MissingContentType { target: actual }) if actual == target
        ));

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml"),
        );
        let unsupported = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, b"REPORT zprogram.".to_vec()),
            target,
        );
        assert!(matches!(
            query.decode(unsupported),
            Err(ResponseError::UnsupportedContentType { content_type, .. })
                if content_type == "application/xml"
        ));
    }

    #[test]
    fn one_class_lock_can_update_multiple_source_components() {
        let class = ObjectRef::new(
            ObjectKey::<Class>::new("ZCL_EXAMPLE"),
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );
        let object_lock = ObjectLock::for_test(class.erase(), AccessMode::Modify);
        for uri in [
            "/sap/bc/adt/oo/classes/zcl_example/includes/definitions",
            "/sap/bc/adt/oo/classes/zcl_example/includes/implementations",
        ] {
            let source = source_ref(&class, uri);
            let update = source
                .update_with_lock("source", object_lock.clone())
                .unwrap();
            let request = update.encode(&()).unwrap();

            assert_eq!(request.target(), &source.uri);
            assert_eq!(
                request.query(),
                [("lockHandle".to_owned(), "LOCK-HANDLE".to_owned())]
            );
        }
    }

    #[test]
    fn source_update_rejects_a_lock_for_another_object() {
        let first = ObjectRef::new(
            ObjectKey::<Program>::new("ZFIRST"),
            AdtUri::parse("/sap/bc/adt/programs/programs/zfirst").unwrap(),
        );
        let second = ObjectRef::new(
            ObjectKey::<Program>::new("ZSECOND"),
            AdtUri::parse("/sap/bc/adt/programs/programs/zsecond").unwrap(),
        );
        let object_lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);

        let error = source_ref(&second, "/sap/bc/adt/programs/programs/zsecond/source/main")
            .update_with_lock("REPORT zsecond.", object_lock)
            .unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockMismatch { .. }));
    }

    #[test]
    fn source_update_requires_a_modification_lock() {
        let program = program();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Show);

        let error = program_source()
            .update_with_lock("REPORT zprogram.", object_lock)
            .unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockNotModifiable));
    }

    #[test]
    fn source_update_rejects_the_same_owner_key_at_another_uri() {
        let owner = program();
        let other = ObjectRef::new(owner.key().clone(), AdtUri::parse("other/program").unwrap());
        let lock = ObjectLock::for_test(other.erase(), AccessMode::Modify);

        assert!(matches!(
            program_source().update_with_lock("REPORT zprogram.", lock),
            Err(ObjectError::ObjectLockMismatch { .. })
        ));
    }

    #[test]
    fn source_helpers_preserve_located_owner_and_ignore_parent_metadata_for_locks() {
        let owner = ObjectRef::new(
            ObjectKey::<crate::FunctionGroup>::new("ZFIRST")
                .subobject::<crate::FunctionModule>("ZMODULE"),
            AdtUri::parse("advertised/module").unwrap(),
        )
        .with_parent_uri(AdtUri::parse("advertised/parent").unwrap());
        let source =
            SourceRef::from_href(owner.erase(), "source/main?version=inactive", None).unwrap();
        assert_eq!(source.object, owner.erase());
        assert_eq!(source.object.parent_uri(), owner.parent_uri());
        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/advertised/module/source/main"
        );
        assert_eq!(
            source.query,
            [("version".to_owned(), "inactive".to_owned())]
        );

        let lock_owner = ObjectRef::new(
            ObjectKey::<crate::FunctionGroup>::new("ZSECOND")
                .subobject::<crate::FunctionModule>("ZMODULE"),
            owner.uri().clone(),
        );
        let lock = ObjectLock::for_test(lock_owner.erase(), AccessMode::Modify);
        assert!(source.update_with_lock("source", lock).is_ok());
    }

    #[test]
    fn source_update_inherits_the_locks_transport_request() {
        let program = program();
        let mut source = program_source();
        source
            .query
            .push(("version".to_owned(), "inactive".to_owned()));
        let object_lock =
            ObjectLock::for_test_with_transport(program.erase(), AccessMode::Modify, "A4HK900001");
        let update = source
            .update_with_lock("REPORT zprogram.", object_lock)
            .unwrap();

        let request = update.encode(&()).unwrap();

        assert_eq!(request.method(), Method::PUT);
        assert_eq!(request.target(), &source.uri);
        assert_eq!(
            request.query(),
            [
                ("corrNr".to_owned(), "A4HK900001".to_owned()),
                ("lockHandle".to_owned(), "LOCK-HANDLE".to_owned()),
            ]
        );
        assert_eq!(request.body(), b"REPORT zprogram.");
    }

    #[test]
    fn source_update_accepts_an_explicit_transport_request() {
        let program = program();
        let source = program_source();

        for object_lock in [
            ObjectLock::for_test(program.erase(), AccessMode::Modify),
            ObjectLock::for_test_with_transport(program.erase(), AccessMode::Modify, "A4HK900001"),
        ] {
            let update = source
                .update_with_lock("REPORT zprogram.", object_lock)
                .unwrap()
                .transport("A4HK900002");
            let request = update.encode(&()).unwrap();

            assert_eq!(
                request.query(),
                [
                    ("corrNr".to_owned(), "A4HK900002".to_owned()),
                    ("lockHandle".to_owned(), "LOCK-HANDLE".to_owned()),
                ]
            );
        }
    }

    #[test]
    fn source_update_decodes_canonical_content_and_etag() {
        let program = program();
        let source = program_source();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = source
            .update_with_lock("REPORT zprogram.", object_lock)
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::ETAG, HeaderValue::from_static("source-etag-2"));

        let result = <Locked<SourceUpdate> as Operation>::decode(
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
        let source = program_source();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = source
            .update_with_lock("REPORT zprogram.", object_lock)
            .unwrap();
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

        let error = <Locked<SourceUpdate> as Operation>::decode(
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
        let source = program_source();
        let object_lock = ObjectLock::for_test(program.erase(), AccessMode::Modify);
        let update = source
            .update_with_lock("REPORT zprogram.", object_lock)
            .unwrap();

        let result = <Locked<SourceUpdate> as Operation>::decode(
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
        let update = program_source()
            .update_with_lock("REPORT zprogram.", object_lock)
            .unwrap();
        let session = Client::new(UnusedTransport).create_user_session();

        let error = update.execute(&session).await.unwrap_err();

        assert!(matches!(error, OperationError::UserSessionMismatch));
    }
}
