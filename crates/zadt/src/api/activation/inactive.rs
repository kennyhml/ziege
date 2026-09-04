use std::collections::HashMap;

use http::{Method, StatusCode};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    AdvertisedObjectReference, CategoryId, Discovery, EncodeError, EncodedOperation, ObjectError,
    Operation, OperationResponse, RequiresDiscovery, ResolveError, ResponseError, Stateless, User,
    objects::ObjectReferences, resource::AdtUriTemplate,
};

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
    username: Option<User>,
}

impl InactiveObjectsQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/activation",
        term: "inactiveobjects",
    };
    const RELATION: &str = "http://www.sap.com/adt/relations/activation/inactiveobjects";
    const MEDIA_TYPE: &'static str = "application/xml";

    pub fn new() -> Self {
        Self { username: None }
    }

    /// Restricts the query to inactive objects owned by `name`.
    pub fn username(&mut self, user: impl Into<User>) -> &mut Self {
        self.username = Some(user.into());
        self
    }

    /// Requests the detailed inactive CTS representation instead.
    pub fn with_transports(self) -> InactiveCtsObjectsQuery {
        InactiveCtsObjectsQuery { inner: self }
    }
}

impl Operation for InactiveObjectsQuery {
    type Kind = Stateless;
    type Response = ObjectReferences;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        // Might as well use the template if we got the username even though it
        // does not provide much benefit over just using a query parameter
        let mut request = if let Some(username) = &self.username {
            let link = resolver.require_template(Self::CATEGORY, Self::RELATION)?;
            let template = AdtUriTemplate::new(link.template());
            if !template.has_variable("USERNAME") {
                return Err(
                    ResolveError::from(ObjectError::UnsupportedTemplateParameter {
                        parameter: "USERNAME",
                    })
                    .into(),
                );
            }
            let variables = HashMap::from([(
                "USERNAME".to_owned(),
                Value::String(username.as_str().to_owned()),
            )]);
            let (target, query) = template.expand(&variables).map_err(ResolveError::from)?;
            let mut request = EncodedOperation::new(Method::GET, target);
            for (name, value) in query {
                request.push_query(name, value);
            }
            request
        } else {
            let target = resolver.require_collection_target(Self::CATEGORY)?;
            EncodedOperation::new(Method::GET, target)
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

impl User {
    /// Creates a query for this user's inactive repository objects.
    pub fn inactive_objects(&self) -> InactiveObjectsQuery {
        let mut query = InactiveObjectsQuery::new();
        query.username(self);
        query
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
    pub fn username(&mut self, user: impl Into<User>) -> &mut Self {
        self.inner.username = Some(user.into());
        self
    }
}

impl Operation for InactiveCtsObjectsQuery {
    type Kind = Stateless;
    type Response = InactiveCtsObjects;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let mut request = self.inner.encode(resolver)?;
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
    pub user: Option<User>,
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
    pub user: Option<User>,
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
    use crate::{AdtRequest, AdtResponse, Client, TransportError};
    use async_trait::async_trait;
    use http::header;

    const DISCOVERY_XML: &[u8] = br#"
        <app:service xmlns:app="http://www.w3.org/2007/app"
                xmlns:atom="http://www.w3.org/2005/Atom"
                xmlns:adtcomp="http://www.sap.com/adt/compatibility">
            <app:workspace>
                <atom:title>Activation</atom:title>
                <app:collection href="/sap/bc/adt/activation/inactiveobjects">
                    <atom:category scheme="http://www.sap.com/adt/categories/activation"
                        term="inactiveobjects" />
                    <adtcomp:templateLinks>
                        <adtcomp:templateLink
                            rel="http://www.sap.com/adt/relations/activation/inactiveobjects"
                            template="/sap/bc/adt/activation/inactiveobjects{?USERNAME}" />
                    </adtcomp:templateLinks>
                </app:collection>
            </app:workspace>
        </app:service>
    "#;

    const REFERENCES_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/inactive-object-references.xml");
    const CTS_OBJECTS_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/inactive-cts-objects.xml");

    struct UnusedTransport;

    #[async_trait]
    impl crate::Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn discovered_client() -> Client<Discovery> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
        )
    }

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
        assert_eq!(
            objects.entries[1].transport.user.as_ref().map(User::as_str),
            Some("DEVELOPER")
        );
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

    #[test]
    fn user_creates_an_inactive_objects_query_with_preserved_identity() {
        let client = discovered_client();
        let user = User::new("DEVELOPER");
        let query = user.inactive_objects().with_transports();

        assert_eq!(
            query.inner.username.as_ref().map(User::as_str),
            Some("DEVELOPER")
        );
        let request = query.encode(client.discovery()).unwrap();
        assert_eq!(
            request.target().as_str(),
            "/sap/bc/adt/activation/inactiveobjects"
        );
        assert_eq!(
            request.query(),
            [("USERNAME".to_owned(), "DEVELOPER".to_owned())]
        );
        assert_eq!(
            request.headers()[header::ACCEPT],
            InactiveCtsObjectsQuery::MEDIA_TYPE
        );
    }
}
