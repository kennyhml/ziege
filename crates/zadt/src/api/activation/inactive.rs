use std::collections::HashMap;

use http::{Method, StatusCode};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    AdtRequest, AdvertisedObjectReference, CategoryId, Client, ObjectError, Operation,
    OperationError, OperationResponse, Ready, ResponseError, Stateless,
    objects::ObjectReferences,
    target::{CollectionTarget, TemplateTarget},
};

const INACTIVE_OBJECTS: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/activation",
    term: "inactiveobjects",
};
const QUERY_RELATION: &str = "http://www.sap.com/adt/relations/activation/inactiveobjects";

/// Retrieves the inactive objects of the given user. If the user is omitted,
/// the user making the request is used instead.
///
/// This is the basic variant of the operation using `application/xml`, which simply
/// provides a list of object references of the inactive objects.
///
/// If more detail is needed (such as assigned transports), the [`InactiveCtsObjectsQuery`]
/// operation can be used instead. You can use the [`Self::with_transports`] method to
/// upgrade this request to the more detailed variant.
///
/// Backend handler: `CL_SEU_ADT_RES_INACTIVE`
#[derive(Debug, Default)]
pub struct InactiveObjectsQuery {
    username: Option<String>,
}

impl InactiveObjectsQuery {
    const MEDIA_TYPE: &'static str = "application/xml";

    pub fn new() -> Self {
        Self { username: None }
    }

    /// Restricts the query to inactive objects owned by `name`.
    pub fn username<T: Into<Option<String>>>(&mut self, name: T) -> &mut Self {
        self.username = name.into();
        self
    }

    /// Requests the detailed inactive CTS representation instead.
    pub fn with_transports(self) -> InactiveCtsObjectsQuery {
        InactiveCtsObjectsQuery { inner: self }
    }
}

impl Operation<Ready> for InactiveObjectsQuery {
    type Kind = Stateless;
    type Response = ObjectReferences;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        // Might as well use the template if we got the username even though it
        // does not provide much benefit over just using a query parameter
        let mut request = if let Some(username) = &self.username {
            let target = TemplateTarget::new(INACTIVE_OBJECTS, QUERY_RELATION);
            let variables = HashMap::from([("USERNAME".into(), Value::String(username.clone()))]);
            let (target, query) = target.template(client)?.expand(&variables)?;
            let mut request = AdtRequest::new(Method::GET, target);
            for (name, value) in query {
                request.push_query(name, value);
            }
            request
        } else {
            let target = CollectionTarget::new(INACTIVE_OBJECTS);
            target.request(client, Method::GET)?
        };
        request.set_accept(Self::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        if response.body().is_empty() {
            return Ok(ObjectReferences::default());
        }

        let objects =
            serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;

        Ok(objects)
    }
}

/// Retrieves the inactive objects of the given user. If the user is omitted,
/// the user making the request is used instead.
///
/// This is the enhanced, CTS variant of the operation. If you only need the object
/// references, consider using [`InactiveObjectsQuery`] for simplicity and less
/// transport overhead.
///
/// Backend handler: `CL_SEU_ADT_RES_INACTIVE`
#[derive(Debug, Default)]
pub struct InactiveCtsObjectsQuery {
    inner: InactiveObjectsQuery,
}

impl InactiveCtsObjectsQuery {
    const MEDIA_TYPE: &'static str = "application/vnd.sap.adt.inactivectsobjects.v1+xml";

    pub fn new() -> Self {
        Self {
            inner: InactiveObjectsQuery::new(),
        }
    }

    /// Restricts the query to inactive objects owned by `name`.
    pub fn username<T: Into<Option<String>>>(&mut self, name: T) -> &mut Self {
        self.inner.username = name.into();
        self
    }
}

impl Operation<Ready> for InactiveCtsObjectsQuery {
    type Kind = Stateless;
    type Response = InactiveCtsObjects;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let mut request = self.inner.request(client)?;
        request.set_accept(Self::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        if response.body().is_empty() {
            return Ok(InactiveCtsObjects::default());
        }
        let objects =
            serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;

        Ok(objects)
    }
}

