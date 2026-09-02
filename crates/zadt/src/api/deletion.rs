//! Represents the `/sap/bc/adt/deletion` workspace which contains
//! bulk object deletion and deletion check runs. Objects are not
//! deleted by sending a DELETE to the object uri!
use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    AdtUri, Advertised, AdvertisedLink, CategoryId, EncodeError, EncodedOperation,
    GlobalWorkbenchType, ObjectError, ObjectRef, ObjectSnapshot, Operation, OperationResponse,
    ResponseError, SnapshotKind, Stateless, TransportNumber, User, operation::CollectionLocator,
};

/// Checks whether one or more repository objects can be deleted.
///
/// For each object, it is generally enough to pass the object [`AdtUri`] to
/// the server for identification. If the object is currently locked, the
/// lock handle must also be included to avoid locking conflicts.
///
/// The response informs us whether an object can be deleted and any included
/// objects that may have to be deleted with it, as is the case for function
/// modules inside function groups, for instance.
///
/// Backend handler: `CL_ADT_DELETION_CHECK_RES`
#[derive(Debug, Default)]
pub struct DeletionCheck {
    payload: DeletionCheckRequest,
}

impl DeletionCheck {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/deletion",
        term: "check",
    };

    /// Creates an empty deletion check.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an object to the deletion check.
    pub fn push_object(&mut self, object: impl Into<DeletionCheckObject>) -> &mut Self {
        self.payload.objects.push(object.into());
        self
    }
}

impl Operation for DeletionCheck {
    type Kind = Stateless;
    type Response = DeletionCheckResponse;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let mut request = CollectionLocator::new(Self::CATEGORY).operation(Method::POST);
        request.set_body(self.payload.serialize()?);
        request.set_content_type(DeletionCheckRequest::MEDIA_TYPE);
        request.set_accept(DeletionCheckResponse::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[DeletionCheckResponse::MEDIA_TYPE])?;
        serde_xml_rs::from_reader(response.body())
            .map_err(ObjectError::InvalidResponse)
            .map_err(Into::into)
    }
}

impl<T> ObjectRef<T> {
    /// Constructs a [`DeletionCheck`] containing this object.
    ///
    /// Other objects may be added to the check.
    pub fn deletion_check(&self) -> DeletionCheck {
        let mut check = DeletionCheck::new();
        check.push_object(self);
        check
    }
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Constructs a [`DeletionCheck`] containing this object.
    ///
    /// Other objects may be added to the check.
    pub fn deletion_check(&self) -> DeletionCheck {
        self.reference().deletion_check()
    }
}

/// Deletes one or more repository objects.
///
/// Just like in an [`DeletionCheck`], an [`AdtUri`] is enough to identify
/// the objects to be deleted. However, for transportable objects, a transport
/// request must be assigned as a deletion is also a development change that
/// requires recording.
///
/// The backend returns a `200 OK` in any case. Whether an object was actually
/// deleted is determined by the `is_deleted` result. Contained messages can
/// provide information to troubleshoot when needed.
///
/// Backend handler: `CL_ADT_DELETION_RESOURCE`
#[derive(Debug, Default)]
pub struct ObjectDeletion {
    payload: DeletionRequest,
}

impl ObjectDeletion {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/deletion",
        term: "delete",
    };

    /// Creates an empty bulk deletion.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an object to the deletion request.
    pub fn push_object(&mut self, object: impl Into<DeletionObject>) -> &mut Self {
        self.payload.objects.push(object.into());
        self
    }
}

impl Operation for ObjectDeletion {
    type Kind = Stateless;
    type Response = DeletionResult;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let mut request = CollectionLocator::new(Self::CATEGORY).operation(Method::POST);
        request.set_body(self.payload.serialize()?);
        request.set_content_type(DeletionRequest::MEDIA_TYPE);
        request.set_accept(DeletionResult::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[DeletionResult::MEDIA_TYPE])?;
        if response.body().is_empty() {
            return Ok(DeletionResult::default());
        }
        serde_xml_rs::from_reader(response.body())
            .map_err(ObjectError::InvalidResponse)
            .map_err(Into::into)
    }
}

impl<T> ObjectRef<T> {
    /// Constructs a deletion request containing this local object.
    pub fn deletion(&self) -> ObjectDeletion {
        let mut deletion = ObjectDeletion::new();
        deletion.push_object(self);
        deletion
    }

