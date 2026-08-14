use std::marker::PhantomData;

use http::{Method, StatusCode, header};

use crate::{
    Erased, ObjectError, ObjectLock, TransportNumber,
    client::{Client, ClientState, Ready},
    error::{OperationError, ResponseError},
    objects::{
        ObjectRef, ObjectVersion, PropertyModel, ReadProperties, RuntimeObjectTypeDescriptor,
        UpdateProperties,
    },
    operation::{IfNoneMatch, Operation, OperationResponse, Stateful, Stateless},
    protocol::{AdtRequest, EntityTag},
    vocabulary::query_parameter,
};

/// A fetched object-properties payload and its transport metadata.
#[derive(Clone, Debug)]
pub struct ObjectProperties<P>
where
    P: PropertyModel,
{
    pub(crate) media_version: P::Version,
    pub etag: Option<EntityTag>,
    pub payload: P,
}

impl<P> ObjectProperties<P>
where
    P: PropertyModel,
{
    /// Returns the media-type version used by this representation.
    pub fn media_version(&self) -> P::Version {
        self.media_version
    }

    /// Returns the media type used by this representation.
    pub fn media_type(&self) -> &'static str {
        P::media_type(self.media_version)
    }
}

/// A fetched object-properties payload exposed through its runtime JSON form.
#[derive(Clone, Debug)]
pub struct JsonObjectProperties {
    pub(crate) media_type: &'static str,
    pub etag: Option<EntityTag>,
    pub payload: serde_json::Value,
}

impl JsonObjectProperties {
    pub fn media_type(&self) -> &'static str {
        self.media_type
    }
}

/// Fetches a versioned, statically typed object-properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    pub resource: ObjectRef<T>,
    pub version: Option<ObjectVersion>,
}

impl<T> ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    pub fn new(resource: ObjectRef<T>) -> Self {
        Self {
            resource,
            version: None,
        }
    }

    pub fn version(mut self, version: ObjectVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn if_none_match(self, etag: EntityTag) -> IfNoneMatch<Self> {
        IfNoneMatch::new(self, etag)
    }
}

impl<T> Operation<Ready> for ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    type Response = ObjectProperties<T::Properties>;
    type Kind = Stateless;

    fn request(&self, _client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::GET, self.resource.uri().clone());
        if let Some(version) = self.version {
            request.push_query(query_parameter::VERSION, version.as_str());
        }
        let media_types = T::Properties::SUPPORTED_VERSIONS
            .iter()
            .map(|version| T::Properties::media_type(*version))
            .collect::<Vec<_>>();
        request.set_accepts(&media_types);
        request.set_cache_revalidation(None);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_query::<T>(response)
    }
}

impl<T> ObjectRef<T>
where
    T: ReadProperties,
{
    pub fn query(&self) -> ObjectPropertiesQuery<T> {
        ObjectPropertiesQuery::new(self.clone())
    }
}

/// Fetches one runtime-typed object's properties as JSON.
#[derive(Debug)]
pub struct JsonObjectPropertiesQuery {
    resource: ObjectRef<Erased>,
    descriptor: &'static dyn RuntimeObjectTypeDescriptor,
    version: Option<ObjectVersion>,
}

impl JsonObjectPropertiesQuery {
    pub fn version(mut self, version: ObjectVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn if_none_match(self, etag: EntityTag) -> IfNoneMatch<Self> {
        IfNoneMatch::new(self, etag)
    }
}

impl Operation<Ready> for JsonObjectPropertiesQuery {
    type Response = JsonObjectProperties;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        self.descriptor
            .properties_request(&self.resource, self.version, client)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        self.descriptor.properties_to_json(&self.resource, response)
    }
}

impl ObjectRef<Erased> {
    pub fn query(&self) -> Result<JsonObjectPropertiesQuery, ObjectError> {
        let descriptor = self
            .descriptor()
            .ok_or_else(|| unsupported(self, "object properties"))?;
        Ok(JsonObjectPropertiesQuery {
            resource: self.clone(),
            descriptor,
            version: None,
        })
    }
}

/// Replaces an object's statically typed properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesUpdate<T>
where
    T: ReadProperties,
{
    resource: ObjectRef<Erased>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
    marker: PhantomData<fn() -> T>,
}

impl<T> ObjectPropertiesUpdate<T>
where
    T: ReadProperties,
{
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }
}

