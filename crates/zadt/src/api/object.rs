//! Core object operations for creation, snapshot queries, and property updates.
//!
//! Typed operations use concrete property models, while erased operations use
//! runtime descriptors to convert properties. The operations share response
//! decoding because each representation produces an object snapshot.
use super::transports::TRANSPORT_REQUEST_QUERY;
use crate::{
    Discovery, IfMatch, Locked, ObjectError, ObjectLock, ObjectSnapshot, RequiresDiscovery,
    SnapshotKind, TransportNumber,
    compatibility::MediaTypes,
    error::{EncodeError, ResponseError},
    objects::{
        AssignObjectIdentity, Create, MediaTyped, ObjectIdentity, ObjectRef, ObjectType, ToXml,
        WorkbenchVersion, XmlConversion,
    },
    operation::{EncodedOperation, IfNoneMatch, Operation, OperationResponse, Stateless},
    protocol::EntityTag,
};
use http::Method;

/// Creates a repository object from a family-specific creation payload.
///
/// Objects are created using a `POST` request on the object collection path,
/// which is generally discovered through its category. For instance, a
/// `POST` request to `/sap/bc/adt/oo/classes` creates a new class based
/// on a request body with the corresponding creation content type.
///
/// Because only a small subset of the properties are actually used during
/// creation, the API mirrors the properties into a separate struct instead
/// of marking all other fields as optional.
///
/// Successful responses do not contain a representation. Query the object
/// reference after creation to load its properties.
#[derive(Debug)]
pub struct ObjectCreation<T, P> {
    /// A reference to the object to create.
    reference: ObjectRef<T>,
    /// The request payload, either typed or JSON.
    payload: P,
    /// Media types supported for creation.
    media_types: MediaTypes,
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

    /// Shared internal helper for both typed and erased paths.
    fn build_request(
        &self,
        body: Vec<u8>,
        resolver: &Discovery,
    ) -> Result<EncodedOperation, EncodeError> {
        let collection = resolver.resolve_object_collection(&self.reference)?;
        let content_type = self
            .media_types
            .select_compatible(&collection.accepted_media_types)
            .map_err(ObjectError::from)?;

        let mut request = EncodedOperation::new(Method::POST, collection.target);
        request.set_content_type(content_type);
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
    P: AssignObjectIdentity + Clone + ToXml + Send + Sync,
{
    type Kind = Stateless;
    type Response = ();
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let reference = resolver.resolve_object(&self.reference)?;

        // PERF: No choice but to clone the payload because we need the resolver.
        // Not a big issue as its rather small and creation is rare.
        let mut payload = self.payload.clone();
        payload.assign_reference(&reference);

        let body = payload.to_xml()?;
        self.build_request(body, resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()
    }
}

// Untyped creation implementation
impl Operation for ObjectCreation<(), serde_json::Value> {
    type Kind = Stateless;
    type Response = ();
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let descriptor = self.reference.require_descriptor()?;
        let reference = resolver.resolve_object(&self.reference)?;

        // PERF: Same payload cloning is needed - identity set in the descriptor.
        let payload = descriptor.creation_payload_to_xml(&reference, self.payload.clone())?;
        self.build_request(payload, resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()
    }
}

impl<T> ObjectRef<T>
where
    T: Create,
{
    /// Constructs an [`Operation`] to create an object.
    ///
    /// The object identity, including Workbench type and name, comes from this
    /// reference.
    ///
    /// Other properties, such as description, ABAP language version, and
    /// object specific properties can be supplied in the payload.
    ///
    /// A successful response confirms creation without returning properties.
    pub fn create(&self, payload: T::Payload) -> ObjectCreation<T, T::Payload> {
        ObjectCreation {
            reference: self.clone(),
            payload,
            transport_request: None,
            media_types: T::CREATE_MEDIA_TYPES,
        }
    }
}

impl ObjectRef<()> {
    /// Constructs an [`Operation`] to create an object.
    ///
    /// The object identity, including Workbench type and name, comes from this
    /// reference.
    ///
    /// Other properties, such as description, ABAP language version, and
    /// object specific properties can be supplied in the payload.
    ///
    /// JSON conversion and XML encoding occur when the operation is encoded and
    /// can fail at that stage. A successful response confirms creation without
    /// returning properties.
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
            media_types: create_media_types,
        })
    }
}