    /// Constructs a deletion request recorded in the supplied transport.
    pub fn deletion_with_transport(&self, transport: impl Into<TransportNumber>) -> ObjectDeletion {
        let mut deletion = ObjectDeletion::new();
        deletion.push_object(DeletionObject::new(self).transport(transport));
        deletion
    }
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Constructs a deletion request containing this local object.
    pub fn deletion(&self) -> ObjectDeletion {
        self.reference().deletion()
    }

    /// Constructs a deletion request recorded in the supplied transport.
    pub fn deletion_with_transport(&self, transport: impl Into<TransportNumber>) -> ObjectDeletion {
        self.reference().deletion_with_transport(transport)
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename = "del:checkRequest")]
pub struct DeletionCheckRequest {
    #[serde(rename = "del:object")]
    pub objects: Vec<DeletionCheckObject>,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "del:checkResponse")]
pub struct DeletionCheckResponse {
    /// Results in request order.
    #[serde(rename = "del:object", default)]
    pub objects: Vec<DeletionCheckObjectResult>,
}

impl DeletionCheckRequest {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.check.request.v1+xml";
    const DELETION_NAMESPACE: &str = "http://www.sap.com/adt/deletion";
    const ADT_CORE_NAMESPACE: &str = "http://www.sap.com/adt/core";

    /// Serializes the deletion-check request using the ADT XML namespaces.
    pub fn serialize(&self) -> Result<String, ObjectError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("del", Self::DELETION_NAMESPACE)
            .namespace("adtcore", Self::ADT_CORE_NAMESPACE)
            .to_string(self)
            .map_err(ObjectError::InvalidRequest)
    }
}

impl DeletionCheckResponse {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.check.response.v1+xml";
}

/// Wire payload sent to the ADT deletion endpoint.
#[derive(Debug, Default, Serialize)]
#[serde(rename = "del:deletionRequest")]
pub struct DeletionRequest {
    /// Objects to delete.
    #[serde(rename = "del:object")]
    pub objects: Vec<DeletionObject>,
}

impl DeletionRequest {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.request.v1+xml";
    const DELETION_NAMESPACE: &str = "http://www.sap.com/adt/deletion";
    const ADT_CORE_NAMESPACE: &str = "http://www.sap.com/adt/core";

    /// Serializes the deletion request using the ADT XML namespaces.
    pub fn serialize(&self) -> Result<String, ObjectError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("del", Self::DELETION_NAMESPACE)
            .namespace("adtcore", Self::ADT_CORE_NAMESPACE)
            .to_string(self)
            .map_err(ObjectError::InvalidRequest)
    }
}

/// One object to deletion check. It seems URI and lock are the
/// only mandatory fields. Name and type are possible but are
/// instead derived from the URI.
#[derive(Clone, Debug, Serialize)]
pub struct DeletionCheckObject {
    #[serde(rename = "@adtcore:uri")]
    pub uri: AdtUri,

    #[serde(rename = "del:lockHandle")]
    pub lock_handle: Option<String>,
}

impl DeletionCheckObject {
    /// Creates a deletion-check entry for an object reference.
    pub fn new<T>(object: &ObjectRef<T>) -> Self {
        object.into()
    }

    /// Supplies a lock handle retained by the current ADT user session.
    #[must_use]
    pub fn lock_handle(mut self, lock_handle: impl Into<String>) -> Self {
        self.lock_handle = Some(lock_handle.into());
        self
    }
}

impl<T> From<&ObjectRef<T>> for DeletionCheckObject {
    fn from(value: &ObjectRef<T>) -> Self {
        Self {
            uri: value.uri().clone(),
            lock_handle: None,
        }
    }
}

/// One object in a deletion request.
#[derive(Clone, Debug, Serialize)]
pub struct DeletionObject {
    /// URI of the object to delete.
    #[serde(rename = "@adtcore:uri")]
    pub uri: AdtUri,

    /// Transport request that records deletion of a transportable object.
    ///
    /// This remains empty when deleting a local object.
    #[serde(rename = "del:transportNumber")]
    pub transport_number: String,
}

impl DeletionObject {
    /// Creates a deletion entry for an object reference.
    pub fn new<T>(object: &ObjectRef<T>) -> Self {
        object.into()
    }

    /// Records this object's deletion in the supplied transport request.
    #[must_use]
    pub fn transport(mut self, transport: impl Into<TransportNumber>) -> Self {
        self.transport_number = transport.into().into();
        self
    }
}

