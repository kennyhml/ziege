use std::fmt;

use http::{Method, StatusCode};
use serde::Deserialize;

use crate::{
    Discovery, PostAction, RequiresDiscovery, User, UserSessionId,
    error::{EncodeError, ObjectError, ResponseError},
    objects::{ObjectKey, ObjectRef, ObjectSnapshot, ObjectTarget, ObjectType},
    operation::{EncodedOperation, Operation, OperationResponse, Stateful},
};

use super::transports::{TransportNumber, TransportRequest};

pub(crate) const LOCK_HANDLE_QUERY: &str = "lockHandle";

/// The access requested when locking an ADT repository object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    /// Locks the object for read-only display.
    Show,

    /// Locks the object for modification.
    Modify,
}

impl AccessMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Show => "SHOW",
            Self::Modify => "MODIFY",
        }
    }
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
    transport_request_owner: Option<User>,
    link_up: bool,
    link_up_mode: Option<String>,
    modification_support: Option<String>,
}

impl ObjectLock {
    const MEDIA_TYPE: &str =
        "application/vnd.sap.as+xml; charset=utf-8; dataname=com.sap.adt.lock.Result2";

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
            ..
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
            transport_request_owner: non_empty(transport_request_owner).map(User::from),
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

    pub(crate) fn validate_modification_for<T>(
        &self,
        object: &ObjectRef<T>,
    ) -> Result<(), ObjectError> {
        if !object.same_identity(&self.object) {
            return Err(ObjectError::ObjectLockMismatch {
                expected: object.to_string(),
                actual: self.object.to_string(),
            });
        }
        if self.access_mode != AccessMode::Modify {
            return Err(ObjectError::ObjectLockNotModifiable);
        }
        Ok(())
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
    pub fn transport_request_owner(&self) -> Option<&User> {
        self.transport_request_owner.as_ref()
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

    /// Consumes this handle and creates an operation that removes the lock.
    pub fn remove(self) -> UnlockRequest {
        UnlockRequest::new(self)
    }

    #[cfg(test)]
    pub(crate) fn for_test(object: ObjectRef, access_mode: AccessMode) -> Self {
        Self {
            object,
            handle: "LOCK-HANDLE".to_owned(),
            access_mode,
            user_session: Some(UserSessionId::generate()),
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

impl<T> ObjectKey<T> {
    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        LockRequest::new(self.erase(), access_mode)
    }

    /// Creates an operation that releases this object's lock.
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        if !self.same_identity(object_lock.object().key()) {
            return Err(ObjectError::ObjectLockMismatch {
                expected: self.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        Ok(UnlockRequest::new(object_lock))
    }
}

impl<T> ObjectRef<T> {
    /// Creates an object-lock operation at this reference's URI.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        LockRequest::new(self.erase(), access_mode)
    }

    /// Releases a lock for this object's identity and URI.
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        if !self.same_identity(object_lock.object()) {
            return Err(ObjectError::ObjectLockMismatch {
                expected: self.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        Ok(UnlockRequest::new(object_lock))
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        self.reference().lock(access_mode)
    }

    /// Creates an operation that releases this object's lock.
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        self.reference().unlock(object_lock)
    }
}

impl ObjectSnapshot<()> {
    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        self.reference().lock(access_mode)
    }

    /// Creates an operation that releases this object's lock.
    pub fn unlock(&self, object_lock: ObjectLock) -> Result<UnlockRequest, ObjectError> {
        self.reference().unlock(object_lock)
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
    target: ObjectTarget,

    /// Whether the object is locked for display or modification.
    pub access_mode: AccessMode,
}

impl LockRequest {
    const ACCESS_MODE_QUERY: &str = "accessMode";

    pub(crate) fn new(target: impl Into<ObjectTarget>, access_mode: AccessMode) -> Self {
        Self {
            target: target.into(),
            access_mode,
        }
    }
}

impl Operation for LockRequest {
    type Response = ObjectLock;
    type Kind = Stateful;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let target = self.target.resolve_uri(resolver)?;
        let mut request = EncodedOperation::new(Method::POST, target);
        request.push_query(PostAction::QUERY_PARAMETER, PostAction::Lock.as_str());
        request.push_query(Self::ACCESS_MODE_QUERY, self.access_mode.as_str());
        request.set_accept(ObjectLock::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        Ok(ObjectLock::parse(
            self.target.at(response.request_target().clone()),
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

impl Operation for UnlockRequest {
    type Response = ();
    type Kind = Stateful;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, _: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let target = self.object_lock.object().uri().clone();
        let mut request = EncodedOperation::new(Method::POST, target);
        request.push_query(PostAction::QUERY_PARAMETER, PostAction::Unlock.as_str());
        request.push_query(LOCK_HANDLE_QUERY, self.object_lock.handle());
        if let Some(user_session) = self.object_lock.user_session() {
            request.bind_user_session(user_session);
        }
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[derive(Deserialize)]
#[serde(rename = "asx:abap", deny_unknown_fields)]
struct RawLock {
    #[serde(rename = "@version", default)]
    _version: Option<String>,

    #[serde(rename = "asx:values")]
    values: RawLockValues,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockValues {
    #[serde(rename = "DATA")]
    data: RawLockData,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(rename = "CORR_LOCKS", default)]
    _transport_locks: RawLockTransports,
    #[serde(rename = "CORR_CONTENTS", default)]
    _transport_contents: RawLockTransportContents,
    #[serde(rename = "SCOPE_MESSAGES", default)]
    _scope_messages: RawLockScopeMessages,
}

// Container and field names are evidenced by Eclipse ADT 3.52's AdtLockResult
// and its ObjectEntry, TransportObject, ObjectEntryAsAdtReference, LockMessage,
// and LockMessageResult parsers. CTS_REQ_HEADER also has a repository fixture.
// These are the evidenced fields, not a complete DDIC E071/MSG schema: additional
// backend fields must be researched before accepting them, not silently skipped.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockTransports {
    #[serde(rename = "CTS_REQ_HEADER", default)]
    _requests: Vec<TransportRequest>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockTransportContents {
    #[serde(rename = "SADT_TRANSPORT_REQ_CONTENTS", default)]
    _entries: Vec<RawLockTransportContent>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLockTransportContent {
    #[serde(rename = "REQ_HEADER")]
    _request: String,
    #[serde(rename = "REQ_TASK")]
    _task: String,
    #[serde(rename = "OWNER")]
    _owner: String,
    #[serde(rename = "OBJ_LIST")]
    _objects: RawLockTransportObjects,
    #[serde(rename = "OBJ_REF_LIST")]
    _references: RawLockObjectReferences,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockTransportObjects {
    #[serde(rename = "E071", default)]
    _objects: Vec<RawLockTransportObject>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLockTransportObject {
    #[serde(rename = "PGMID")]
    _program_id: String,
    #[serde(rename = "OBJECT")]
    _object_type: String,
    #[serde(rename = "OBJ_NAME")]
    _object_name: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockObjectReferences {
    #[serde(rename = "SADT_OBJECT_REFERENCE", default)]
    _references: Vec<RawLockObjectReference>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLockObjectReference {
    #[serde(rename = "URI")]
    _uri: String,
    #[serde(rename = "TYPE")]
    _object_type: String,
    #[serde(rename = "NAME")]
    _name: String,
    #[serde(rename = "PARENT_URI")]
    _parent_uri: String,
    #[serde(rename = "PACKAGE_NAME")]
    _package: String,
    #[serde(rename = "DESCRIPTION")]
    _description: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockScopeMessages {
    #[serde(rename = "SADT_SCOPE_LOCK_MESSAGE", default)]
    _messages: Vec<RawLockScopeMessage>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLockScopeMessage {
    #[serde(rename = "SCOPE")]
    _scope: String,
    #[serde(rename = "TEXT")]
    _text: String,
    #[serde(rename = "LONGTEXT")]
    _long_text: String,
    #[serde(rename = "MSG")]
    _message: RawLockMessage,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLockMessage {
    #[serde(rename = "MSGTY")]
    _severity: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtResponse, AdtUri, Program};
    use http::HeaderMap;

    const LOCK_XML: &[u8] = include_bytes!("../../tests/fixtures/object-lock.xml");

    fn program() -> ObjectRef<Program> {
        ObjectRef::new(
            ObjectKey::<Program>::new("ZTEST"),
            AdtUri::parse("/sap/bc/adt/programs/programs/ztest").unwrap(),
        )
    }

    fn discovery(xml: &[u8]) -> Discovery {
        struct UnusedTransport;

        #[async_trait::async_trait]
        impl crate::Transport for UnusedTransport {
            async fn send(
                &self,
                _: crate::AdtRequest,
            ) -> Result<AdtResponse, crate::TransportError> {
                unreachable!("locking tests never send requests")
            }
        }

        crate::Client::new(UnusedTransport)
            .with_capabilities(
                crate::api::discovery::parse_capabilities(xml).unwrap(),
                crate::api::discovery::parse_capabilities(xml).unwrap(),
            )
            .discovery()
            .clone()
    }

    // Synthetic values using the container nesting and fields read by the local
    // Eclipse AdtLockResult parsers, not a captured populated backend response.
    const LOCK_CONTENTS: &str = r#"
        <CORR_CONTENTS><SADT_TRANSPORT_REQ_CONTENTS>
            <REQ_HEADER>DEVK900001</REQ_HEADER><REQ_TASK>DEVK900002</REQ_TASK>
            <OWNER>DEVELOPER</OWNER>
            <OBJ_LIST><E071><PGMID>LIMU</PGMID><OBJECT>CINC</OBJECT>
                <OBJ_NAME>ZCL_EXAMPLE==============CCAU</OBJ_NAME></E071></OBJ_LIST>
            <OBJ_REF_LIST><SADT_OBJECT_REFERENCE>
                <URI>/sap/bc/adt/oo/classes/zcl_example/includes/testclasses</URI>
                <TYPE>CLAS/OC</TYPE><NAME>ZCL_EXAMPLE</NAME>
                <PARENT_URI>/sap/bc/adt/oo/classes/zcl_example</PARENT_URI>
                <PACKAGE_NAME>ZPACKAGE</PACKAGE_NAME><DESCRIPTION>Test classes</DESCRIPTION>
            </SADT_OBJECT_REFERENCE></OBJ_REF_LIST>
        </SADT_TRANSPORT_REQ_CONTENTS></CORR_CONTENTS>
        <SCOPE_MESSAGES><SADT_SCOPE_LOCK_MESSAGE>
            <SCOPE>ARS</SCOPE><TEXT>Example warning</TEXT><LONGTEXT>Details</LONGTEXT>
            <MSG><MSGTY>W</MSGTY></MSG>
        </SADT_SCOPE_LOCK_MESSAGE></SCOPE_MESSAGES>"#;

    #[test]
    fn models_populated_lock_collections() {
        let requests = include_str!("../../tests/fixtures/transport-requests.xml");
        let headers = &requests[requests.find("<CTS_REQ_HEADER>").unwrap()
            ..requests.rfind("</CTS_REQ_HEADER>").unwrap() + "</CTS_REQ_HEADER>".len()];
        let xml = std::str::from_utf8(LOCK_XML)
            .unwrap()
            .replace(
                "<CORR_LOCKS />",
                &format!("<CORR_LOCKS>{headers}</CORR_LOCKS>"),
            )
            .replace("<CORR_CONTENTS />", LOCK_CONTENTS)
            .replace("<SCOPE_MESSAGES />", "");
        let raw: RawLock = serde_xml_rs::from_str(&xml).unwrap();
        assert_eq!(raw._version.as_deref(), Some("1.0"));
        let data = raw.values.data;
        assert_eq!(data._transport_locks._requests.len(), 2);
        assert_eq!(
            data._transport_locks._requests[0].number.as_str(),
            "DEVK900001"
        );
        let content = &data._transport_contents._entries[0];
        assert_eq!(content._request, "DEVK900001");
        assert_eq!(content._task, "DEVK900002");
        assert_eq!(content._owner, "DEVELOPER");
        assert_eq!(content._objects._objects[0]._program_id, "LIMU");
        assert_eq!(content._references._references[0]._package, "ZPACKAGE");
        let message = &data._scope_messages._messages[0];
        assert_eq!(message._scope, "ARS");
        assert_eq!(message._message._severity, "W");
        ObjectLock::parse(program().erase(), AccessMode::Modify, None, xml.as_bytes()).unwrap();
    }

    #[test]
    fn lock_responses_reject_unknown_fields_in_wrappers_and_containers() {
        let xml = std::str::from_utf8(LOCK_XML)
            .unwrap()
            .replace("<CORR_LOCKS />", "<CORR_LOCKS></CORR_LOCKS>")
            .replace("<CORR_CONTENTS />", LOCK_CONTENTS)
            .replace("<SCOPE_MESSAGES />", "");
        for container in [
            "asx:abap",
            "asx:values",
            "DATA",
            "CORR_LOCKS",
            "CORR_CONTENTS",
            "SADT_TRANSPORT_REQ_CONTENTS",
            "OBJ_LIST",
            "E071",
            "OBJ_REF_LIST",
            "SADT_OBJECT_REFERENCE",
            "SCOPE_MESSAGES",
            "SADT_SCOPE_LOCK_MESSAGE",
            "MSG",
        ] {
            for addition in ["<UNEXPECTED />", "unexpected text"] {
                let closing = format!("</{container}>");
                assert!(xml.contains(&closing));
                let changed = xml.replacen(&closing, &format!("{addition}{closing}"), 1);
                let error = ObjectLock::parse(
                    program().erase(),
                    AccessMode::Modify,
                    None,
                    changed.as_bytes(),
                )
                .unwrap_err();
                assert!(
                    matches!(error, ObjectError::InvalidLockResponse(_)),
                    "{container}: {error}"
                );
                if addition == "<UNEXPECTED />" {
                    assert!(
                        error.to_string().contains("unknown field `UNEXPECTED`"),
                        "{container}: {error}"
                    );
                }
            }
        }
        let changed = xml.replacen("<asx:abap ", "<asx:abap unexpected=\"value\" ", 1);
        let error = ObjectLock::parse(
            program().erase(),
            AccessMode::Modify,
            None,
            changed.as_bytes(),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("unknown field `@unexpected`"),
            "{error}"
        );
    }

    #[test]
    fn parses_object_lock_and_transport_metadata() {
        let object = program().erase();
        let lock = ObjectLock::parse(
            object,
            AccessMode::Modify,
            Some(UserSessionId::generate()),
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
        let object = ObjectRef::new(
            ObjectKey::<crate::Class>::new("ZCL_TEST"),
            AdtUri::parse("/sap/bc/adt/oo/classes/zcl_test").unwrap(),
        )
        .erase();
        let lock = ObjectLock::parse(
            object,
            AccessMode::Modify,
            Some(UserSessionId::generate()),
            xml.as_bytes(),
        )
        .unwrap();

        assert!(lock.is_transport_relevant());
        assert_eq!(
            lock.transport_request().map(TransportNumber::as_str),
            Some("A4HK900001")
        );
        assert_eq!(
            lock.transport_request_owner().map(User::as_str),
            Some("DEVELOPER")
        );
        assert_eq!(lock.transport_request_description(), Some("Source update"));
        assert!(lock.is_link_up());
        assert_eq!(lock.link_up_mode(), Some("MultipleRequests"));
    }

    #[test]
    fn locking_operations_require_discovery() {
        fn requires_discovery<O: Operation<ResolutionRequirement = RequiresDiscovery>>() {}

        requires_discovery::<LockRequest>();
        requires_discovery::<UnlockRequest>();
    }

    #[test]
    fn lock_decode_preserves_logical_object_identity_and_actual_request_uri() {
        let object = ObjectKey::<Program>::new("ZTEST").erase();
        let request = LockRequest::new(object.clone(), AccessMode::Modify);
        let target = AdtUri::parse("/sap/bc/adt/programs/programs/ztest").unwrap();
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, HeaderMap::new(), LOCK_XML.to_vec()),
            target.clone(),
        );

        let object_lock = request.decode(response).unwrap();

        assert_eq!(object_lock.object().key(), &object);
        assert_eq!(object_lock.object().uri(), &target);
    }

    #[test]
    fn object_rejects_another_objects_lock_for_unlock() {
        let first = ObjectRef::new(
            ObjectKey::<Program>::new("ZFIRST"),
            AdtUri::parse("/sap/bc/adt/programs/programs/zfirst").unwrap(),
        );
        let second = ObjectKey::<Program>::new("ZSECOND");
        let object_lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);

        let error = second.unlock(object_lock).unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockMismatch { .. }));
    }

    #[test]
    fn object_accepts_the_same_normalized_logical_identity_for_unlock() {
        let first = ObjectRef::new(
            ObjectKey::<Program>::new("ztest"),
            AdtUri::parse("/sap/bc/adt/programs/programs/ztest").unwrap(),
        );
        let second = ObjectKey::<Program>::new("ZTEST");
        let object_lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);

        assert!(second.unlock(object_lock).is_ok());
    }

    #[test]
    fn lock_and_unlock_preserve_the_request_uri_across_discovery_drift() {
        let xml = include_str!("../../tests/fixtures/discovery.xml");
        let original = discovery(xml.as_bytes());
        let changed = discovery(
            xml.replace("programs/programs", "relocated/programs")
                .as_bytes(),
        );
        let empty = discovery(br#"<app:service xmlns:app="http://www.w3.org/2007/app" />"#);
        let key = program().key().clone();
        let request = key.lock(AccessMode::Modify);
        let target = request.encode(&original).unwrap().target().clone();
        assert_ne!(request.encode(&changed).unwrap().target(), &target);
        let lock = request
            .decode(OperationResponse::new(
                AdtResponse::new(StatusCode::OK, HeaderMap::new(), LOCK_XML.to_vec()),
                target.clone(),
            ))
            .unwrap();
        assert_eq!(lock.object().key(), &key.erase());
        assert_eq!(lock.object().uri(), &target);
        for resolver in [&changed, &empty] {
            let unlock = key.unlock(lock.clone()).unwrap().encode(resolver).unwrap();
            assert_eq!(unlock.target(), &target);
            assert_eq!(
                unlock.query(),
                &[
                    ("_action".to_owned(), "UNLOCK".to_owned()),
                    ("lockHandle".to_owned(), "LOCK-HANDLE-1".to_owned()),
                ]
            );
            assert_eq!(
                lock.clone().remove().encode(resolver).unwrap().target(),
                &target
            );
            assert_eq!(
                lock.object()
                    .lock(AccessMode::Modify)
                    .encode(resolver)
                    .unwrap()
                    .target(),
                &target
            );
            assert_eq!(
                lock.object()
                    .unlock(lock.clone())
                    .unwrap()
                    .encode(resolver)
                    .unwrap()
                    .target(),
                &target
            );
        }

        let located = program().with_parent_uri(AdtUri::parse("advertised/parent").unwrap());
        let actual = AdtUri::parse("actual/lock/target").unwrap();
        let lock = located
            .lock(AccessMode::Modify)
            .decode(OperationResponse::new(
                AdtResponse::new(StatusCode::OK, HeaderMap::new(), LOCK_XML.to_vec()),
                actual.clone(),
            ))
            .unwrap();
        assert_eq!(lock.object().uri(), &actual);
        assert_eq!(lock.object().parent_uri(), located.parent_uri());
        assert_eq!(lock.remove().encode(&empty).unwrap().target(), &actual);
    }

    #[test]
    fn located_unlock_and_modification_reject_the_same_key_at_another_uri() {
        let first = program();
        let second = ObjectRef::new(first.key().clone(), AdtUri::parse("other/program").unwrap());
        let lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);
        assert!(matches!(
            second.unlock(lock.clone()),
            Err(ObjectError::ObjectLockMismatch { .. })
        ));
        assert!(matches!(
            lock.validate_modification_for(&second),
            Err(ObjectError::ObjectLockMismatch { .. })
        ));
    }

    #[test]
    fn located_unlock_and_modification_ignore_parent_metadata() {
        let first = ObjectRef::new(
            ObjectKey::<crate::FunctionGroup>::new("ZFIRST")
                .subobject::<crate::FunctionModule>("ZMODULE"),
            AdtUri::parse("advertised/module").unwrap(),
        );
        let second = ObjectRef::new(
            ObjectKey::<crate::FunctionGroup>::new("ZSECOND")
                .subobject::<crate::FunctionModule>("ZMODULE"),
            first.uri().clone(),
        )
        .with_parent_uri(AdtUri::parse("advertised/parent").unwrap());
        assert_ne!(first.key(), second.key());
        let lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);
        assert!(second.unlock(lock.clone()).is_ok());
        assert!(lock.validate_modification_for(&second).is_ok());
    }
}