/// Fetches a snapshot of the specified repository object.
///
/// A [`WorkbenchVersion`] can be passed to control which object state
/// is queried. If omitted, the server decides which version to return.
///
/// The returned [`ObjectSnapshot`] represents the state of the object,
/// combined with its version, ETag, and properties at the time it was
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
    /// Internal helper to build the request for both typed and erased paths.
    fn build_request(
        &self,
        media_types: MediaTypes,
        resolver: &Discovery,
    ) -> Result<EncodedOperation, EncodeError> {
        let target = resolver.resolve_object_uri(&self.resource)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        if let Some(version) = self.workbench_version {
            request.push_query(WorkbenchVersion::QUERY_PARAMETER, version.as_str());
        }

        request.set_accepts(media_types.as_slice());
        request.set_cache_revalidation(None);
        Ok(request)
    }

    /// Sets the workbench version of the object to be queried.
    pub fn workbench_version(mut self, version: WorkbenchVersion) -> Self {
        self.workbench_version = Some(version);
        self
    }

    /// Adds an `If-None-Match` cache validator to this query.
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
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        self.build_request(T::Properties::MEDIA_TYPES, resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ObjectSnapshot::<T>::decode(&self.resource, response)
    }
}

// Erased query implementation
impl Operation for ObjectQuery<()> {
    type Response = ObjectSnapshot<()>;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let descriptor = self.resource.require_descriptor()?;
        self.build_request(descriptor.properties_media_types(), resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ObjectSnapshot::<()>::decode(&self.resource, response)
    }
}

impl<T> ObjectRef<T> {
    /// Constructs an [`ObjectQuery<T>`] to snapshot this object.
    ///
    /// A specific Workbench version can be selected through the operation
    /// builder. The snapshot retains the version returned by the server, which
    /// is not necessarily the requested one.
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
    /// Constructs an ETag-decorated [`ObjectQuery`] to revalidate this
    /// snapshot, provided that the snapshot has an ETag.
    ///
    /// The response can then be used to check whether this snapshot is
    /// still the latest server state or, if not, replace the current
    /// snapshot.
    pub fn revalidation(&self) -> Option<IfNoneMatch<ObjectQuery<T>>> {
        let etag = self.etag()?.clone();
        Some(
            self.reference()
                .query()
                .workbench_version(self.workbench_version())
                .if_none_match(etag),
        )
    }
}

/// Updates object properties using a normalized XML request body.
///
/// Only properties supported by the concrete object type can be changed through
/// this operation. Source code updates use the corresponding [`crate::SourceRef`]
/// operations instead.
///
/// Successful execution decodes a new snapshot when ADT returns a response
/// representation. An empty success response returns `None`.
#[derive(Debug)]
pub struct ObjectUpdate<T> {
    /// A reference to the object to be updated
    resource: ObjectRef<T>,
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
    /// For a locked update, this replaces the transport request inherited from
    /// the lock.
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }

    // Shared request encoding for typed, erased, stateless, and stateful updates.
    fn build_request(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let target = resolver.resolve_object_uri(&self.resource)?;

        let mut request = EncodedOperation::new(Method::PUT, target);
        request.set_accept(self.media_type);
        request.set_content_type(self.media_type);
        request.set_body(self.body.clone());

        if let Some(transport_request) = &self.transport_request {
            request.push_query(TRANSPORT_REQUEST_QUERY, transport_request.as_str());
        }
        Ok(request)
    }
}

// typed implementation
impl<T> Operation for ObjectUpdate<T>
where
    T: ObjectType,
{
    type Response = Option<ObjectSnapshot<T>>;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        self.build_request(resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;

        if response.body().is_empty() {
            return Ok(None);
        }
        ObjectSnapshot::<T>::decode(&self.resource, response).map(Some)
    }
}

