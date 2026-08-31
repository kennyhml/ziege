use http::{Method, StatusCode};

use crate::{
    Advertised, CategoryId, ErasedObject, Object, ObjectError, ObjectLock, TransportNumber,
    compatibility::matching_media_type,
    error::{EncodeError, ResponseError},
    objects::{
        AssignObjectIdentity, Create, ErasedProperties, MediaTyped, ObjectIdentity, ObjectRef,
        ObjectType, ObjectVersion, ToXml, XmlConversion,
    },
    operation::{
        CollectionTarget, EncodedOperation, IfNoneMatch, Operation, OperationResponse, Owned,
        Stateful, Stateless,
    },
    protocol::EntityTag,
};

use super::{locking::LOCK_HANDLE_QUERY, transports::TRANSPORT_REQUEST_QUERY};

/// Creates a repository object from a family-specific creation payload.
///
/// Successful responses without a representation decode to `None`. Object
/// families that return their properties decode to a loaded object.
///
/// The operation supports both typed and generic JSON payloads. In a typed
/// context, the response is also typed. Otherwise, the descriptor retains
/// the concrete response properties behind an [`ErasedObject`].
#[derive(Debug)]
pub struct CreateObjectRequest<T, P> {
    reference: ObjectRef<T>,
    payload: P,
    create_media_types: &'static [&'static str],
    response_media_types: &'static [&'static str],
    transport_request: Option<TransportNumber>,
}

impl<T, P> CreateObjectRequest<T, P> {
    /// Records the creation in the supplied transport request.
    pub fn transport(&mut self, transport_request: TransportNumber) -> &mut Self {
        self.transport_request = Some(transport_request);
        self
    }

    fn build_request(
        &self,
        object_category: CategoryId,
        body: Vec<u8>,
    ) -> Result<EncodedOperation<Advertised>, EncodeError> {
        let mut target = CollectionTarget::new(object_category).target();
        target.require_accepted_media_types(self.create_media_types);
        let mut request = EncodedOperation::advertised(Method::POST, target);
        request.set_accepts(self.response_media_types);
        request.set_body(body);
        if let Some(transport) = &self.transport_request {
            request.push_query(TRANSPORT_REQUEST_QUERY, transport.as_str());
        }
        Ok(request)
    }
}

impl<T, P> Operation for CreateObjectRequest<T, P>
where
    T: Create<Payload = P>,
    P: ToXml + Send + Sync,
{
    type Kind = Stateless;
    type Response = Option<Object<T>>;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        self.build_request(T::CATEGORY, self.payload.to_xml()?)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }
        Object::decode_properties(&self.reference, response).map(Some)
    }
}

impl Operation for CreateObjectRequest<(), serde_json::Value> {
    type Kind = Stateless;
    type Response = Option<ErasedObject>;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let descriptor = self.reference.require_descriptor()?;
        self.build_request(
            descriptor
                .category()
                .ok_or_else(|| ObjectError::ParentObjectRequired {
                    object_type: self.reference.object_type().clone(),
                })?,
            descriptor.creation_payload_to_xml(&self.reference, self.payload.clone())?,
        )
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }
        ErasedObject::decode_properties(&self.reference, response).map(Some)
    }
}

impl<T> ObjectRef<T>
where
    T: Create,
{
    /// Creates a typed object-creation request.
    pub fn create(&self, mut payload: T::Payload) -> CreateObjectRequest<T, T::Payload> {
        payload.assign_identity(self);
        CreateObjectRequest {
            reference: self.clone(),
            payload,
            transport_request: None,
            create_media_types: T::CREATE_MEDIA_TYPES,
            response_media_types: T::Properties::MEDIA_TYPES,
        }
    }
}

