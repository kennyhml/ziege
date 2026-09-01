/// Everything related to core object operations such as querying snapshots,
/// updating object properties or creating objects.
///
/// Because all of this directly involves the objects properties, alof of
/// parallel implementations are needed for typed and erased paths where
/// the erased path must use the descriptor to cast the properties.
///
/// Because all of the operations return an object snapshot response using
/// the same media type, it makes sense to keep them in one module.
use super::{locking::LOCK_HANDLE_QUERY, transports::TRANSPORT_REQUEST_QUERY};
use crate::{
    Advertised, CategoryId, ObjectError, ObjectLock, ObjectSnapshot, SnapshotKind, TransportNumber,
    compatibility,
    error::{EncodeError, ResponseError},
    objects::{
        AssignObjectIdentity, Create, MediaTyped, ObjectIdentity, ObjectRef, ObjectType, ToXml,
        WorkbenchVersion, XmlConversion,
    },
    operation::{
        CollectionLocator, EncodedOperation, IfNoneMatch, Operation, OperationResponse, Owned,
        Stateful, Stateless,
    },
    protocol::EntityTag,
};
use http::Method;

/// Creates a repository object from a family-specific creation payload.
///
/// Objects are created using a `POST` request on the objects base path,
/// which is generally discovered through its category. For instance, a
/// `POST` request to `/sap/bc/adt/oo/classes` creates a new class based
/// on the request body, which has the same content type as the objects
/// properties.
///
/// Because only a small subset of the properties are actually used during
/// creation, the api mirrors the properties into a separate struct instead
/// of marking all other fields as optional.
///
/// Successful responses without a representation decode to `None`. Object
/// families that return their properties decode to a loaded object.
#[derive(Debug)]
pub struct ObjectCreation<T, P> {
    /// A reference to the object to create, needed to decode the response.
    reference: ObjectRef<T>,
    /// The request payload, either typed or JSON.
    payload: P,
    /// Media types we support creation with.
    create_media_types: &'static [&'static str],
    /// Media types we accept as response.
    response_media_types: &'static [&'static str],
    /// A transport request to assign the new object to.
    transport_request: Option<TransportNumber>,
}

impl<T, P> ObjectCreation<T, P> {
    /// Records the creation in the supplied transport request.
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }

    /// Shared internal helper for both typed and erased paths
    fn build_request(
        &self,
        category: CategoryId,
        body: Vec<u8>,
    ) -> Result<EncodedOperation<Advertised>, EncodeError> {
        // Mark the target collection that we require it to accept one
        // of the creation media types we used. This is then validated
        // when the target collection is resolved.
        let mut target = CollectionLocator::new(category).target();
        target.require_accepted_media_types(self.create_media_types);

        let mut request = EncodedOperation::advertised(Method::POST, target);
        request.set_accepts(self.response_media_types);
        request.set_body(body);

        // Transport handling, no lock here that may carry one.
        if let Some(transport) = &self.transport_request {
            request.push_query(TRANSPORT_REQUEST_QUERY, transport.as_str());
        }
        Ok(request)
    }
}

// Typed creation implementation
impl<T, P> Operation for ObjectCreation<T, P>
where
    T: Create<Payload = P>,
    P: ToXml + Send + Sync,
{
    type Kind = Stateless;
    type Response = Option<ObjectSnapshot<T>>;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        self.build_request(T::CATEGORY, self.payload.to_xml()?)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }
        ObjectSnapshot::<T>::decode(&self.reference, response).map(Some)
    }
}

// Untyped creation implementation
impl Operation for ObjectCreation<(), serde_json::Value> {
    type Kind = Stateless;
    type Response = Option<ObjectSnapshot<()>>;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let descriptor = self.reference.require_descriptor()?;

        // Currently only primary object creation is supported
        let category = descriptor
            .category()
            .ok_or_else(|| ObjectError::ParentObjectRequired {
                object_type: self.reference.object_type().clone(),
            })?;

        let payload = descriptor.creation_payload_to_xml(&self.reference, self.payload.clone())?;
        self.build_request(category, payload)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }
        ObjectSnapshot::<()>::decode(&self.reference, response).map(Some)
    }
}

impl<T> ObjectRef<T>
where
    T: Create,
{
    /// Constructs an [`Operation`] to create an object.
    ///
    /// The object identity (workbench type & name) are taken from `self`.
    ///
    /// Other properties, such as description, abap language version and
    /// object specific properties can be supplied in the payload.
    ///
    /// The created object is returnd as snapshot on success.
    pub fn create(&self, mut payload: T::Payload) -> ObjectCreation<T, T::Payload> {
        // Make sure identify matches the reference. In the erased path,
        // the same thing happens during the descriptor xml serialization.
        payload.assign_identity(self);

        ObjectCreation {
            reference: self.clone(),
            payload,
            transport_request: None,
            create_media_types: T::CREATE_MEDIA_TYPES,
            response_media_types: T::Properties::MEDIA_TYPES,
        }
    }
}