impl<T> From<&ObjectRef<T>> for DeletionObject {
    fn from(value: &ObjectRef<T>) -> Self {
        Self {
            uri: value.uri().clone(),
            transport_number: String::new(),
        }
    }
}

/// The result for one checked object.
#[derive(Debug, Deserialize)]
pub struct DeletionCheckObjectResult {
    /// Strong references from objects outside the proposed deletion set.
    #[serde(rename = "@del:externalStrongReferences")]
    pub external_strong_references: i32,

    /// Weak references from objects outside the proposed deletion set.
    #[serde(rename = "@del:externalWeakReferences")]
    pub external_weak_references: i32,

    #[serde(rename = "@del:isDeletable")]
    pub is_deletable: bool,

    #[serde(rename = "@adtcore:uri")]
    pub uri: AdtUri,

    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    #[serde(rename = "@adtcore:name")]
    pub name: String,

    #[serde(rename = "@adtcore:packageName")]
    pub package_name: Option<String>,

    #[serde(rename = "@adtcore:description")]
    pub description: Option<String>,

    #[serde(rename = "@adtcore:parentUri")]
    pub parent_uri: Option<String>,

    /// User currently locking the object, when the lock check failed.
    #[serde(rename = "del:lockUser", default)]
    pub lock_user: Option<User>,

    #[serde(rename = "del:lockingTransport")]
    pub locking_transport: DeletionLockingTransport,

    #[serde(rename = "del:message", default)]
    pub messages: Vec<DeletionMessage>,

    /// Relationships to other objects in the proposed deletion set.
    #[serde(rename = "del:usage", default)]
    pub usages: Vec<DeletionUsage>,

    /// Objects that the backend associates with this deletion candidate.
    #[serde(rename = "del:includedObject", default)]
    pub included_objects: Vec<DeletionIncludedObject>,
}

/// A repository usage between objects in the proposed deletion set.
#[derive(Debug, Deserialize)]
pub struct DeletionUsage {
    /// Backend-defined relationship degree, such as `strong` or `weak`.
    #[serde(rename = "@del:degree")]
    pub degree: String,

    /// Whether the referencing object is outside the proposed deletion set.
    #[serde(rename = "@del:isExternal")]
    pub is_external: bool,

    #[serde(rename = "@adtcore:uri")]
    pub uri: AdtUri,

    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    #[serde(rename = "@adtcore:name")]
    pub name: String,

    #[serde(rename = "@adtcore:packageName")]
    pub package_name: Option<String>,

    #[serde(rename = "@adtcore:description")]
    pub description: Option<String>,

    #[serde(rename = "@adtcore:parentUri")]
    pub parent_uri: Option<String>,
}

/// An object included by the backend when checking a deletion candidate.
#[derive(Debug, Deserialize)]
pub struct DeletionIncludedObject {
    /// Whether this object may be deleted independently from its parent.
    #[serde(rename = "@del:canBeDeletedWithoutParent")]
    pub can_be_deleted_without_parent: bool,

    /// Whether deleting the parent also requires deleting this object.
    #[serde(rename = "@del:mustBeDeletedWithParent")]
    pub must_be_deleted_with_parent: bool,

    #[serde(rename = "@del:isDeletable")]
    pub is_deletable: bool,

    #[serde(rename = "@adtcore:uri")]
    pub uri: AdtUri,

    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    #[serde(rename = "@adtcore:name")]
    pub name: String,

    #[serde(rename = "@adtcore:packageName")]
    pub package_name: Option<String>,

    #[serde(rename = "@adtcore:description")]
    pub description: Option<String>,

    #[serde(rename = "@adtcore:parentUri")]
    pub parent_uri: Option<String>,

    /// User currently locking the included object, when available.
    #[serde(rename = "del:lockUser", default)]
    pub lock_user: Option<User>,

    #[serde(rename = "del:lockingTransport")]
    pub locking_transport: DeletionLockingTransport,

    #[serde(rename = "del:message", default)]
    pub messages: Vec<DeletionMessage>,
}

/// Result returned by the ADT deletion endpoint.
#[derive(Debug, Default, Deserialize)]
#[serde(rename = "del:deletionResult")]
pub struct DeletionResult {
    /// Per-object deletion results in request order.
    #[serde(rename = "del:object", default)]
    pub objects: Vec<DeletionObjectResult>,
}