// erased implementation
impl Operation for ObjectUpdate<()> {
    type Response = Option<ObjectSnapshot<()>>;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        self.build_request(resolver)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }

        ObjectSnapshot::<()>::decode(&self.resource, response).map(Some)
    }
}

impl<T> Locked<ObjectUpdate<T>> {
    /// Assigns a transport request to record the update in.
    #[must_use]
    pub fn transport(self, transport: impl Into<TransportNumber>) -> Self {
        self.map_inner(|o| o.transport_request = Some(transport.into()))
    }
}

impl<T> IfMatch<ObjectUpdate<T>> {
    /// Assigns a transport request to record the update in.
    #[must_use]
    pub fn transport(self, transport: impl Into<TransportNumber>) -> Self {
        self.map_inner(|o| o.transport_request = Some(transport.into()))
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Creates a stateless update guarded by the entity tag from this snapshot.
    ///
    /// Construction fails when the snapshot has no entity tag. A failed HTTP
    /// precondition is represented by [`crate::PreconditionResult::Failed`].
    pub fn update_if_match(
        &self,
        properties: T::Properties,
    ) -> Result<IfMatch<ObjectUpdate<T>>, ObjectError> {
        let etag = self.etag().ok_or(ObjectError::MissingEntityTag)?.clone();
        self.update(properties)
            .map(|operation| IfMatch::new(operation, etag))
    }

    /// Creates a stateful update guarded by a persistent modification lock.
    ///
    /// The lock must belong to this object and permit modifications. Its user
    /// session and transport request are retained by the returned operation.
    pub fn update_with_lock(
        &self,
        lock: ObjectLock,
        properties: T::Properties,
    ) -> Result<Locked<ObjectUpdate<T>>, ObjectError> {
        let mut update = self.update(properties)?;
        update.transport_request = lock.transport_request().cloned();
        Locked::try_new(update, lock, self.reference())
    }

    fn update(&self, mut properties: T::Properties) -> Result<ObjectUpdate<T>, ObjectError> {
        properties.assign_identity(self.reference());

        Ok(ObjectUpdate {
            resource: self.reference().clone(),
            media_type: self.media_type(),
            body: properties.to_xml()?,
            transport_request: None,
        })
    }
}

impl ObjectSnapshot<()> {
    /// Creates a stateless update guarded by the entity tag from this snapshot.
    ///
    /// Construction fails when the snapshot has no entity tag. JSON conversion
    /// and XML encoding also occur during construction and can fail. A failed
    /// HTTP precondition is represented by [`crate::PreconditionResult::Failed`].
    pub fn update_if_match(
        &self,
        properties: serde_json::Value,
    ) -> Result<IfMatch<ObjectUpdate<()>>, ObjectError> {
        let etag = self.etag().ok_or(ObjectError::MissingEntityTag)?.clone();
        self.update(properties)
            .map(|operation| IfMatch::new(operation, etag))
    }

    /// Creates a stateful update guarded by a persistent modification lock.
    ///
    /// The lock must belong to this object and permit modifications. Its user
    /// session and transport request are retained by the returned operation.
    /// JSON conversion and XML encoding occur during construction and can fail.
    pub fn update_with_lock(
        &self,
        lock: ObjectLock,
        properties: serde_json::Value,
    ) -> Result<Locked<ObjectUpdate<()>>, ObjectError> {
        let mut update = self.update(properties)?;
        update.transport_request = lock.transport_request().cloned();
        Locked::try_new(update, lock, self.reference())
    }