impl ObjectRef<()> {
    /// Creates a runtime object-creation request from its JSON payload.
    pub fn create(
        &self,
        payload: serde_json::Value,
    ) -> Result<CreateObjectRequest<(), serde_json::Value>, ObjectError> {
        let descriptor = self
            .descriptor()
            .ok_or_else(|| ObjectError::UnsupportedObjectType {
                object_type: self.object_type().clone(),
            })?;
        let create_media_types =
            descriptor
                .create_media_types()
                .ok_or_else(|| ObjectError::UnsupportedCapability {
                    object_type: self.object_type().clone(),
                    capability: "object creation",
                })?;

        Ok(CreateObjectRequest {
            reference: self.clone(),
            payload,
            transport_request: None,
            create_media_types,
            response_media_types: descriptor.properties_media_types(),
        })
    }
}

/// Fetches a versioned object-properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesQuery<T> {
    pub resource: ObjectRef<T>,
    pub workbench_version: Option<ObjectVersion>,
}

impl<T> ObjectPropertiesQuery<T> {
    fn build_request(&self, media_types: &'static [&'static str]) -> EncodedOperation<Owned> {
        let mut request = EncodedOperation::owned(Method::GET, self.resource.uri().clone());
        if let Some(version) = self.workbench_version {
            request.push_query(ObjectVersion::QUERY_PARAMETER, version.as_str());
        }
        request.set_accepts(media_types);
        request.set_cache_revalidation(None);
        request
    }

    pub fn workbench_version(mut self, version: ObjectVersion) -> Self {
        self.workbench_version = Some(version);
        self
    }

    pub fn if_none_match(self, etag: EntityTag) -> IfNoneMatch<Self> {
        IfNoneMatch::new(self, etag)
    }
}

impl<T> ObjectPropertiesQuery<T>
where
    T: ObjectType,
{
    pub fn new(resource: ObjectRef<T>) -> Self {
        Self {
            resource,
            workbench_version: None,
        }
    }
}

impl<T> Operation for ObjectPropertiesQuery<T>
where
    T: ObjectType,
{
    type Response = Object<T>;
    type Kind = Stateless;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        Ok(self.build_request(T::Properties::MEDIA_TYPES))
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        Object::decode_properties(&self.resource, response)
    }
}

impl<T> ObjectRef<T>
where
    T: ObjectType,
{
    pub fn query(&self) -> ObjectPropertiesQuery<T> {
        ObjectPropertiesQuery::new(self.clone())
    }
}

impl<T: ObjectType> Object<T> {
    /// Creates a conditional query using this representation's entity tag.
    pub fn revalidate(&self) -> Option<IfNoneMatch<ObjectPropertiesQuery<T>>> {
        self.etag()
            .cloned()
            .map(|etag| self.reference().query().if_none_match(etag))
    }

    fn decode_properties(
        resource: &ObjectRef<T>,
        response: OperationResponse,
    ) -> Result<Self, ResponseError> {
        if response.status() == StatusCode::NOT_MODIFIED {
            return Err(ResponseError::UnexpectedNotModified);
        }
        response.require_success()?;
        let supported = T::Properties::MEDIA_TYPES;
        let content_type = response.require_content_type(supported)?;
        let media_type = matching_media_type(supported, content_type)
            .expect("validated properties Content-Type must match a supported media type");
        let etag = response.entity_tag();
        let properties = T::Properties::from_xml(response.body())?;
        properties.validate_for(resource)?;
        Ok(Self::new(resource.clone(), media_type, etag, properties))
    }
}

impl Operation for ObjectPropertiesQuery<()> {
    type Response = ErasedObject;
    type Kind = Stateless;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let descriptor = self
            .resource
            .descriptor()
            .ok_or_else(|| self.resource.unsupported_capability("object properties"))?;
        Ok(self.build_request(descriptor.properties_media_types()))
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ErasedObject::decode_properties(&self.resource, response)
    }
}

impl ObjectRef<()> {
    pub fn query(&self) -> Result<ObjectPropertiesQuery<()>, ObjectError> {
        self.descriptor()
            .ok_or_else(|| self.unsupported_capability("object properties"))?;
        Ok(ObjectPropertiesQuery {
            resource: self.clone(),
            workbench_version: None,
        })
    }
}