impl<S, T> Operation<S> for ObjectPropertiesUpdate<T>
where
    S: ClientState,
    T: ReadProperties,
{
    type Response = Option<ObjectProperties<T::Properties>>;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::PUT, self.resource.uri().clone());
        request.push_query(query_parameter::LOCK_HANDLE, self.object_lock.handle());
        if let Some(transport_request) = &self.transport_request {
            request.push_query(
                query_parameter::TRANSPORT_REQUEST,
                transport_request.as_str(),
            );
        }
        if let Some(user_session) = self.object_lock.user_session() {
            request.require_user_session(user_session);
        }
        request.set_accept(self.media_type);
        request.set_content_type(self.media_type);
        request.set_body(self.body.clone());
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_update(response, decode_query::<T>)
    }
}

impl<T> ObjectRef<T>
where
    T: UpdateProperties,
{
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: ObjectProperties<T::Properties>,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        let media_type = properties.media_type();
        let serializer = T::Properties::XML_NAMESPACES.iter().fold(
            serde_xml_rs::SerdeXml::new(),
            |serializer, &(prefix, namespace)| serializer.namespace(prefix, namespace),
        );
        let body = serializer
            .to_string(&properties.payload)
            .map_err(ObjectError::InvalidRequest)?
            .into_bytes();
        Ok(ObjectPropertiesUpdate {
            resource: self.erase(),
            object_lock: object_lock.clone(),
            media_type,
            body,
            transport_request: object_lock.transport_request().cloned(),
            marker: PhantomData,
        })
    }
}

/// Replaces one runtime-typed object's JSON properties representation.
#[derive(Debug)]
pub struct JsonObjectPropertiesUpdate {
    resource: ObjectRef<Erased>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
    descriptor: &'static dyn RuntimeObjectTypeDescriptor,
}

impl JsonObjectPropertiesUpdate {
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }
}

impl<S> Operation<S> for JsonObjectPropertiesUpdate
where
    S: ClientState,
{
    type Response = Option<JsonObjectProperties>;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::PUT, self.resource.uri().clone());
        request.push_query(query_parameter::LOCK_HANDLE, self.object_lock.handle());
        if let Some(transport_request) = &self.transport_request {
            request.push_query(
                query_parameter::TRANSPORT_REQUEST,
                transport_request.as_str(),
            );
        }
        if let Some(user_session) = self.object_lock.user_session() {
            request.require_user_session(user_session);
        }
        request.set_accept(self.media_type);
        request.set_content_type(self.media_type);
        request.set_body(self.body.clone());
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_update(response, |response| {
            self.descriptor.properties_to_json(&self.resource, response)
        })
    }
}

impl ObjectRef<Erased> {
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: JsonObjectProperties,
    ) -> Result<JsonObjectPropertiesUpdate, ObjectError> {
        let descriptor = self
            .descriptor()
            .ok_or_else(|| unsupported(self, "object properties update"))?;
        let body = descriptor.properties_to_xml(properties.media_type, properties.payload)?;
        Ok(JsonObjectPropertiesUpdate {
            resource: self.clone(),
            object_lock: object_lock.clone(),
            media_type: properties.media_type,
            body: body.into_bytes(),
            transport_request: object_lock.transport_request().cloned(),
            descriptor,
        })
    }
}

fn unsupported(object: &ObjectRef<Erased>, capability: &'static str) -> ObjectError {
    ObjectError::UnsupportedCapability {
        object_type: object.object_type().clone(),
        capability,
    }
}

// Shared helper for typed property decoding as both update and query
// receive the same response payload.
fn decode_query<T>(
    response: OperationResponse,
) -> Result<ObjectProperties<T::Properties>, ResponseError>
where
    T: ReadProperties,
{
    if response.status() == StatusCode::NOT_MODIFIED {
        return Err(ResponseError::UnexpectedNotModified);
    }
    if response.status() != StatusCode::OK && !response.status().is_success() {
        return Err(ResponseError::unexpected_status(response.response()));
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or(ResponseError::MissingContentType {
            category: T::CATEGORY,
        })?;
    let media_version = T::Properties::version_from_media_type(content_type).ok_or_else(|| {
        ResponseError::UnsupportedContentType {
            category: T::CATEGORY,
            content_type: content_type.to_owned(),
            supported: T::Properties::SUPPORTED_VERSIONS
                .iter()
                .map(|version| T::Properties::media_type(*version).to_owned())
                .collect(),
        }
    })?;
    let etag = response.entity_tag();
    let payload =
        serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;
    Ok(ObjectProperties {
        media_version,
        etag,
        payload,
    })
}

fn decode_update<P>(
    response: OperationResponse,
    decode: impl FnOnce(OperationResponse) -> Result<P, ResponseError>,
) -> Result<Option<P>, ResponseError> {
    if !response.status().is_success() {
        return Err(ResponseError::unexpected_status(response.response()));
    }
    if response.body().is_empty() || response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    decode(response).map(Some)
}