impl ObjectRef<()> {
    /// Constructs an [`Operation`] to create an object.
    ///
    /// The object identity (workbench type & name) are taken from `self`.
    ///
    /// Other properties, such as description, abap language version and
    /// object specific properties can be supplied in the payload.
    ///
    /// Because the payload is supplied as JSON, deserialization to a typed
    /// payload may fail at this point.
    ///
    /// The created object is returnd as snapshot on success.
    pub fn create(
        &self,
        payload: serde_json::Value,
    ) -> Result<ObjectCreation<(), serde_json::Value>, ObjectError> {
        let descriptor = self.require_descriptor()?;
        // If there are no creation media types, we already know that this
        // object type does not support creation.
        let create_media_types = descriptor.creation_media_types().ok_or_else(|| {
            ObjectError::UnsupportedCapability {
                object_type: self.object_type().clone(),
                capability: "object creation",
            }
        })?;

        Ok(ObjectCreation {
            reference: self.clone(),
            payload,
            transport_request: None,
            create_media_types,
            response_media_types: descriptor.properties_media_types(),
        })
    }
}

/// Fetches a snapshot of the specified repository object.
///
/// A [`WorkbenchVersion`] can be passed to control which object state
/// is queried. If omitted, the server decides which version to return.
///
/// The returned [`ObjectSnapshot`] represents the state of the object,
/// combined with its version, etag and properties, at the time it was
/// queried.
///
/// It is a snapshot because it makes no guarantees that the object
/// state has not changed since it has been queried - in extreme
/// cases the object may even have been deleted since querying it.
#[derive(Debug)]
pub struct ObjectQuery<T> {
    resource: ObjectRef<T>,
    workbench_version: Option<WorkbenchVersion>,
}

impl<T> ObjectQuery<T> {
    /// Internal helper to build the request for both typed and erased paths
    fn build_request(&self, media_types: &'static [&'static str]) -> EncodedOperation<Owned> {
        // Because the object uri is resolved at object construction through
        // the client, the request target is already owned. This kinda blurs
        // the lines between the responsibilities but its worthwhile being
        // able to use ObjectRef for both internal and advertised objects.
        let mut request = EncodedOperation::owned(Method::GET, self.resource.uri().clone());
        if let Some(version) = self.workbench_version {
            request.push_query(WorkbenchVersion::QUERY_PARAMETER, version.as_str());
        }

        request.set_accepts(media_types);
        request.set_cache_revalidation(None);
        request
    }

    /// Sets the workbench version of the object to be queried.
    pub fn workbench_version(mut self, version: WorkbenchVersion) -> Self {
        self.workbench_version = Some(version);
        self
    }

    pub fn if_none_match(self, etag: EntityTag) -> IfNoneMatch<Self> {
        IfNoneMatch::new(self, etag)
    }
}

// Typed query implementation
impl<T> Operation for ObjectQuery<T>
where
    T: ObjectType,
{
    type Response = ObjectSnapshot<T>;
    type Kind = Stateless;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        Ok(self.build_request(T::Properties::MEDIA_TYPES))
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ObjectSnapshot::<T>::decode(&self.resource, response)
    }
}

// Erased query implementation
impl Operation for ObjectQuery<()> {
    type Response = ObjectSnapshot<()>;
    type Kind = Stateless;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let descriptor = self.resource.require_descriptor()?;
        Ok(self.build_request(descriptor.properties_media_types()))
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ObjectSnapshot::<()>::decode(&self.resource, response)
    }
}

impl<T> ObjectRef<T> {
    /// Constructs an [`ObjectQuery<T>`] to snapshot this object.
    ///
    /// A specific workbench version can be selected through the operation
    /// builder. The snapshot retains the version retured by the server -
    /// which is not neccessarily the requested one.
    ///
    /// Because the [`ObjectRef`] makes no guarantees that the object
    /// it represents actually exists, the query may not find the object.
    pub fn query(&self) -> ObjectQuery<T> {
        ObjectQuery {
            resource: self.clone(),
            workbench_version: None,
        }
    }
}

impl<T: SnapshotKind> ObjectSnapshot<T> {
    /// Constructs an etag-decorated [`ObjectQuery`] to revalidate this
    /// snapshot, provided that the snapshot has an etag.
    ///
    /// The response can then be used to check whether this snapshot is
    /// still the latest server state or, if not, replace the current
    /// snapshot.
    pub fn revalidate(&self) -> Option<IfNoneMatch<ObjectQuery<T>>> {
        self.etag()
            .cloned()
            .map(|etag| self.reference().query().if_none_match(etag))
    }
}

/// Updates an objects properties.
///
/// Successful execution decodes a new loaded object when ADT returns a response
/// representation. An empty success response returns `None`.
#[derive(Debug)]
pub struct ObjectUpdate<T> {
    /// A reference to the object to be updated
    resource: ObjectRef<T>,
    /// The lock to update the object with, bound to some user session
    object_lock: ObjectLock,
    /// The new, already encoded properties
    body: Vec<u8>,
    /// The content type of the request body
    media_type: &'static str,
    /// A transport request to assign the changes to
    transport_request: Option<TransportNumber>,
}

