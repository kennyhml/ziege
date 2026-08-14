use std::{collections::HashMap, fmt};

use http::{Method, StatusCode};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    client::{Client, ClientState, Ready},
    error::{ObjectError, OperationError, ResponseError},
    objects::{GlobalWorkbenchType, HasSource, ImmediateRun, ObjectRef, ObjectType, RunCapability},
    operation::{Operation, OperationResponse, Stateful, Stateless, UserSessionId},
    protocol::{AdtRequest, AdtResponse, EntityTag},
    resource::SourceRef,
    vocabulary::{PostAction, media_type, query_parameter},
};

use super::transports::TransportNumber;

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

/// Plain-text output produced by running a type-erased repository object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectRunResult {
    /// The type-erased object that was executed.
    pub reference: ObjectRef,

    /// The exact Workbench type of the executed object.
    pub object_type: GlobalWorkbenchType,

    /// The rendered output returned by SAP.
    pub content: String,
}

impl ObjectRunResult {
    pub(crate) fn new(
        reference: ObjectRef,
        object_type: GlobalWorkbenchType,
        content: String,
    ) -> Self {
        Self {
            reference,
            object_type,
            content,
        }
    }
}

/// The access requested when locking an ADT repository object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    /// Locks the object for read-only display.
    Show,

    /// Locks the object for modification.
    Modify,
}

/// An opaque lock obtained for one object in a specific SAP user session.
///
/// The handle is bound to both [`ObjectRef`] and [`crate::UserSession`]. A
/// handle string alone is not sufficient to update another resource.
#[derive(Clone, Eq, PartialEq)]
pub struct ObjectLock {
    /// The locked object.
    object: ObjectRef,

    /// The opaque handle supplied by SAP.
    handle: String,

    access_mode: AccessMode,
    user_session: Option<UserSessionId>,
    transport_relevant: bool,
    transport_request: Option<TransportNumber>,
    transport_request_description: Option<String>,
    transport_request_owner: Option<String>,
    link_up: bool,
    link_up_mode: Option<String>,
    modification_support: Option<String>,
}

impl ObjectLock {
    pub(crate) fn parse(
        object: ObjectRef,
        access_mode: AccessMode,
        user_session: Option<UserSessionId>,
        body: &[u8],
    ) -> Result<Self, ObjectError> {
        let raw: RawLock =
            serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidLockResponse)?;
        let RawLockData {
            lock_handle,
            transport_request,
            transport_request_owner,
            transport_request_description,
            is_local,
            is_link_up,
            modification_support,
            link_up_mode,
        } = raw.values.data;
        let handle = non_empty(lock_handle).ok_or(ObjectError::MissingLockHandle)?;

