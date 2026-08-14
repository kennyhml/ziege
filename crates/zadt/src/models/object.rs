use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    EntityTag, GlobalWorkbenchType, ObjectError, ObjectRef, SourceRef, TransportNumber,
    operation::UserSessionId,
};

/// An unresolved object reference exactly as advertised in an ADT payload.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdvertisedObjectReference {
    /// The referenced object's URI, when advertised.
    #[serde(rename = "@adtcore:uri", skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// The referenced object's global Workbench type, when advertised.
    #[serde(rename = "@adtcore:type", skip_serializing_if = "Option::is_none")]
    pub object_type: Option<GlobalWorkbenchType>,

    /// The referenced object's name, when advertised.
    #[serde(rename = "@adtcore:name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The referenced object's package name, when advertised.
    #[serde(
        rename = "@adtcore:packageName",
        skip_serializing_if = "Option::is_none"
    )]
    pub package_name: Option<String>,

    /// The referenced object's description, when advertised.
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdtUri;

    const LOCK_XML: &[u8] = include_bytes!("../../tests/fixtures/object-lock.xml");

    #[test]
    fn advertised_object_references_preserve_partial_wire_values() {
        let xml = r#"<adtcore:objectRef adtcore:type="CLAS/OC" adtcore:name="ZCL_TEST" adtcore:packageName="ZPACKAGE" xmlns:adtcore="http://www.sap.com/adt/core" />"#;
        let reference: AdvertisedObjectReference = serde_xml_rs::from_str(xml).unwrap();

        assert_eq!(reference.object_type.as_ref().unwrap().as_str(), "CLAS/OC");
        assert_eq!(reference.name.as_deref(), Some("ZCL_TEST"));
        assert_eq!(reference.package_name.as_deref(), Some("ZPACKAGE"));
        assert!(reference.uri.is_none());

        let json = serde_json::to_value(&reference).unwrap();
        assert_eq!(json["@adtcore:type"], "CLAS/OC");
        assert_eq!(json["@adtcore:packageName"], "ZPACKAGE");
        assert!(json.get("@adtcore:uri").is_none());
        assert_eq!(
            serde_json::from_value::<AdvertisedObjectReference>(json).unwrap(),
            reference
        );
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
}
