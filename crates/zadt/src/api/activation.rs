use std::collections::HashMap;

use http::{Method, StatusCode};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    AdtObject, AdtRequest, AdvertisedLink, AdvertisedObjectReference, CategoryId, Client,
    ObjectError, ObjectRef, Operation, OperationError, OperationResponse, Ready, ResponseError,
    Stateless,
    objects::ObjectReferences,
    target::{CollectionTarget, TemplateTarget},
};

const INACTIVE_OBJECTS: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/activation",
    term: "inactiveobjects",
};

const ACTIVATE_OBJECTS: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/activation",
    term: "activationruns",
};

const QUERY_RELATION: &str = "http://www.sap.com/adt/relations/activation/inactiveobjects";

#[derive(Debug, Eq, PartialEq)]
pub enum ActivationRunMode {
    /// Checks the objects without activating them.
    Check,
    /// Activates the objects.
    Activate,
}

impl ActivationRunMode {
    fn as_str(&self) -> &str {
        match self {
            Self::Activate => "activate",
            Self::Check => "check",
        }
    }
}

/// Activates or activation-checks the list of provided objects.
///
/// The object references use the same media type returned by [`InactiveObjectsQuery`].
///
/// The endpoint aborts activation when the passed objects are assigned to
/// different CTS transport requests. This conflict must then be explicitly
/// accepted by passing `preaudit=false`.
///
/// Backend handler: `CL_SEU_ADT_RES_ACTIVATION`
#[derive(Debug)]
pub struct ActivationRun {
    /// The mode of the run. `Check` does not actually activate any objects.
    mode: ActivationRunMode,
    /// Repository objects included in the activation worklist.
    objects: ObjectReferences,
    /// Whether activation should be forced. Off by default
    forced: Option<bool>,
    /// Whether a request preaudit should be performed. On by default
    preaudit: bool,
}

impl ActivationRun {
    const MEDIA_TYPE: &'static str = "application/xml";

    /// Creates an activation or check run for `objects`.
    pub fn new(mode: ActivationRunMode, objects: ObjectReferences) -> Self {
        Self {
            mode,
            objects,
            forced: None,
            preaudit: true,
        }
    }

    /// Allow distinct transport requests. This will prevent the preaudit
    /// from aborting the activation.
    pub fn allow_distinct_transports(&mut self, allowed: bool) -> &mut Self {
        self.preaudit = !allowed;
        self
    }

    /// Adds an object to the activation list
    pub fn push_object<T>(&mut self, object: &ObjectRef<T>) -> &mut Self
    where
        for<'a> &'a ObjectRef<T>: Into<AdvertisedObjectReference>,
    {
        self.objects.objects.push(object.into());
        self
    }

    /// Requests forced Workbench activation.
    ///
    /// This seems to promote the object to the active version even if activation
    /// fails. Only works when explicitly supported for an error.
    pub fn forced(&mut self, forced: bool) -> &mut Self {
        self.forced = Some(forced);
        self
    }
}

impl Operation<Ready> for ActivationRun {
    type Kind = Stateless;
    type Response = ActivationRunMessages;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let body = serde_xml_rs::SerdeXml::new()
            .namespace("adtcore", "http://www.sap.com/adt/core")
            .to_string(&self.objects)
            .map_err(ObjectError::InvalidRequest)?;
        let target = CollectionTarget::new(ACTIVATE_OBJECTS);
        let mut request = target.request(client, Method::POST)?;
        request.push_query("method", self.mode.as_str());
        request.push_query("preaudit", self.preaudit.to_string());
        if let Some(forced) = self.forced {
            request.push_query("forced", forced.to_string());
        }
        request.set_accept(Self::MEDIA_TYPE);
        request.set_content_type(Self::MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() != StatusCode::OK {
            return Err(ResponseError::unexpected_status(response.response()));
        }
        if response.body().is_empty() {
            return Ok(ActivationRunMessages::default());
        }

        let messages =
            serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;

        Ok(messages)
    }
}

impl<T> ObjectRef<T>
where
    for<'a> &'a ObjectRef<T>: Into<AdvertisedObjectReference>,
{
    /// Creates an activation run for this object.
    pub fn activation(&self) -> ActivationRun {
        ActivationRun::new(
            ActivationRunMode::Activate,
            ObjectReferences {
                objects: vec![self.into()],
            },
        )
    }
}

impl<P> AdtObject<P> {
    /// Creates an activation run for this loaded object.
    pub fn activation(&self) -> ActivationRun {
        self.reference().activation()
    }
}

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
        if response.status() != StatusCode::OK {
            return Err(ResponseError::unexpected_status(response.response()));
        }
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
        if response.status() != StatusCode::OK {
            return Err(ResponseError::unexpected_status(response.response()));
        }
        if response.body().is_empty() {
            return Ok(InactiveCtsObjects::default());
        }
        let objects =
            serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;

        Ok(objects)
    }
}

/// Messages about the result of an activation run. Mainly errors that blocked it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "chkl:messages")]
pub struct ActivationRunMessages {
    /// Execution phases reached by the backend.
    #[serde(rename = "chkl:properties")]
    pub properties: ActivationRunProperties,
    /// Diagnostics emitted by the activation run.
    #[serde(rename = "msg", default)]
    pub messages: Vec<ActivationRunMessage>,
}