/// Replaces an object's properties representation.
///
/// Successful execution returns a new loaded object. A response representation
/// is decoded when ADT supplies one; an empty success response retains the
/// submitted properties and any entity tag returned in the response headers.
#[derive(Debug)]
pub struct ObjectPropertiesUpdate<T> {
    resource: ObjectRef<T>,
    object_lock: ObjectLock,
    media_type: &'static str,
    properties: ErasedProperties,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
}

impl<T> ObjectPropertiesUpdate<T> {
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }

    fn build_request(&self) -> EncodedOperation<Owned> {
        let mut request = EncodedOperation::owned(Method::PUT, self.resource.uri().clone());
        request.push_query(LOCK_HANDLE_QUERY, self.object_lock.handle());
        if let Some(transport_request) = &self.transport_request {
            request.push_query(TRANSPORT_REQUEST_QUERY, transport_request.as_str());
        }
        if let Some(user_session) = self.object_lock.user_session() {
            request.bind_user_session(user_session);
        }
        request.set_accept(self.media_type);
        request.set_content_type(self.media_type);
        request.set_body(self.body.clone());
        request
    }
}

impl<T> Operation for ObjectPropertiesUpdate<T>
where
    T: ObjectType,
{
    type Response = Object<T>;
    type Kind = Stateful;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        Ok(self.build_request())
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            let properties = self
                .properties
                .downcast_ref::<T::Properties>()
                .expect("typed property updates retain their concrete property type")
                .clone();
            return Ok(Object::new(
                self.resource.clone(),
                self.media_type,
                response.entity_tag(),
                properties,
            ));
        }
        Object::decode_properties(&self.resource, response)
    }
}

impl<T: ObjectType> Object<T> {
    /// Creates an update that replaces this loaded representation's properties.
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: T::Properties,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        object_lock.validate_modification_for(self.reference())?;
        properties.validate_for(self.reference())?;
        let media_type = matching_media_type(T::Properties::MEDIA_TYPES, self.media_type())
            .expect("typed ADT objects carry a supported media type");
        let body = properties.to_xml()?;
        Ok(ObjectPropertiesUpdate {
            resource: self.reference().clone(),
            object_lock: object_lock.clone(),
            media_type,
            properties: std::sync::Arc::new(properties),
            body,
            transport_request: object_lock.transport_request().cloned(),
        })
    }
}

impl ErasedObject {
    /// Creates an update that replaces this loaded representation's properties.
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: serde_json::Value,
    ) -> Result<ObjectPropertiesUpdate<()>, ObjectError> {
        object_lock.validate_modification_for(self.reference())?;
        let descriptor = self.reference().require_descriptor()?;
        let media_type = descriptor
            .properties_media_type(self.media_type())
            .ok_or_else(|| ObjectError::UnsupportedPropertiesMediaType {
                object_type: self.reference().object_type().clone(),
                media_type: self.media_type().to_owned(),
            })?;
        let properties = descriptor.properties_from_json(self.reference(), properties)?;
        let body = descriptor.properties_to_xml(self.reference(), &properties)?;
        Ok(ObjectPropertiesUpdate {
            resource: self.reference().clone(),
            object_lock: object_lock.clone(),
            media_type,
            properties,
            body,
            transport_request: object_lock.transport_request().cloned(),
        })
    }
}

impl Operation for ObjectPropertiesUpdate<()> {
    type Response = ErasedObject;
    type Kind = Stateful;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        Ok(self.build_request())
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(ErasedObject::new(
                self.resource.clone(),
                self.media_type,
                response.entity_tag(),
                self.properties.clone(),
            ));
        }
        ErasedObject::decode_properties(&self.resource, response)
    }
}