impl DeletionResult {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.response.v1+xml";
}

/// The result of deleting one repository object.
#[derive(Debug, Deserialize)]
pub struct DeletionObjectResult {
    /// Whether the backend deleted the object.
    #[serde(rename = "@del:isDeleted")]
    pub is_deleted: bool,

    /// URI submitted for deletion.
    #[serde(rename = "@adtcore:uri")]
    pub uri: AdtUri,

    /// Resolved global Workbench type, when available.
    #[serde(rename = "@adtcore:type")]
    pub object_type: Option<GlobalWorkbenchType>,

    /// Resolved object name, when available.
    #[serde(rename = "@adtcore:name")]
    pub name: Option<String>,

    /// Package containing the object, when available.
    #[serde(rename = "@adtcore:packageName")]
    pub package_name: Option<String>,

    /// Messages emitted for this object.
    #[serde(rename = "del:message", default)]
    pub messages: Vec<DeletionMessage>,
}

/// Transport locking a checked object
#[derive(Debug, Deserialize)]
pub struct DeletionLockingTransport {
    #[serde(rename = "del:recording")]
    pub recording: bool,
    #[serde(rename = "del:result")]
    pub result: String,

    /// Messages returned by the CTS transport check.
    #[serde(rename = "del:message", default)]
    pub messages: Vec<DeletionTransportMessage>,

    #[serde(rename = "del:lockingTransport")]
    pub properties: DeletionLockingTransportProperties,
    #[serde(rename = "del:transportLayer")]
    pub transport_layer: String,
}

/// WARN: At least on ABAP Cloud Trial 2025, this struct seems to be
/// bugged. The status and type is encoded into the owner while the
/// rest of the properties are part of the description.
#[derive(Debug, Deserialize)]
pub struct DeletionLockingTransportProperties {
    #[serde(rename = "del:trkorr")]
    pub transport_number: TransportNumber,
    #[serde(rename = "del:owner")]
    pub owner: User,
    #[serde(rename = "del:description")]
    pub description: String,
}

/// A message returned by the CTS transport check.
#[derive(Debug, Deserialize)]
pub struct DeletionTransportMessage {
    #[serde(rename = "del:severity")]
    pub severity: String,

    #[serde(rename = "del:text")]
    pub text: String,
}

/// Transport locking a checked object
#[derive(Debug, Deserialize)]
pub struct DeletionMessage {
    #[serde(rename = "@del:priority")]
    pub priority: i32,
    #[serde(rename = "@del:type")]
    pub message_type: String,
    #[serde(rename = "del:text")]
    pub text: String,

    /// Related resources such as a message long text.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtResponse, Class};
    use http::{HeaderMap, header};

    const CHECK_RESPONSE: &str = r#"<?xml version="1.0" encoding="UTF-8"?><del:checkResponse xmlns:del="http://www.sap.com/adt/deletion">
  <del:object xmlns:adtcore="http://www.sap.com/adt/core" del:externalStrongReferences="0" del:externalWeakReferences="0" del:isDeletable="true" adtcore:uri="/sap/bc/adt/functions/groups/zgroup123/includes/lzgroup123rrr" adtcore:type="FUGR/I" adtcore:name="LZGROUP123RRR" adtcore:packageName="$TMP">
    <del:lockingTransport>
      <del:recording>false</del:recording>
      <del:result/>
      <del:lockingTransport>
        <del:trkorr/>
        <del:owner/>
        <del:description/>
      </del:lockingTransport>
      <del:transportLayer/>
    </del:lockingTransport>
    <del:message del:priority="0" del:type="W">
      <del:text>LZGROUP123RRR does not exist</del:text>
    </del:message>
  </del:object>
</del:checkResponse>"#;

    const CHECK_RESPONSE_WITH_TRANSPORT: &str = r#"<?xml version="1.0" encoding="UTF-8"?><del:checkResponse xmlns:del="http://www.sap.com/adt/deletion">
  <del:object xmlns:adtcore="http://www.sap.com/adt/core" del:externalStrongReferences="0" del:externalWeakReferences="0" del:isDeletable="true" adtcore:uri="/sap/bc/adt/oo/classes/zmyclass" adtcore:type="CLAS/OC" adtcore:name="ZMYCLASS" adtcore:packageName="ZZZMYPACKAGE">
    <del:lockingTransport>
      <del:recording>false</del:recording>
      <del:result/>
      <del:lockingTransport>
        <del:trkorr>A4HK900148</del:trkorr>
        <del:owner>KD</del:owner>
        <del:description>DEVELOPER   20260902145154aaa</del:description>
      </del:lockingTransport>
      <del:transportLayer/>
    </del:lockingTransport>
    <del:message del:priority="0" del:type="S">
      <del:text/>
    </del:message>
  </del:object>
</del:checkResponse>"#;