        Ok(Self {
            object,
            handle,
            access_mode,
            user_session,
            transport_relevant: !is_local.eq_ignore_ascii_case("X"),
            transport_request: non_empty(transport_request).map(TransportNumber::from),
            transport_request_description: non_empty(transport_request_description),
            transport_request_owner: non_empty(transport_request_owner),
            link_up: is_link_up.eq_ignore_ascii_case("X"),
            link_up_mode: non_empty(link_up_mode),
            modification_support: non_empty(modification_support),
        })
    }

    /// Returns the object this lock belongs to.
    pub fn object(&self) -> &ObjectRef {
        &self.object
    }

    /// Returns the opaque handle supplied by SAP.
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Returns the access mode with which this lock was acquired.
    pub fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    /// Returns whether changes to this object are transport relevant.
    pub fn is_transport_relevant(&self) -> bool {
        self.transport_relevant
    }

    /// Returns the transport request currently associated with this lock.
    pub fn transport_request(&self) -> Option<&TransportNumber> {
        self.transport_request.as_ref()
    }

    /// Returns the associated transport request description, when supplied.
    pub fn transport_request_description(&self) -> Option<&str> {
        self.transport_request_description.as_deref()
    }

    /// Returns the owner of the associated transport request, when supplied.
    pub fn transport_request_owner(&self) -> Option<&str> {
        self.transport_request_owner.as_deref()
    }

    /// Returns whether SAP requested transport link-up handling.
    pub fn is_link_up(&self) -> bool {
        self.link_up
    }

    /// Returns the exact transport link-up mode supplied by SAP.
    pub fn link_up_mode(&self) -> Option<&str> {
        self.link_up_mode.as_deref()
    }

    /// Returns the exact manual modification-support value supplied by SAP.
    pub fn modification_support(&self) -> Option<&str> {
        self.modification_support.as_deref()
    }

    pub(crate) fn user_session(&self) -> Option<UserSessionId> {
        self.user_session
    }

    #[cfg(test)]
    pub(crate) fn for_test(object: ObjectRef, access_mode: AccessMode) -> Self {
        Self {
            object,
            handle: "LOCK-HANDLE".to_owned(),
            access_mode,
            user_session: Some(UserSessionId::new()),
            transport_relevant: false,
            transport_request: None,
            transport_request_description: None,
            transport_request_owner: None,
            link_up: false,
            link_up_mode: None,
            modification_support: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_with_transport(
        object: ObjectRef,
        access_mode: AccessMode,
        transport_request: impl Into<TransportNumber>,
    ) -> Self {
        let mut object_lock = Self::for_test(object, access_mode);
        object_lock.transport_relevant = true;
        object_lock.transport_request = Some(transport_request.into());
        object_lock
    }
}

impl fmt::Debug for ObjectLock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectLock")
            .field("object", &self.object)
            .field("handle", &"<opaque>")
            .field("access_mode", &self.access_mode)
            .field("transport_relevant", &self.transport_relevant)
            .field("transport_request", &self.transport_request)
            .field("link_up", &self.link_up)
            .field("link_up_mode", &self.link_up_mode)
            .field("modification_support", &self.modification_support)
            .finish()
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[derive(Deserialize)]
#[serde(rename = "asx:abap")]
struct RawLock {
    #[serde(rename = "asx:values")]
    values: RawLockValues,
}

#[derive(Deserialize)]
struct RawLockValues {
    #[serde(rename = "DATA")]
    data: RawLockData,
}

#[derive(Deserialize)]
struct RawLockData {
    #[serde(rename = "LOCK_HANDLE", default)]
    lock_handle: String,
    #[serde(rename = "CORRNR", default)]
    transport_request: String,
    #[serde(rename = "CORRUSER", default)]
    transport_request_owner: String,
    #[serde(rename = "CORRTEXT", default)]
    transport_request_description: String,
    #[serde(rename = "IS_LOCAL", default)]
    is_local: String,
    #[serde(rename = "IS_LINK_UP", default)]
    is_link_up: String,
    #[serde(rename = "MODIFICATION_SUPPORT", default)]
    modification_support: String,
    #[serde(rename = "LINK_UP_MODE", default)]
    link_up_mode: String,
}

/// Runs a type-erased repository object through its descriptor capability.
#[derive(Debug)]
pub struct ObjectRun {
    reference: ObjectRef,
    run: RunCapability,
    profiler_id: Option<String>,
}

impl ObjectRun {
    pub(crate) fn new(reference: ObjectRef, run: RunCapability) -> Self {
        Self {
            reference,
            run,
            profiler_id: None,
        }
    }

    pub(crate) fn typed<T: ImmediateRun>(reference: &ObjectRef<T>) -> Self {
        Self::new(reference.erase(), T::RUN)
    }

    /// Runs the object with the supplied ABAP profiler trace identifier.
    #[must_use]
    pub fn profiler_id(mut self, profiler_id: impl Into<String>) -> Self {
        self.profiler_id = Some(profiler_id.into());
        self
    }
}

impl Operation<Ready> for ObjectRun {
    type Response = ObjectRunResult;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let template = self.run.target.template(client)?;
        if !template.has_variable(self.run.name_variable) {
            return Err(ObjectError::InvalidTemplate {
                template: template.as_str().to_owned(),
                reason: format!("missing `{}` variable", self.run.name_variable),
            }
            .into());
        }
        if self.profiler_id.is_some() && !template.has_variable(query_parameter::PROFILER_ID) {
            return Err(ObjectError::UnsupportedTemplateParameter {
                parameter: query_parameter::PROFILER_ID,
            }
            .into());
        }

        let mut variables = HashMap::from([(
            self.run.name_variable.to_owned(),
            Value::String(self.reference.name().to_ascii_lowercase()),
        )]);
        if let Some(profiler_id) = &self.profiler_id {
            variables.insert(
                query_parameter::PROFILER_ID.to_owned(),
                Value::String(profiler_id.clone()),
            );
        }
        let (target, query) = template.expand(&variables)?;
        let mut request = AdtRequest::new(Method::POST, target);
        for (name, value) in query {
            request.push_query(name, value);
        }
        request.set_accept(media_type::SOURCE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if !response.status().is_success() {
            return Err(ResponseError::unexpected_status(response.response()));
        }
        let content = String::from_utf8(response.into_body())
            .map_err(ObjectError::InvalidResponseEncoding)?;
        Ok(ObjectRunResult::new(
            self.reference.clone(),
            self.reference.object_type().clone(),
            content,
        ))
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
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        if self.uri() != object_lock.object().uri() {
            return Err(ObjectError::ObjectLockMismatch {
                expected: self.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        Ok(UnlockRequest::new(object_lock))
    }
}

impl<T: HasSource> ObjectRef<T> {
    /// Returns the objects conventional source resource.
    pub fn source(&self) -> SourceRef {
        SourceRef::from_object_path(self.erase(), T::SOURCE_PATH)
    }
}

/// Locks a repository object within a [`crate::UserSession`].
///
/// The operation sends `POST` with `_action=LOCK` and the configured
/// `accessMode`. The returned [`ObjectLock`] must remain in the same user
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
    type Response = ObjectLock;
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
        Ok(ObjectLock::parse(
            self.object.clone(),
            self.access_mode,
            response.user_session(),
            response.body(),
        )?)
    }
}

/// Releases an [`ObjectLock`] within its SAP user session.
#[derive(Debug)]
pub struct UnlockRequest {
    /// The lock to release.
    pub object_lock: ObjectLock,
}

impl UnlockRequest {
    pub(crate) fn new(object_lock: ObjectLock) -> Self {
        Self { object_lock }
    }
}

impl<S: ClientState> Operation<S> for UnlockRequest {
    type Response = ();
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::POST, self.object_lock.object().uri().clone());
        request.push_query(query_parameter::ACTION, PostAction::Unlock.as_str());
        request.push_query(query_parameter::LOCK_HANDLE, self.object_lock.handle());
        if let Some(user_session) = self.object_lock.user_session() {
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

impl ObjectLock {
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

    use crate::{AdtUri, Class, ClassSourceComponent, Initial, ObjectRef, Program, Transport};

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");
    const LOCK_XML: &[u8] = include_bytes!("../../tests/fixtures/object-lock.xml");

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

    fn ready_client() -> Client<Ready> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
        )
    }

    #[test]
    fn parses_object_lock_and_transport_metadata() {
        let object = ObjectRef::erased(
            "ZTEST".to_owned(),
            AdtUri::parse("/sap/bc/adt/programs/programs/ztest").unwrap(),
            "PROG/P".parse().unwrap(),
        );
        let lock = ObjectLock::parse(
            object,
            AccessMode::Modify,
            Some(UserSessionId::new()),
            LOCK_XML,
        )
        .unwrap();

        assert_eq!(lock.handle(), "LOCK-HANDLE-1");
        assert_eq!(lock.access_mode(), AccessMode::Modify);
        assert!(!lock.is_transport_relevant());
        assert_eq!(lock.transport_request(), None);
        assert!(!lock.is_link_up());
        assert_eq!(lock.link_up_mode(), None);
        assert_eq!(lock.modification_support(), Some("NoModification"));
    }

    #[test]
    fn preserves_transport_and_link_up_metadata() {
        let xml = String::from_utf8(LOCK_XML.to_vec())
            .unwrap()
            .replace("<CORRNR />", "<CORRNR>A4HK900001</CORRNR>")
            .replace("<CORRUSER />", "<CORRUSER>DEVELOPER</CORRUSER>")
            .replace("<CORRTEXT />", "<CORRTEXT>Source update</CORRTEXT>")
            .replace("<IS_LOCAL>X</IS_LOCAL>", "<IS_LOCAL />")
            .replace("<IS_LINK_UP />", "<IS_LINK_UP>X</IS_LINK_UP>")
            .replace(
                "<LINK_UP_MODE />",
                "<LINK_UP_MODE>MultipleRequests</LINK_UP_MODE>",
            );
        let object = ObjectRef::erased(
            "ZCL_TEST".to_owned(),
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_test").unwrap(),
            "CLAS/OC".parse().unwrap(),
        );
        let lock = ObjectLock::parse(
            object,
            AccessMode::Modify,
            Some(UserSessionId::new()),
            xml.as_bytes(),
        )
        .unwrap();

        assert!(lock.is_transport_relevant());
        assert_eq!(
            lock.transport_request().map(TransportNumber::as_str),
            Some("A4HK900001")
        );
        assert_eq!(lock.transport_request_owner(), Some("DEVELOPER"));
        assert_eq!(lock.transport_request_description(), Some("Source update"));
        assert!(lock.is_link_up());
        assert_eq!(lock.link_up_mode(), Some("MultipleRequests"));
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
    fn type_erased_runs_dispatch_through_object_descriptors() {
        fn accepts_ready<O: Operation<Ready>>() {}
        accepts_ready::<ObjectRun>();

        let client = ready_client();
        let program = program();
        let program_run = program.erase().run().unwrap();
        let program_request = program_run.request(&client).unwrap();
        assert_eq!(
            program_request.target().as_str(),
            "/sap/bc/adt/programs/programrun/zprogram"
        );

        let program_output = program_run
            .decode(OperationResponse::new(
                AdtResponse::new(StatusCode::OK, HeaderMap::new(), b"program output".to_vec()),
                program_request.target().clone(),
            ))
            .unwrap();
        assert_eq!(program_output.reference, program.erase());
        assert_eq!(program_output.object_type.as_str(), "PROG/P");
        assert_eq!(program_output.content, "program output");

        let class = ObjectRef::<Class>::for_test(
            "ZCL_EXAMPLE",
            crate::AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );
        let class_request = class
            .erase()
            .run()
            .unwrap()
            .profiler_id("TRACE ID")
            .request(&client)
            .unwrap();
        assert_eq!(
            class_request.target().as_str(),
            "/sap/bc/adt/oo/classrun/zcl_example"
        );
        assert_eq!(
            class_request.query(),
            [("profilerId".to_owned(), "TRACE ID".to_owned())]
        );
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
            crate::AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
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
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/ZFIRST").unwrap(),
        );
        let second = ObjectRef::<Program>::for_test(
            "ZSECOND",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/ZSECOND").unwrap(),
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
        let object_lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);

        let error = second.unlock(object_lock).unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockMismatch { .. }));
    }
}
