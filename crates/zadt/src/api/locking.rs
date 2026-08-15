use std::fmt;

use http::{Method, StatusCode};
use serde::Deserialize;

use crate::{
    client::{Client, ClientState},
    error::{ObjectError, OperationError, ResponseError},
    objects::{Erased, ObjectRef, ObjectType},
    operation::{Operation, OperationResponse, Stateful, UserSessionId},
    protocol::{AdtRequest, AdtResponse},
    vocabulary::{PostAction, media_type, query_parameter},
};

use super::transports::TransportNumber;

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

impl ObjectRef<Erased> {
    /// Creates an object-lock operation.
    pub fn lock(&self, access_mode: AccessMode) -> LockRequest {
        LockRequest::new(self.clone(), access_mode)
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

fn expect_ok(response: &AdtResponse) -> Result<(), ResponseError> {
    if response.status() == StatusCode::OK {
        Ok(())
    } else {
        Err(ResponseError::unexpected_status(response))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, Initial, Program};

    const LOCK_XML: &[u8] = include_bytes!("../../tests/fixtures/object-lock.xml");

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
    fn locking_operations_do_not_require_discovery() {
        fn accepts_initial<O: Operation<Initial>>() {}

        accepts_initial::<LockRequest>();
        accepts_initial::<UnlockRequest>();
    }

    #[test]
    fn object_rejects_another_objects_lock_for_unlock() {
        let first = ObjectRef::<Program>::for_test(
            "ZFIRST",
            AdtUri::parse("/sap/bc/adt/programs/programs/ZFIRST").unwrap(),
        );
        let second = ObjectRef::<Program>::for_test(
            "ZSECOND",
            AdtUri::parse("/sap/bc/adt/programs/programs/ZSECOND").unwrap(),
        );
        let object_lock = ObjectLock::for_test(first.erase(), AccessMode::Modify);

        let error = second.unlock(object_lock).unwrap_err();

        assert!(matches!(error, ObjectError::ObjectLockMismatch { .. }));
    }
}