    const CHECK_RESPONSE_WITH_RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><del:checkResponse xmlns:del="http://www.sap.com/adt/deletion">
  <del:object xmlns:adtcore="http://www.sap.com/adt/core" del:externalStrongReferences="0" del:externalWeakReferences="0" del:isDeletable="true" adtcore:uri="/sap/bc/adt/oo/classes/zparent" adtcore:type="CLAS/OC" adtcore:name="ZPARENT" adtcore:packageName="ZPACKAGE">
    <del:lockUser>LOCK_OWNER</del:lockUser>
    <del:lockingTransport>
      <del:recording>true</del:recording>
      <del:result/>
      <del:message><del:severity>W</del:severity><del:text>Transport warning</del:text></del:message>
      <del:lockingTransport><del:trkorr/><del:owner/><del:description/></del:lockingTransport>
      <del:transportLayer>ZDEV</del:transportLayer>
    </del:lockingTransport>
    <del:message del:priority="1" del:type="W">
      <del:text>Related long text</del:text>
      <atom:link xmlns:atom="http://www.w3.org/2005/Atom" href="/sap/bc/adt/longtexts/1" rel="alternate" type="text/html"/>
    </del:message>
    <del:usage del:degree="weak" del:isExternal="false" adtcore:uri="/sap/bc/adt/oo/classes/zchild" adtcore:type="CLAS/OC" adtcore:name="ZCHILD" adtcore:packageName="ZPACKAGE"/>
  </del:object>
</del:checkResponse>"#;

    const CHECK_RESPONSE_WITH_INCLUDED_OBJECT: &str = r#"<?xml version="1.0" encoding="UTF-8"?><del:checkResponse xmlns:del="http://www.sap.com/adt/deletion">
  <del:object xmlns:adtcore="http://www.sap.com/adt/core" del:externalStrongReferences="0" del:externalWeakReferences="0" del:isDeletable="true" adtcore:uri="/sap/bc/adt/functions/groups/zgroup123" adtcore:type="FUGR/F" adtcore:name="ZGROUP123" adtcore:packageName="$TMP">
    <del:lockingTransport>
      <del:recording>false</del:recording>
      <del:result/>
      <del:lockingTransport>
        <del:trkorr/>
        <del:owner/>
        <del:description/>
      </del:lockingTransport>
      <del:transportLayer/>
    </del:lockingTransport>
    <del:message del:priority="0" del:type="S">
      <del:text/>
    </del:message>
    <del:includedObject del:canBeDeletedWithoutParent="true" del:mustBeDeletedWithParent="true" del:isDeletable="true" adtcore:uri="/sap/bc/adt/functions/groups/zgroup123/fmodules/zftftr" adtcore:type="FUGR/FF" adtcore:name="ZFTFTR" adtcore:packageName="$TMP">
      <del:lockingTransport>
        <del:recording>false</del:recording>
        <del:result/>
        <del:lockingTransport>
          <del:trkorr/>
          <del:owner/>
          <del:description/>
        </del:lockingTransport>
        <del:transportLayer/>
      </del:lockingTransport>
      <del:message del:priority="0" del:type="W">
        <del:text>ZFTFTR does not exist</del:text>
      </del:message>
    </del:includedObject>
  </del:object>
</del:checkResponse>"#;

    const DELETE_RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?><del:deletionResult xmlns:del="http://www.sap.com/adt/deletion"><del:object del:isDeleted="true" adtcore:uri="/sap/bc/adt/oo/classes/zmyclass" adtcore:type="CLAS/OC" adtcore:name="ZMYCLASS" adtcore:packageName="ZZZMYPACKAGE" xmlns:adtcore="http://www.sap.com/adt/core"><del:message del:priority="0" del:type="S"><del:text/></del:message></del:object></del:deletionResult>"#;