impl ErasedObject {
    fn decode_properties(
        resource: &ObjectRef<()>,
        response: OperationResponse,
    ) -> Result<Self, ResponseError> {
        if response.status() == StatusCode::NOT_MODIFIED {
            return Err(ResponseError::UnexpectedNotModified);
        }
        response.require_success()?;
        let descriptor = resource
            .descriptor()
            .ok_or_else(|| resource.unsupported_capability("object properties"))?;
        let supported = descriptor.properties_media_types();
        let content_type = response.require_content_type(supported)?;
        let media_type = descriptor
            .properties_media_type(content_type)
            .expect("validated properties Content-Type must match a supported media type");
        let etag = response.entity_tag();
        let properties = descriptor.properties_from_xml(resource, response.body())?;
        Ok(Self::new(resource.clone(), media_type, etag, properties))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use super::*;
    use crate::{
        AbapLanguageVersion, AdtRequest, AdtResponse, AdtUri, AdvertisedObjectReference, Class,
        ClassCategory, ClassCreateProperties, ClassProperties, ClassTemplate, Client,
        CompatibilityError, ObjectType, Package, Ready, Resolve, Transport,
    };

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");
    const CLASS_XML: &[u8] = include_bytes!("../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn ready_client(xml: &[u8]) -> Client<Ready> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(xml).unwrap(),
            crate::api::discovery::parse_capabilities(xml).unwrap(),
        )
    }

    fn reference(name: &str) -> ObjectRef<Class> {
        ObjectRef::new(
            name.to_owned(),
            AdtUri::parse(&format!(
                "/sap/bc/adt/oo/classes/{}",
                name.to_ascii_lowercase()
            ))
            .unwrap(),
        )
    }

    fn create_properties() -> ClassCreateProperties {
        ClassCreateProperties::builder()
            .description("Created class")
            .package(AdvertisedObjectReference {
                name: Some("$TMP".to_owned()),
                ..Default::default()
            })
            .build()
            .unwrap()
    }

    #[test]
    fn typed_and_runtime_creation_build_the_same_class_request() {
        let reference = reference("ZZZTEST");
        let properties = create_properties();
        let typed = reference.create(properties.clone());
        let mut runtime_payload = serde_json::to_value(properties).unwrap();
        let runtime_values = runtime_payload.as_object_mut().unwrap();
        runtime_values.remove("@adtcore:name");
        runtime_values.remove("@adtcore:type");
        let runtime = reference.erase().create(runtime_payload).unwrap();

        let typed_request = typed.encode().unwrap();
        let runtime_request = runtime.encode().unwrap();

        assert_eq!(typed_request.method(), Method::POST);
        assert_eq!(
            typed_request.headers()[header::ACCEPT],
            ClassProperties::MEDIA_TYPES.join(", ")
        );
        assert!(!typed_request.headers().contains_key(header::CONTENT_TYPE));
        assert_eq!(typed_request.body(), runtime_request.body());
        let body = std::str::from_utf8(typed_request.body()).unwrap();
        assert!(body.contains("<class:abapClass"));
        assert!(body.contains("adtcore:name=\"ZZZTEST\""));
        assert!(body.contains("class:final=\"true\""));
        assert!(body.contains("class:visibility=\"public\""));
        assert!(body.contains("class:includeType=\"testclasses\""));
        assert!(body.contains("<adtcore:packageRef adtcore:name=\"$TMP\""));
        assert!(body.contains("<class:superClassRef"));
        assert!(!body.contains("abapsource:sourceUri"));
        assert!(!body.contains("adtcore:changedAt"));
        assert!(!body.contains("adtcore:createdAt"));
        assert!(!body.contains("atom:link"));
        assert!(!body.contains("adtcore:language="));
        assert!(!body.contains("class:category"));
        assert!(!body.contains("adtcore:masterLanguage"));
        assert!(!body.contains("adtcore:masterSystem"));
        assert!(!body.contains("adtcore:responsible"));
    }

