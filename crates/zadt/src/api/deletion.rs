//! Represents the `/sap/bc/adt/deletion` workspace which contains
//! bulk object deletion and deletion check runs. Objects are not
//! deleted by sending a DELETE to the object uri.

use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    AdtUri, Advertised, CategoryId, EncodeError, EncodedOperation, GlobalWorkbenchType,
    ObjectError, ObjectRef, ObjectSnapshot, Operation, OperationResponse, ResponseError,
    SnapshotKind, Stateless, TransportNumber, User, operation::CollectionLocator,
};

/// Checks whether one or more repository objects can be deleted.
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
    /// Constructs a deletion check containing this object.
    pub fn deletion_check(&self) -> DeletionCheck {
        let mut check = DeletionCheck::new();
        check.push_object(self);
        check
    }
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Constructs a deletion check containing this object.
    pub fn deletion_check(&self) -> DeletionCheck {
        self.reference().deletion_check()
    }
}

/// Deletes one or more repository objects through the ADT deletion workspace.
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
    pub fn transport(mut self, transport: impl Into<String>) -> Self {
        self.transport_number = transport.into();
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
    #[serde(rename = "@del:externalStrongReferences")]
    pub external_strong_references: i32,
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
    #[serde(rename = "del:lockingTransport")]
    pub properties: DeletionLockingTransportProperties,
    #[serde(rename = "del:transportLayer")]
    pub transport_layer: String,
}

#[derive(Debug, Deserialize)]
pub struct DeletionLockingTransportProperties {
    #[serde(rename = "del:trkorr")]
    pub transport_number: TransportNumber,
    #[serde(rename = "del:owner")]
    pub owner: User,
    #[serde(rename = "del:description")]
    pub description: String,
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
        let mut deletion = ObjectDeletion::new();
        deletion
            .push_object(&local)
            .push_object(DeletionObject::new(&transported).transport("A4HK900148"));

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