    const DELETE_FAILURE_RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?><del:deletionResult xmlns:del="http://www.sap.com/adt/deletion"><del:object del:isDeleted="false" adtcore:uri="/sap/bc/adt/oo/classes/zmissing" adtcore:type="CLAS/OC" adtcore:name="ZMISSING" xmlns:adtcore="http://www.sap.com/adt/core"><del:message del:priority="0" del:type="E"><del:text>Object does not exist</del:text></del:message></del:object></del:deletionResult>"#;

    fn class_reference(name: &str) -> ObjectRef<Class> {
        ObjectRef::new(
            name.to_owned(),
            AdtUri::parse(&format!(
                "/sap/bc/adt/oo/classes/{}",
                name.to_ascii_lowercase()
            ))
            .unwrap(),
        )
    }

    fn operation_response(content_type: &'static str, body: &str) -> OperationResponse {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
        OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, body.as_bytes().to_vec()),
            AdtUri::parse("/sap/bc/adt/deletion").unwrap(),
        )
    }

    #[test]
    fn deletion_check_encodes_the_bulk_request_contract() {
        let reference = class_reference("ZMYCLASS");
        let mut check = DeletionCheck::new();
        check
            .push_object(&reference)
            .push_object(DeletionCheckObject::new(&reference).lock_handle("DELETION-LOCK"));

        let request = check.encode().unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            DeletionCheckRequest::MEDIA_TYPE
        );
        assert_eq!(
            request.headers()[header::ACCEPT],
            DeletionCheckResponse::MEDIA_TYPE
        );
        let body = std::str::from_utf8(request.body()).unwrap();
        assert!(body.contains("<del:checkRequest"));
        assert_eq!(body.matches("<del:object ").count(), 2);
        assert!(body.contains("adtcore:uri=\"/sap/bc/adt/oo/classes/zmyclass\""));
        assert!(body.contains("<del:lockHandle>DELETION-LOCK</del:lockHandle>"));
    }

    #[test]
    fn object_deletion_encodes_local_and_transported_objects() {
        let local = class_reference("ZLOCAL");
        let transported = class_reference("ZTRANSPORTED");
        let mut deletion = local.deletion();
        deletion.push_object(DeletionObject::new(&transported).transport("A4HK900148"));

        let request = deletion.encode().unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            DeletionRequest::MEDIA_TYPE
        );
        assert_eq!(
            request.headers()[header::ACCEPT],
            DeletionResult::MEDIA_TYPE
        );
        let body = std::str::from_utf8(request.body()).unwrap();
        assert!(body.contains("<del:deletionRequest"));
        assert!(body.contains("adtcore:uri=\"/sap/bc/adt/oo/classes/zlocal\""));
        assert!(body.contains("adtcore:uri=\"/sap/bc/adt/oo/classes/ztransported\""));
        assert!(body.contains("<del:transportNumber></del:transportNumber>"));
        assert!(body.contains("<del:transportNumber>A4HK900148</del:transportNumber>"));
    }

    #[test]
    fn deserializes_a_deletion_check_response() {
        let response: DeletionCheckResponse = serde_xml_rs::from_str(CHECK_RESPONSE).unwrap();

        assert_eq!(response.objects.len(), 1);
        let object = &response.objects[0];
        assert_eq!(object.external_strong_references, 0);
        assert_eq!(object.external_weak_references, 0);
        assert!(object.is_deletable);
        assert_eq!(
            object.uri.as_str(),
            "/sap/bc/adt/functions/groups/zgroup123/includes/lzgroup123rrr"
        );
        assert_eq!(object.object_type.as_str(), "FUGR/I");
        assert_eq!(object.name, "LZGROUP123RRR");
        assert_eq!(object.package_name.as_deref(), Some("$TMP"));

        let locking_transport = &object.locking_transport;
        assert!(!locking_transport.recording);
        assert!(locking_transport.result.is_empty());
        assert!(
            locking_transport
                .properties
                .transport_number
                .as_str()
                .is_empty()
        );
        assert!(locking_transport.properties.owner.as_str().is_empty());
        assert!(locking_transport.properties.description.is_empty());
        assert!(locking_transport.transport_layer.is_empty());

        assert_eq!(object.messages.len(), 1);
        let message = &object.messages[0];
        assert_eq!(message.priority, 0);
        assert_eq!(message.message_type, "W");
        assert_eq!(message.text, "LZGROUP123RRR does not exist");
    }

    #[test]
    fn deserializes_populated_deletion_transport_metadata() {
        let response: DeletionCheckResponse =
            serde_xml_rs::from_str(CHECK_RESPONSE_WITH_TRANSPORT).unwrap();

        assert_eq!(response.objects.len(), 1);
        let object = &response.objects[0];
        assert_eq!(object.uri.as_str(), "/sap/bc/adt/oo/classes/zmyclass");
        assert_eq!(object.object_type.as_str(), "CLAS/OC");
        assert_eq!(object.name, "ZMYCLASS");
        assert_eq!(object.package_name.as_deref(), Some("ZZZMYPACKAGE"));

        let locking_transport = &object.locking_transport;
        assert!(!locking_transport.recording);
        assert_eq!(
            locking_transport.properties.transport_number.as_str(),
            "A4HK900148"
        );
        assert_eq!(locking_transport.properties.owner.as_str(), "KD");
        assert_eq!(
            locking_transport.properties.description,
            "DEVELOPER   20260902145154aaa"
        );
        assert!(locking_transport.transport_layer.is_empty());

        assert_eq!(object.messages.len(), 1);
        let message = &object.messages[0];
        assert_eq!(message.priority, 0);
        assert_eq!(message.message_type, "S");
        assert!(message.text.is_empty());
    }

    #[test]
    fn deserializes_deletion_relationships_and_extended_messages() {
        let response: DeletionCheckResponse =
            serde_xml_rs::from_str(CHECK_RESPONSE_WITH_RELATIONSHIPS).unwrap();

        let object = &response.objects[0];
        assert_eq!(object.lock_user.as_ref().unwrap().as_str(), "LOCK_OWNER");
        assert_eq!(object.locking_transport.messages.len(), 1);
        assert_eq!(
            object.locking_transport.messages[0].text,
            "Transport warning"
        );
        assert_eq!(object.messages[0].links.len(), 1);
        assert_eq!(
            object.messages[0].links[0].relation.as_deref(),
            Some("alternate")
        );

        assert_eq!(object.usages.len(), 1);
        assert_eq!(object.usages[0].degree, "weak");
        assert!(!object.usages[0].is_external);
        assert_eq!(object.usages[0].name, "ZCHILD");

        assert!(object.included_objects.is_empty());
    }

    #[test]
    fn deserializes_an_included_function_module() {
        let response: DeletionCheckResponse =
            serde_xml_rs::from_str(CHECK_RESPONSE_WITH_INCLUDED_OBJECT).unwrap();

        let object = &response.objects[0];
        assert_eq!(object.object_type.as_str(), "FUGR/F");
        assert_eq!(object.included_objects.len(), 1);
        let included = &object.included_objects[0];
        assert!(included.can_be_deleted_without_parent);
        assert!(included.must_be_deleted_with_parent);
        assert!(included.is_deletable);
        assert_eq!(included.object_type.as_str(), "FUGR/FF");
        assert_eq!(included.name, "ZFTFTR");
        assert_eq!(included.package_name.as_deref(), Some("$TMP"));
        assert_eq!(included.messages.len(), 1);
        assert_eq!(included.messages[0].message_type, "W");
        assert_eq!(included.messages[0].text, "ZFTFTR does not exist");
    }

    #[test]
    fn decodes_successful_and_failed_deletion_results() {
        let deletion = ObjectDeletion::new();
        let success = deletion
            .decode(operation_response(
                DeletionResult::MEDIA_TYPE,
                DELETE_RESPONSE,
            ))
            .unwrap();
        let failure = deletion
            .decode(operation_response(
                DeletionResult::MEDIA_TYPE,
                DELETE_FAILURE_RESPONSE,
            ))
            .unwrap();

        assert_eq!(success.objects.len(), 1);
        assert!(success.objects[0].is_deleted);
        assert_eq!(success.objects[0].name.as_deref(), Some("ZMYCLASS"));
        assert_eq!(
            success.objects[0].package_name.as_deref(),
            Some("ZZZMYPACKAGE")
        );
        assert_eq!(success.objects[0].messages[0].message_type, "S");
        assert!(success.objects[0].messages[0].text.is_empty());

        assert_eq!(failure.objects.len(), 1);
        assert!(!failure.objects[0].is_deleted);
        assert_eq!(failure.objects[0].messages[0].message_type, "E");
        assert_eq!(failure.objects[0].messages[0].text, "Object does not exist");
    }
}