/// Execution phases reached by an activation run.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "chkl:properties")]
pub struct ActivationRunProperties {
    /// Whether the object check phase ran.
    #[serde(rename = "@checkExecuted")]
    pub check_executed: bool,
    /// Whether the activation phase ran.
    #[serde(rename = "@activationExecuted")]
    pub activation_executed: bool,
    /// Whether the generation phase ran.
    #[serde(rename = "@generationExecuted")]
    pub generation_executed: bool,
}

/// One diagnostic emitted by an activation run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename = "msg")]
pub struct ActivationRunMessage {
    /// Backend-formatted description of the affected object or component.
    #[serde(rename = "@objDescr")]
    pub description: String,
    /// SAP message severity, such as `E` for an error.
    #[serde(rename = "@type")]
    pub message_type: String,
    /// Ordinal position of the diagnostic in the response.
    #[serde(rename = "@line")]
    pub line: i32,
    /// Raw source link, including any source-range fragment.
    #[serde(rename = "@href")]
    pub href: String,
    /// Raw backend value indicating whether forced activation is supported.
    #[serde(rename = "@forceSupported")]
    pub force_supported: String,
    /// Short diagnostic text lines.
    #[serde(rename = "shortText")]
    pub short_text: ActivationRunMessageText,
    /// Extended diagnostic text lines, when supplied.
    #[serde(rename = "longText", default)]
    pub long_text: Option<ActivationRunMessageText>,
    /// Advertised links, including quick-fix relations.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

/// A string table used for short and extended checklist text.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct ActivationRunMessageText {
    /// Human-readable text lines in backend order.
    #[serde(rename = "txt", default)]
    pub lines: Vec<String>,
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
    use crate::{AdtResponse, AdtUri, Class};
    use http::header;

    const REFERENCES_XML: &[u8] =
        include_bytes!("../../tests/fixtures/inactive-object-references.xml");
    const CTS_OBJECTS_XML: &[u8] = include_bytes!("../../tests/fixtures/inactive-cts-objects.xml");
    const ACTIVATION_MESSAGES_XML: &[u8] =
        include_bytes!("../../tests/fixtures/activation-run-messages.xml");
    const ACTIVATION_DISCOVERY_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
        <app:service xmlns:app="http://www.w3.org/2007/app"
                     xmlns:atom="http://www.w3.org/2005/Atom">
            <app:workspace>
                <atom:title>Activation</atom:title>
                <app:collection href="/sap/bc/adt/activation">
                    <atom:title>Activation Runs</atom:title>
                    <atom:category term="activationruns"
                        scheme="http://www.sap.com/adt/categories/activation"/>
                </app:collection>
            </app:workspace>
        </app:service>"#;

    struct UnusedTransport;

    #[async_trait::async_trait]
    impl crate::Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn activation_client() -> Client<Ready> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(ACTIVATION_DISCOVERY_XML).unwrap(),
            crate::api::discovery::parse_capabilities(ACTIVATION_DISCOVERY_XML).unwrap(),
        )
    }

    #[test]
    fn activation_run_posts_flags_and_namespaced_object_references() {
        let object = ObjectRef::<Class>::for_test(
            "Z_SYNTAX_TEST",
            AdtUri::parse("/sap/bc/adt/oo/classes/z_syntax_test").unwrap(),
        );
        let mut run = object.activation();
        run.allow_distinct_transports(true).forced(true);

        let request = run.request(&activation_client()).unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.target().as_str(), "/sap/bc/adt/activation");
        assert_eq!(
            request.query(),
            [
                ("method".to_owned(), "activate".to_owned()),
                ("preaudit".to_owned(), "false".to_owned()),
                ("forced".to_owned(), "true".to_owned()),
            ]
        );
        assert_eq!(request.headers()[header::ACCEPT], ActivationRun::MEDIA_TYPE);
        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            ActivationRun::MEDIA_TYPE
        );
        let body = std::str::from_utf8(request.body()).unwrap();
        assert!(body.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
        assert!(body.contains("adtcore:type=\"CLAS/OC\""));
        assert!(body.contains("adtcore:name=\"Z_SYNTAX_TEST\""));
    }

    #[test]
    fn parses_activation_run_messages() {
        let result: ActivationRunMessages =
            serde_xml_rs::from_reader(ACTIVATION_MESSAGES_XML).unwrap();

        assert_eq!(result.properties, ActivationRunProperties::default());
        assert_eq!(result.messages.len(), 18);

        let first = &result.messages[0];
        assert_eq!(first.description, "Class Z_SYNTAX_TEST, Public Section");
        assert_eq!(first.message_type, "E");
        assert_eq!(first.line, 1);
        assert_eq!(
            first.href,
            "/sap/bc/adt/oo/classes/z_syntax_test/source/main#start=207,19"
        );
        assert_eq!(first.force_supported, "true");
        assert_eq!(
            first.short_text.lines,
            ["Implementation missing for method \"CLS_METHODS_MULTIPLE1\"."]
        );
        assert!(first.long_text.is_none());
        assert_eq!(first.links.len(), 1);
        assert_eq!(first.links[0].href, "art.syntax:G(2");
        assert_eq!(
            first.links[0].relation.as_deref(),
            Some("http://www.sap.com/adt/categories/quickfixes")
        );

        let last = result.messages.last().unwrap();
        assert_eq!(last.line, 18);
        assert_eq!(
            last.short_text.lines,
            ["Implementation missing for method \"SINGLE_METHOD_USING_ESCAPE\"."]
        );
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