    #[test]
    fn creation_negotiates_the_preferred_advertised_media_type() {
        let client = ready_client(DISCOVERY_XML);
        let typed = reference("ZZZTEST").create(create_properties());
        let runtime = reference("ZZZTEST")
            .erase()
            .create(serde_json::to_value(create_properties()).unwrap())
            .unwrap();

        let typed = client.resolve(typed.encode().unwrap()).unwrap();
        let runtime = client.resolve(runtime.encode().unwrap()).unwrap();

        assert_eq!(
            typed.request().headers()[header::CONTENT_TYPE],
            ClassProperties::MEDIA_TYPES[0]
        );
        assert_eq!(
            runtime.request().headers()[header::CONTENT_TYPE],
            ClassProperties::MEDIA_TYPES[0]
        );
        assert_eq!(typed.request().target().as_str(), "/sap/bc/adt/oo/classes");
    }

    #[test]
    fn creation_rejects_a_collection_without_an_accepted_media_type() {
        let mut discovery = String::from_utf8(DISCOVERY_XML.to_vec()).unwrap();
        for media_type in ClassProperties::MEDIA_TYPES {
            discovery = discovery.replace(&format!("<app:accept>{media_type}</app:accept>"), "");
        }
        let client = ready_client(discovery.as_bytes());
        let operation = reference("ZZZTEST").create(create_properties());

        let Err(error) = client.resolve(operation.encode().unwrap()) else {
            panic!("creation resolution should reject a collection without app:accept")
        };

        match error {
            crate::ResolveError::Compatibility(CompatibilityError::NoCompatibleMediaType {
                supported,
                accepted,
            }) => {
                assert_eq!(
                    supported,
                    Class::CREATE_MEDIA_TYPES
                        .iter()
                        .map(|media_type| (*media_type).to_owned())
                        .collect::<Vec<_>>()
                );
                assert!(accepted.is_empty());
            }
            error => panic!("unexpected resolution error: {error}"),
        }
    }

    #[test]
    fn creation_does_not_relabel_a_newer_payload_as_an_older_media_type() {
        let discovery = String::from_utf8(DISCOVERY_XML.to_vec())
            .unwrap()
            .replace(
                "<app:accept>application/vnd.sap.adt.oo.classes.v4+xml</app:accept>",
                "",
            )
            .replace(
                "<app:accept>application/vnd.sap.adt.oo.classes.v3+xml</app:accept>",
                "",
            );
        let client = ready_client(discovery.as_bytes());
        let operation = reference("ZZZTEST").create(create_properties());

        let Err(error) = client.resolve(operation.encode().unwrap()) else {
            panic!("a V4 creation payload must not be advertised as V2")
        };

        assert!(matches!(
            error,
            crate::ResolveError::Compatibility(CompatibilityError::NoCompatibleMediaType {
                ref supported,
                ref accepted,
            }) if supported == Class::CREATE_MEDIA_TYPES
                && accepted == &[ClassProperties::MEDIA_TYPES[2].to_owned()]
        ));
    }

    #[test]
    fn runtime_creation_applies_the_typed_builder_defaults() {
        let reference = reference("ZZZTEST");
        let typed = reference.create(create_properties());
        let runtime = reference
            .erase()
            .create(serde_json::json!({
                "@adtcore:description": "Created class",
                "adtcore:packageRef": {
                    "@adtcore:name": "$TMP"
                }
            }))
            .unwrap();

        let typed_request = typed.encode().unwrap();
        let runtime_request = runtime.encode().unwrap();

        assert_eq!(runtime_request.body(), typed_request.body());
    }