/// Detailed inactive objects and transport associations returned by ADT.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "ioc:inactiveObjects")]
pub struct InactiveCtsObjects {
    /// Inactive object/transport slots in response order.
    #[serde(rename = "ioc:entry", default)]
    pub entries: Vec<InactiveCtsObjectEntry>,
}

/// One pair of inactive object and transport slots.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename = "ioc:entry")]
pub struct InactiveCtsObjectEntry {
    /// Inactive repository object information, or an empty slot.
    #[serde(rename = "ioc:object")]
    pub object: InactiveCtsObject,
    /// Associated transport information, or an empty slot.
    #[serde(rename = "ioc:transport")]
    pub transport: InactiveCtsObjectTransport,
}

/// An inactive object slot, which may be empty in the wire representation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct InactiveCtsObject {
    /// User owning the inactive object, when this slot is populated.
    #[serde(rename = "@ioc:user", default)]
    pub user: Option<String>,
    /// Whether the represented repository object has been deleted.
    #[serde(rename = "@ioc:deleted", default)]
    pub deleted: Option<bool>,
    /// The inactive repository object reference, when this slot is populated.
    #[serde(rename = "ioc:ref", default)]
    pub reference: Option<AdvertisedObjectReference>,
}

/// A CTS assignment slot, which may be empty in the wire representation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct InactiveCtsObjectTransport {
    /// User owning the transport assignment, when populated.
    #[serde(rename = "@ioc:user", default)]
    pub user: Option<String>,
    /// Whether the transport is linked to the paired inactive object.
    #[serde(rename = "@ioc:linked", default)]
    pub linked: Option<bool>,
    /// The transport request or task reference, when populated.
    #[serde(rename = "ioc:ref", default)]
    pub reference: Option<AdvertisedObjectReference>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCES_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/inactive-object-references.xml");
    const CTS_OBJECTS_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/inactive-cts-objects.xml");

    #[test]
    fn parses_inactive_object_references() {
        let references: ObjectReferences = serde_xml_rs::from_reader(REFERENCES_XML).unwrap();

        assert_eq!(references.objects.len(), 15);
        assert_eq!(references.objects[0].name.as_deref(), Some("ZTFRWTFRT"));
        let function = references
            .objects
            .iter()
            .find(|reference| reference.name.as_deref() == Some("ZZZZFUNC"))
            .unwrap();
        assert_eq!(
            function.parent_uri.as_deref(),
            Some("/sap/bc/adt/functions/groups/z_test_group")
        );
        assert_eq!(
            references.objects[4].uri.as_deref(),
            Some("/sap/bc/adt/oo/classes/%2fdmo%2fcl_travel_auxiliary")
        );
    }

    #[test]
    fn parses_inactive_cts_objects_and_empty_slots() {
        let objects: InactiveCtsObjects = serde_xml_rs::from_reader(CTS_OBJECTS_XML).unwrap();

        assert_eq!(objects.entries.len(), 6);
        assert!(objects.entries[0].object.reference.is_none());
        assert!(objects.entries[0].transport.reference.is_none());

        let transport = objects.entries[1].transport.reference.as_ref().unwrap();
        assert_eq!(transport.name.as_deref(), Some("A4HK900099"));
        assert_eq!(objects.entries[1].transport.linked, Some(false));

        let include = &objects.entries[3];
        assert_eq!(
            include
                .object
                .reference
                .as_ref()
                .and_then(|reference| reference.parent_uri.as_deref()),
            Some("/sap/bc/adt/oo/classes/%2fdmo%2ftfartfar")
        );
        assert_eq!(include.transport.linked, Some(true));
        assert_eq!(
            include
                .transport
                .reference
                .as_ref()
                .and_then(|reference| reference.parent_uri.as_deref()),
            Some("/sap/bc/adt/cts/transportrequests/A4HK900099")
        );

        let deleted = &objects.entries[4].object;
        assert_eq!(deleted.deleted, Some(true));
        assert!(
            deleted
                .reference
                .as_ref()
                .and_then(|reference| reference.uri.as_deref())
                .is_some_and(|uri| uri.contains("#type=CLAS%2FOM"))
        );
    }
}