    /// Shared helper to build the core update operation for erased objects.
    fn update(&self, properties: serde_json::Value) -> Result<ObjectUpdate<()>, ObjectError> {
        let descriptor = self.reference().require_descriptor()?;

        // Descriptor does the heavy lifting here, recover the typed properties
        // from JSON and then serialize them to xml. This all uses the same
        // implementations as the static path does under the hood.
        let properties = descriptor.properties_from_json(self.reference(), properties)?;
        let body = descriptor.properties_to_xml(self.reference(), &properties)?;

        Ok(ObjectUpdate {
            resource: self.reference().clone(),
            media_type: self.media_type(),
            body,
            transport_request: None,
        })
    }
}

impl<T: ObjectType> ObjectSnapshot<T> {
    /// Internal, module-private helper to construct the snapshot from a
    /// response since creation, updating and query all may return an
    /// object snapshot response with an identical content type.
    fn decode(resource: &ObjectRef<T>, response: OperationResponse) -> Result<Self, ResponseError> {
        response.require_success()?;
        let uri = response.request_target().clone();

        let supported = T::Properties::MEDIA_TYPES;
        let media_type = response.require_supported_media_type(supported)?;

        let properties = T::Properties::from_xml(response.body())?;
        properties.validate_for(resource)?;
        let extract = WorkbenchVersionExtractor::from_xml(response.body())?;

        Ok(Self::new(
            resource.clone(),
            uri,
            extract.workbench_version,
            media_type,
            response.etag(),
            properties,
        ))
    }
}

impl ObjectSnapshot<()> {
    /// Internal, module-private helper to construct the snapshot from a
    /// response since creation, updating and query all may return an
    /// object snapshot response with an identical content type.
    fn decode(resource: &ObjectRef, response: OperationResponse) -> Result<Self, ResponseError> {
        response.require_success()?;
        let uri = response.request_target().clone();
        let descriptor = resource.require_descriptor()?;

        let supported = descriptor.properties_media_types();
        let media_type = response.require_supported_media_type(supported)?;

        let properties = descriptor.properties_from_xml(resource, response.body())?;
        let extract = WorkbenchVersionExtractor::from_xml(response.body())?;

        Ok(Self::new_erased(
            resource.clone(),
            uri,
            extract.workbench_version,
            media_type,
            response.etag(),
            properties,
        ))
    }
}

/// Version-only projection, used after the complete properties model has been
/// decoded strictly. Intentionally accepts the other already-validated fields;
/// this is not a standalone response model.
#[derive(serde::Deserialize)]
struct WorkbenchVersionExtractor {
    #[serde(rename = "@adtcore:version")]
    workbench_version: WorkbenchVersion,
}

impl WorkbenchVersionExtractor {
    fn from_xml(body: &[u8]) -> Result<Self, ObjectError> {
        serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use super::*;
    use crate::api::locking::LOCK_HANDLE_QUERY;
    use crate::{
        AbapLanguageVersion, AccessMode, AdtRequest, AdtResponse, AdtUri,
        AdvertisedObjectReference, Class, ClassCategory, ClassCreateProperties, ClassProperties,
        ClassTemplate, Client, CompatibilityError, Discovery, FunctionGroup, FunctionGroupInclude,
        FunctionGroupIncludeCreateProperties, FunctionModule, FunctionModuleCreateProperties,
        ObjectType, Package, Stateful, Transport,
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

    fn discovered_client(xml: &[u8]) -> Client<Discovery> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(xml).unwrap(),
            crate::api::discovery::parse_capabilities(xml).unwrap(),
        )
    }

    fn reference(name: &str) -> ObjectRef<Class> {
        ObjectRef::new(name)
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
        let client = discovered_client(DISCOVERY_XML);
        let reference = reference("ZZZTEST");
        let properties = create_properties();
        let typed = reference.create(properties.clone());
        let mut runtime_payload = serde_json::to_value(properties).unwrap();
        let runtime_values = runtime_payload.as_object_mut().unwrap();
        runtime_values.remove("@adtcore:name");
        runtime_values.remove("@adtcore:type");
        let runtime = reference.erase().create(runtime_payload).unwrap();

        let typed_request = typed.encode(client.discovery()).unwrap();
        let runtime_request = runtime.encode(client.discovery()).unwrap();

        assert_eq!(typed_request.method(), Method::POST);
        assert!(!typed_request.headers().contains_key(header::ACCEPT));
        assert_eq!(
            typed_request.headers()[header::CONTENT_TYPE],
            ClassProperties::MEDIA_TYPES[0]
        );
        assert_eq!(typed_request.target().as_str(), "/sap/bc/adt/oo/classes");
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
        let client = discovered_client(DISCOVERY_XML);
        let reference = ObjectRef::<Class>::new("ZZZTEST");
        let typed = reference.create(create_properties());
        let runtime = reference
            .erase()
            .create(serde_json::to_value(create_properties()).unwrap())
            .unwrap();

        let typed = typed.encode(client.discovery()).unwrap();
        let runtime = runtime.encode(client.discovery()).unwrap();

        assert_eq!(
            typed.headers()[header::CONTENT_TYPE],
            ClassProperties::MEDIA_TYPES[0]
        );
        assert_eq!(
            runtime.headers()[header::CONTENT_TYPE],
            ClassProperties::MEDIA_TYPES[0]
        );
        assert_eq!(typed.target().as_str(), "/sap/bc/adt/oo/classes");
    }