    #[test]
    fn creation_uses_the_reference_identity() {
        let mut properties = create_properties();
        properties.name = "ZOTHER".to_owned();
        properties.object_type = Package::WORKBENCH_TYPE;
        let operation = reference("ZZZTEST").create(properties);
        let request = operation.encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("adtcore:name=\"ZZZTEST\""));
        assert!(body.contains("adtcore:type=\"CLAS/OC\""));
        assert!(!body.contains("ZOTHER"));
    }

    #[test]
    fn creation_serializes_optional_class_settings() {
        let properties = ClassCreateProperties::builder()
            .description("Created class")
            .abap_language_version(AbapLanguageVersion::CloudDevelopment)
            .category(ClassCategory::ExceptionClass)
            .template(ClassTemplate::new("ZOTHERCLASS"))
            .package("$TMP")
            .build()
            .unwrap();
        let operation = reference("ZZZTEST").create(properties);
        let request = operation.encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("adtcore:abapLanguageVersion=\"5\""));
        assert!(body.contains("class:category=\"exceptionClass\""));
        assert!(body.contains("<abapsource:template abapsource:name=\"ZOTHERCLASS\""));
        assert!(
            body.find("<adtcore:packageRef").unwrap() < body.find("<abapsource:template").unwrap()
        );
        assert!(!body.contains("<abapsource:property"));
        assert!(!body.contains("class:templateName"));
        assert!(!body.contains("class:templateProperties"));
    }

    #[test]
    fn creation_serializes_template_properties_as_nested_elements() {
        let properties = ClassCreateProperties::builder()
            .description("Generated class")
            .template(
                ClassTemplate::new("IF_FOR_AUTO_CLASS_GENERATION")
                    .property("CCAU_CONTENT", "<cds:cdstobetested/>")
                    .property(
                        "Content-Type",
                        "application/vnd.sap.adt.oo.cds.codgen.v1+xml",
                    ),
            )
            .package("$TMP")
            .build()
            .unwrap();
        let operation = reference("ZZZTEST").create(properties);
        let request = operation.encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(
            body.contains("<abapsource:template abapsource:name=\"IF_FOR_AUTO_CLASS_GENERATION\">")
        );
        assert!(body.contains(
            "<abapsource:property abapsource:key=\"CCAU_CONTENT\">&lt;cds:cdstobetested/&gt;</abapsource:property>"
        ));
        assert!(body.contains(
            "<abapsource:property abapsource:key=\"Content-Type\">application/vnd.sap.adt.oo.cds.codgen.v1+xml</abapsource:property>"
        ));
        assert!(body.contains("</abapsource:template>"));
    }

    #[test]
    fn runtime_creation_rejects_non_creatable_object_types() {
        let reference = ObjectRef::<Package>::new(
            "$TMP".to_owned(),
            AdtUri::parse("/sap/bc/adt/packages/$tmp").unwrap(),
        )
        .erase();

        let error = reference.create(serde_json::json!({})).unwrap_err();

        assert!(matches!(
            error,
            ObjectError::UnsupportedCapability {
                capability: "object creation",
                ..
            }
        ));
    }

    #[test]
    fn creation_accepts_an_empty_success_response() {
        let operation = reference("ZZZTEST").create(create_properties());
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::CREATED, HeaderMap::new(), Vec::new()),
            AdtUri::parse("/sap/bc/adt/oo/classes").unwrap(),
        );

        assert!(operation.decode(response).unwrap().is_none());
    }

    #[test]
    fn creation_decodes_returned_properties() {
        let reference = reference("CL_ADT_URI_MAPPER");
        let operation = reference.create(create_properties());
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.sap.adt.oo.classes.v4+xml"),
        );
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::CREATED, headers.clone(), CLASS_XML.to_vec()),
            reference.uri().clone(),
        );
        let runtime_response = OperationResponse::new(
            AdtResponse::new(StatusCode::CREATED, headers, CLASS_XML.to_vec()),
            reference.uri().clone(),
        );
        let runtime = reference
            .erase()
            .create(serde_json::to_value(create_properties()).unwrap())
            .unwrap();

        let created = operation.decode(response).unwrap().unwrap();
        let runtime_created = runtime.decode(runtime_response).unwrap().unwrap();

        assert_eq!(created.properties().name, "CL_ADT_URI_MAPPER");
        assert_eq!(
            runtime_created.properties().unwrap()["@adtcore:name"],
            "CL_ADT_URI_MAPPER"
        );
    }
}