impl<T> ObjectUpdate<T> {
    /// Records this update in the supplied transport request.
    ///
    /// This replaces any transport request inherited from the lock.
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }

    /// Shared helper to construct the operation for both typed and erased paths.
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

impl<T> Operation for ObjectUpdate<T>
where
    T: ObjectType,
{
    type Response = Option<ObjectSnapshot<T>>;
    type Kind = Stateful;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        Ok(self.build_request())
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;

        if response.body().is_empty() {
            return Ok(None);
        }
        ObjectSnapshot::<T>::decode(&self.resource, response).map(Some)
    }
}

impl Operation for ObjectUpdate<()> {
    type Response = Option<ObjectSnapshot<()>>;
    type Kind = Stateful;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        Ok(self.build_request())
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;

        if response.body().is_empty() {
            return Ok(None);
        }
        ObjectSnapshot::<()>::decode(&self.resource, response).map(Some)
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Creates an update operation for this object.
    ///
    /// Only certain object properties, such as description, language
    /// versions or object specifific properties can be updated through
    /// this method. Source code updates take place on their respective
    /// [`crate::SourceRef`] handles.
    ///
    /// Whether a property can be updated or not depends on the concrete object
    /// type. When ADT returns a representation, the response includes the new
    /// state after applying all possible updates on an inactive object snapshot.
    pub fn update(
        &self,
        lock: &ObjectLock,
        properties: T::Properties,
    ) -> Result<ObjectUpdate<T>, ObjectError> {
        lock.validate_modification_for(self.reference())?;
        properties.validate_for(self.reference())?;

        let media_type =
            compatibility::matching_media_type(T::Properties::MEDIA_TYPES, self.media_type())
                .expect("typed ADT objects carry a supported media type");

        Ok(ObjectUpdate {
            resource: self.reference().clone(),
            object_lock: lock.clone(),
            media_type,
            body: properties.to_xml()?,
            transport_request: lock.transport_request().cloned(),
        })
    }
}

impl ObjectSnapshot<()> {
    /// Creates an update operation for this object.
    ///
    /// Only certain object properties, such as description, language
    /// versions or object specifific properties can be updated through
    /// this method. Source code updates take place on their respective
    /// [`crate::SourceRef`] handles.
    ///
    /// Whether a property can be updated or not depends on the concrete object
    /// type. When ADT returns a representation, the response includes the new
    /// state after applying all possible updates on an inactive object snapshot.
    ///
    /// Because the properties are supplied as JSON, deserialization to the
    /// concrete object properties may fail at this point.
    pub fn update(
        &self,
        lock: &ObjectLock,
        properties: serde_json::Value,
    ) -> Result<ObjectUpdate<()>, ObjectError> {
        lock.validate_modification_for(self.reference())?;
        let descriptor = self.reference().require_descriptor()?;

        // Select media type through the descriptor
        let media_type = compatibility::matching_media_type(
            descriptor.properties_media_types(),
            self.media_type(),
        )
        .ok_or_else(|| ObjectError::UnsupportedPropertiesMediaType {
            object_type: self.reference().object_type().clone(),
            media_type: self.media_type().to_owned(),
        })?;

        // Descriptor does the heavy lifting here, recover the typed properties
        // from JSON and then serialize them to xml. This all uses the same
        // implementations as the static path does under the hood.
        let properties = descriptor.properties_from_json(self.reference(), properties)?;
        let body = descriptor.properties_to_xml(self.reference(), &properties)?;

        Ok(ObjectUpdate {
            resource: self.reference().clone(),
            object_lock: lock.clone(),
            media_type,
            body,
            transport_request: lock.transport_request().cloned(),
        })
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Internal, module-private helper to construct the snapshot from a
    /// response since creation, updating and query all may return an
    /// object snapshot response with an identical content type.
    fn decode(resource: &ObjectRef<T>, response: OperationResponse) -> Result<Self, ResponseError> {
        response.require_success()?;

        let supported = T::Properties::MEDIA_TYPES;
        let content_type = response.require_content_type(supported)?;
        let media_type = compatibility::matching_media_type(supported, content_type)
            .expect("validated properties Content-Type must match a supported media type");

        let etag = response.entity_tag();
        let properties = T::Properties::from_xml(response.body())?;
        properties.validate_for(resource)?;

        Ok(Self::new(resource.clone(), media_type, etag, properties))
    }
}

impl ObjectSnapshot<()> {
    /// Internal, module-private helper to construct the snapshot from a
    /// response since creation, updating and query all may return an
    /// object snapshot response with an identical content type.
    fn decode(
        resource: &ObjectRef<()>,
        response: OperationResponse,
    ) -> Result<Self, ResponseError> {
        response.require_success()?;
        let descriptor = resource.require_descriptor()?;

        let supported = descriptor.properties_media_types();
        let content_type = response.require_content_type(supported)?;
        let media_type = compatibility::matching_media_type(supported, content_type)
            .expect("validated properties Content-Type must match a supported media type");

        let etag = response.entity_tag();
        let properties = descriptor.properties_from_xml(resource, response.body())?;
        Ok(Self::new_erased(
            resource.clone(),
            media_type,
            etag,
            properties,
        ))
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