    #[test]
    fn creation_rejects_a_collection_without_an_accepted_media_type() {
        let mut discovery = String::from_utf8(DISCOVERY_XML.to_vec()).unwrap();
        for media_type in ClassProperties::MEDIA_TYPES {
            discovery = discovery.replace(&format!("<app:accept>{media_type}</app:accept>"), "");
        }
        let client = discovered_client(discovery.as_bytes());
        let operation = ObjectRef::<Class>::new("ZZZTEST").create(create_properties());

        let Err(EncodeError::Object(ObjectError::Compatibility(error))) =
            operation.encode(client.discovery())
        else {
            panic!("creation encoding should reject a collection without app:accept")
        };

        match error {
            CompatibilityError::NoCompatibleMediaType {
                supported,
                accepted,
            } => {
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
    fn creation_uses_an_older_advertised_property_media_type() {
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
        let client = discovered_client(discovery.as_bytes());
        let operation = ObjectRef::<Class>::new("ZZZTEST").create(create_properties());

        let request = operation.encode(client.discovery()).unwrap();

        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            ClassProperties::MEDIA_TYPES[2]
        );
    }

    #[test]
    fn child_reference_resolves_its_collection() {
        let client = discovered_client(DISCOVERY_XML);
        let group = ObjectRef::<FunctionGroup>::new("Z_TEST_GROUP");
        let module = group.subobject::<FunctionModule>("ZZZZFUNC");

        let collection = client
            .discovery()
            .resolve_object_collection(&module)
            .unwrap();
        assert_eq!(
            collection.target.as_str(),
            "/sap/bc/adt/functions/groups/z_test_group/fmodules"
        );
        assert_eq!(
            collection.accepted_media_types,
            ["application/vnd.sap.adt.functions.fmodules.v3+xml"]
        );
    }

    #[test]
    fn typed_and_runtime_include_creation_use_the_parent_collection() {
        let client = discovered_client(DISCOVERY_XML);
        let group = ObjectRef::<FunctionGroup>::new("ZGROUP123");
        let include = group.subobject::<FunctionGroupInclude>("LZGROUP123RRR");
        let typed = include.create(
            FunctionGroupIncludeCreateProperties::builder()
                .description("zttfart")
                .build()
                .unwrap(),
        );
        let runtime = include
            .erase()
            .create(serde_json::json!({ "@adtcore:description": "zttfart" }))
            .unwrap();

        let typed = typed.encode(client.discovery()).unwrap();
        let runtime = runtime.encode(client.discovery()).unwrap();
        assert_eq!(
            typed.target().as_str(),
            "/sap/bc/adt/functions/groups/zgroup123/includes"
        );
        assert_eq!(
            typed.headers()[header::CONTENT_TYPE],
            "application/vnd.sap.adt.functions.fincludes.v2+xml"
        );
        assert_eq!(typed.body(), runtime.body());
        let body = std::str::from_utf8(typed.body()).unwrap();
        assert!(body.contains("<adtcore:containerRef"));
        assert!(body.contains("adtcore:name=\"ZGROUP123\""));
        assert!(body.contains("adtcore:type=\"FUGR/F\""));
        assert!(body.contains("adtcore:uri=\"/sap/bc/adt/functions/groups/zgroup123\""));
    }

    #[test]
    fn typed_and_runtime_function_module_creation_use_the_parent_collection() {
        let client = discovered_client(DISCOVERY_XML);
        let group = ObjectRef::<FunctionGroup>::new("ZGROUP123");
        let module = group.subobject::<FunctionModule>("ZFTFART");
        let typed = module.create(
            FunctionModuleCreateProperties::builder()
                .description("tfatart")
                .build()
                .unwrap(),
        );
        let runtime = module
            .erase()
            .create(serde_json::json!({ "@adtcore:description": "tfatart" }))
            .unwrap();

        let typed = typed.encode(client.discovery()).unwrap();
        let runtime = runtime.encode(client.discovery()).unwrap();
        assert_eq!(
            typed.target().as_str(),
            "/sap/bc/adt/functions/groups/zgroup123/fmodules"
        );
        assert_eq!(
            typed.headers()[header::CONTENT_TYPE],
            "application/vnd.sap.adt.functions.fmodules.v3+xml"
        );
        assert_eq!(typed.body(), runtime.body());
        let body = std::str::from_utf8(typed.body()).unwrap();
        assert!(body.contains("<fmodule:abapFunctionModule"));
        assert!(body.contains("adtcore:description=\"tfatart\""));
        assert!(body.contains("adtcore:name=\"ZFTFART\""));
        assert!(body.contains("adtcore:type=\"FUGR/FF\""));
        assert!(body.contains("<adtcore:containerRef"));
        assert!(body.contains("adtcore:name=\"ZGROUP123\""));
        assert!(body.contains("adtcore:type=\"FUGR/F\""));
        assert!(body.contains("adtcore:uri=\"/sap/bc/adt/functions/groups/zgroup123\""));
        assert!(!body.contains("fmodule:processingType"));
        assert!(!body.contains("fmodule:releaseState"));
    }

    #[test]
    fn typed_and_erased_queries_reject_unknown_property_fields() {
        let reference = reference("CL_ADT_URI_MAPPER");
        let original = std::str::from_utf8(CLASS_XML).unwrap();
        for element in ["class:abapClass", "adtcore:packageRef", "atom:link"] {
            let marker = format!("<{element}");
            let xml = original.replacen(&marker, &format!("{marker} unexpected=\"value\""), 1);
            assert_ne!(xml, original);
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(ClassProperties::MEDIA_TYPES[0]),
            );
            let response = || {
                OperationResponse::new(
                    AdtResponse::new(StatusCode::OK, headers.clone(), xml.as_bytes().to_vec()),
                    AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
                )
            };
            let typed = reference.query().decode(response()).unwrap_err();
            let erased = reference.erase().query().decode(response()).unwrap_err();
            for error in [typed, erased] {
                assert!(
                    error.to_string().contains("unknown field `@unexpected`"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn runtime_writes_reject_unknown_root_and_nested_fields() {
        let client = discovered_client(DISCOVERY_XML);
        let reference = reference("CL_ADT_URI_MAPPER");
        let properties: ClassProperties = serde_xml_rs::from_reader(CLASS_XML).unwrap();
        let snapshot = ObjectSnapshot::new(
            reference.clone(),
            AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            WorkbenchVersion::Active,
            ClassProperties::MEDIA_TYPES[0],
            Some(EntityTag::from_static("class-etag")),
            properties,
        )
        .into_erased();
        for pointer in ["", "/adtcore:packageRef"] {
            let mut create = serde_json::to_value(create_properties()).unwrap();
            create.pointer_mut(pointer).unwrap()["unexpected"] = true.into();
            let error = reference
                .erase()
                .create(create)
                .unwrap()
                .encode(client.discovery())
                .err()
                .unwrap();
            assert!(
                error.to_string().contains("unknown field `unexpected`"),
                "{error}"
            );

            let mut update = snapshot.properties().unwrap();
            update.pointer_mut(pointer).unwrap()["unexpected"] = true.into();
            let error = snapshot.update_if_match(update).unwrap_err();
            assert!(
                error.to_string().contains("unknown field `unexpected`"),
                "{error}"
            );
        }
    }

    #[test]
    fn runtime_creation_applies_the_typed_builder_defaults() {
        let client = discovered_client(DISCOVERY_XML);
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

        let typed_request = typed.encode(client.discovery()).unwrap();
        let runtime_request = runtime.encode(client.discovery()).unwrap();

        assert_eq!(runtime_request.body(), typed_request.body());
    }

    #[test]
    fn creation_uses_the_reference_identity() {
        let client = discovered_client(DISCOVERY_XML);
        let mut properties = create_properties();
        properties.name = "ZOTHER".to_owned();
        properties.object_type = Package::WORKBENCH_TYPE;
        let operation = reference("ZZZTEST").create(properties);
        let request = operation.encode(client.discovery()).unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("adtcore:name=\"ZZZTEST\""));
        assert!(body.contains("adtcore:type=\"CLAS/OC\""));
        assert!(!body.contains("ZOTHER"));
    }

    #[test]
    fn creation_serializes_optional_class_settings() {
        let client = discovered_client(DISCOVERY_XML);
        let properties = ClassCreateProperties::builder()
            .description("Created class")
            .abap_language_version(AbapLanguageVersion::CloudDevelopment)
            .category(ClassCategory::ExceptionClass)
            .template(ClassTemplate::new("ZOTHERCLASS"))
            .package("$TMP")
            .build()
            .unwrap();
        let operation = reference("ZZZTEST").create(properties);
        let request = operation.encode(client.discovery()).unwrap();
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
        let client = discovered_client(DISCOVERY_XML);
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
        let request = operation.encode(client.discovery()).unwrap();
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
        let reference = ObjectRef::<Package>::new("$TMP").erase();

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
    fn creation_accepts_a_success_response() {
        let operation = reference("ZZZTEST").create(create_properties());
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::CREATED, HeaderMap::new(), Vec::new()),
            AdtUri::parse("/sap/bc/adt/oo/classes").unwrap(),
        );

        assert_eq!(operation.decode(response).unwrap(), ());
    }

    #[test]
    fn updates_canonicalize_identity_and_use_the_property_version() {
        let client = discovered_client(DISCOVERY_XML);
        let reference = reference("CL_ADT_URI_MAPPER");
        let properties: ClassProperties = serde_xml_rs::from_reader(CLASS_XML).unwrap();
        let snapshot = ObjectSnapshot::new(
            reference.clone(),
            AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            WorkbenchVersion::Active,
            ClassProperties::MEDIA_TYPES[0],
            Some(EntityTag::from_static("class-etag")),
            properties.clone(),
        );
        let lock = ObjectLock::for_test(reference.erase(), AccessMode::Modify);

        let mut typed_properties = properties;
        typed_properties.name = "ZOTHER".to_owned();
        typed_properties.object_type = Package::WORKBENCH_TYPE;
        typed_properties.version = WorkbenchVersion::Inactive;
        let typed_update = snapshot.update_if_match(typed_properties).unwrap();
        fn assert_stateless<O: Operation<Kind = Stateless>>(_: &O) {}
        assert_stateless(&typed_update);
        let typed = typed_update.encode(client.discovery()).unwrap();

        let runtime_snapshot = snapshot.into_erased();
        let mut runtime_properties = runtime_snapshot.properties().unwrap();
        runtime_properties["@adtcore:name"] = "ZOTHER".into();
        runtime_properties["@adtcore:type"] = "DEVC/K".into();
        runtime_properties["@adtcore:version"] = "inactive".into();
        let runtime_update = runtime_snapshot
            .update_with_lock(lock, runtime_properties)
            .unwrap();
        fn assert_stateful<O: Operation<Kind = Stateful>>(_: &O) {}
        assert_stateful(&runtime_update);
        let runtime = runtime_update.encode(client.discovery()).unwrap();

        assert_eq!(typed.headers().get(header::IF_MATCH).unwrap(), "class-etag");
        assert!(
            typed
                .query()
                .iter()
                .all(|(name, _)| name != LOCK_HANDLE_QUERY)
        );
        assert!(!runtime.headers().contains_key(header::IF_MATCH));
        assert!(
            runtime
                .query()
                .contains(&(LOCK_HANDLE_QUERY.to_owned(), "LOCK-HANDLE".to_owned()))
        );

        for request in [typed, runtime] {
            let body = std::str::from_utf8(request.body()).unwrap();
            let root = body
                .split_once("<class:abapClass")
                .unwrap()
                .1
                .split_once('>')
                .unwrap()
                .0;
            assert!(body.contains("adtcore:name=\"CL_ADT_URI_MAPPER\""));
            assert!(body.contains("adtcore:type=\"CLAS/OC\""));
            assert!(!body.contains("ZOTHER"));
            assert!(root.contains("adtcore:version=\"inactive\""));
        }
    }

    #[test]
    fn optimistic_update_reports_a_failed_precondition() {
        let client = discovered_client(DISCOVERY_XML);
        let reference = reference("CL_ADT_URI_MAPPER");
        let properties: ClassProperties = serde_xml_rs::from_reader(CLASS_XML).unwrap();
        let snapshot = ObjectSnapshot::new(
            reference.clone(),
            AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            WorkbenchVersion::Active,
            ClassProperties::MEDIA_TYPES[0],
            Some(EntityTag::from_static("stale-etag")),
            properties.clone(),
        );
        let update = snapshot.update_if_match(properties).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::ETAG, HeaderValue::from_static("current-etag"));
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::PRECONDITION_FAILED, headers, Vec::new()),
            client.discovery().resolve_object_uri(&reference).unwrap(),
        );

        let result = update.decode(response).unwrap();

        assert!(matches!(
            result,
            crate::PreconditionResult::Failed { etag }
                if etag.as_deref() == Some("current-etag")
        ));
    }

    #[test]
    fn locked_update_does_not_require_an_entity_tag() {
        let client = discovered_client(DISCOVERY_XML);
        let reference = reference("CL_ADT_URI_MAPPER");
        let properties: ClassProperties = serde_xml_rs::from_reader(CLASS_XML).unwrap();
        let snapshot = ObjectSnapshot::new(
            reference.clone(),
            AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            WorkbenchVersion::Active,
            ClassProperties::MEDIA_TYPES[0],
            None,
            properties.clone(),
        );
        let error = snapshot.update_if_match(properties.clone()).unwrap_err();
        assert!(matches!(error, ObjectError::MissingEntityTag));

        let lock = ObjectLock::for_test_with_transport(
            reference.erase(),
            AccessMode::Modify,
            "A4HK900001",
        );
        let lock_session = lock.user_session();
        let request = snapshot
            .update_with_lock(lock.clone(), properties)
            .unwrap()
            .encode(client.discovery())
            .unwrap();

        assert!(!request.headers().contains_key(header::IF_MATCH));
        assert!(
            request
                .query()
                .contains(&(LOCK_HANDLE_QUERY.to_owned(), "LOCK-HANDLE".to_owned()))
        );
        assert!(
            request
                .query()
                .contains(&(TRANSPORT_REQUEST_QUERY.to_owned(), "A4HK900001".to_owned()))
        );
        let (_, _, bound_session) = request.into_parts();
        assert_eq!(bound_session, lock_session);

        let request = snapshot
            .update_with_lock(lock, snapshot.properties().clone())
            .unwrap()
            .transport("A4HK900002")
            .encode(client.discovery())
            .unwrap();
        assert!(
            request
                .query()
                .contains(&(TRANSPORT_REQUEST_QUERY.to_owned(), "A4HK900002".to_owned()))
        );
        assert!(
            !request
                .query()
                .contains(&(TRANSPORT_REQUEST_QUERY.to_owned(), "A4HK900001".to_owned()))
        );

        let show_lock = ObjectLock::for_test(reference.erase(), AccessMode::Show);
        let error = snapshot
            .update_with_lock(show_lock, snapshot.properties().clone())
            .unwrap_err();
        assert!(matches!(error, ObjectError::ObjectLockNotModifiable));
    }
}
