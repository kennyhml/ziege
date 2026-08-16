use std::marker::PhantomData;

use http::{Method, StatusCode, header};

use crate::{
    AccessMode, AdtObject, ObjectError, ObjectLock, TransportNumber,
    client::{Client, ClientState, Ready},
    error::{OperationError, ResponseError},
    objects::{
        ObjectRef, ObjectType, ObjectVersion, PropertyModel, RuntimeObjectTypeDescriptor,
        UpdateProperties,
    },
    operation::{IfNoneMatch, Operation, OperationResponse, Stateful, Stateless},
    protocol::{AdtRequest, EntityTag},
    vocabulary::query_parameter,
};

/// Fetches a versioned, statically typed object-properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesQuery<T>
where
    T: ObjectType,
{
    pub resource: ObjectRef<T>,
    pub version: Option<ObjectVersion>,
}

impl<T> ObjectPropertiesQuery<T>
where
    T: ObjectType,
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
    T: ObjectType,
{
    type Response = AdtObject<T::Properties>;
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
        decode_query::<T>(&self.resource.erase(), response)
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

impl<P> AdtObject<P>
where
    Self: ObjectType<Properties = P>,
    P: PropertyModel,
{
    /// Creates a conditional query using this representation's entity tag.
    pub fn revalidate(&self) -> Option<IfNoneMatch<ObjectPropertiesQuery<Self>>> {
        self.etag
            .clone()
            .map(|etag| self.reference().retag::<Self>().query().if_none_match(etag))
    }
}

/// Fetches one runtime-typed object's properties as JSON.
#[derive(Debug)]
pub struct AdtObjectQuery {
    resource: ObjectRef<()>,
    descriptor: &'static dyn RuntimeObjectTypeDescriptor,
    version: Option<ObjectVersion>,
}

impl AdtObjectQuery {
    pub fn version(mut self, version: ObjectVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn if_none_match(self, etag: EntityTag) -> IfNoneMatch<Self> {
        IfNoneMatch::new(self, etag)
    }
}

impl Operation<Ready> for AdtObjectQuery {
    type Response = AdtObject;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        self.descriptor
            .properties_request(&self.resource, self.version, client)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        self.descriptor.properties_to_json(&self.resource, response)
    }
}

impl ObjectRef<()> {
    pub fn query(&self) -> Result<AdtObjectQuery, ObjectError> {
        let descriptor = self
            .descriptor()
            .ok_or_else(|| unsupported(self, "object properties"))?;
        Ok(AdtObjectQuery {
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
    T: ObjectType,
{
    resource: ObjectRef<()>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
    marker: PhantomData<fn() -> T>,
}

impl<T> ObjectPropertiesUpdate<T>
where
    T: ObjectType,
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
    T: ObjectType,
{
    type Response = Option<AdtObject<T::Properties>>;
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
            decode_query::<T>(&self.resource, response)
        })
    }
}

impl<T> ObjectRef<T>
where
    T: UpdateProperties,
{
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: AdtObject<T::Properties>,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        let resource = self.erase();
        ensure_same_resource(&resource, properties.reference())?;
        validate_payload_identity(&resource, &properties.properties)?;
        validate_update_lock(&resource, object_lock)?;
        let media_type = T::Properties::media_type(properties.media_version());
        let serializer = T::Properties::XML_NAMESPACES.iter().fold(
            serde_xml_rs::SerdeXml::new(),
            |serializer, &(prefix, namespace)| serializer.namespace(prefix, namespace),
        );
        let body = serializer
            .to_string(&properties.properties)
            .map_err(ObjectError::InvalidRequest)?
            .into_bytes();
        Ok(ObjectPropertiesUpdate {
            resource,
            object_lock: object_lock.clone(),
            media_type,
            body,
            transport_request: object_lock.transport_request().cloned(),
            marker: PhantomData,
        })
    }
}

impl<P> AdtObject<P>
where
    Self: UpdateProperties<Properties = P>,
    P: PropertyModel,
{
    /// Replaces this loaded object's properties under the supplied lock.
    pub fn update(
        self,
        object_lock: &ObjectLock,
    ) -> Result<ObjectPropertiesUpdate<Self>, ObjectError> {
        let reference = self.reference().clone();
        reference.retag::<Self>().update(object_lock, self)
    }
}

/// Replaces one runtime-typed object's JSON properties representation.
#[derive(Debug)]
pub struct AdtObjectUpdate {
    resource: ObjectRef<()>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
    descriptor: &'static dyn RuntimeObjectTypeDescriptor,
}

impl AdtObjectUpdate {
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }
}

impl<S> Operation<S> for AdtObjectUpdate
where
    S: ClientState,
{
    type Response = Option<AdtObject>;
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

impl ObjectRef<()> {
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: AdtObject,
    ) -> Result<AdtObjectUpdate, ObjectError> {
        ensure_same_resource(self, &properties.reference().erase())?;
        validate_update_lock(self, object_lock)?;
        let descriptor = self
            .descriptor()
            .ok_or_else(|| unsupported(self, "object properties update"))?;
        let media_type = descriptor
            .properties_media_type(properties.media_type())
            .ok_or_else(|| ObjectError::UnsupportedPropertiesMediaType {
                object_type: self.object_type().clone(),
                media_type: properties.media_type().to_owned(),
            })?;
        let body = descriptor.properties_to_xml(self, media_type, properties.properties)?;
        Ok(AdtObjectUpdate {
            resource: self.clone(),
            object_lock: object_lock.clone(),
            media_type,
            body: body.into_bytes(),
            transport_request: object_lock.transport_request().cloned(),
            descriptor,
        })
    }
}

fn unsupported(object: &ObjectRef<()>, capability: &'static str) -> ObjectError {
    ObjectError::UnsupportedCapability {
        object_type: object.object_type().clone(),
        capability,
    }
}

fn ensure_same_resource(
    expected: &ObjectRef<()>,
    actual: &ObjectRef<()>,
) -> Result<(), ObjectError> {
    if expected.uri() != actual.uri()
        || expected.name() != actual.name()
        || expected.object_type() != actual.object_type()
    {
        return Err(ObjectError::UnexpectedObjectReference {
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn validate_payload_identity<P>(resource: &ObjectRef<()>, properties: &P) -> Result<(), ObjectError>
where
    P: PropertyModel,
{
    if properties.object_name() != resource.name() {
        return Err(ObjectError::UnexpectedObjectReference {
            expected: resource.to_string(),
            actual: format!(
                "{} ({})",
                properties.object_name(),
                properties.object_type()
            ),
        });
    }
    if properties.object_type() != resource.object_type() {
        return Err(ObjectError::UnexpectedObjectType {
            expected: resource.object_type().clone(),
            actual: properties.object_type().clone(),
        });
    }
    Ok(())
}

fn validate_update_lock(
    resource: &ObjectRef<()>,
    object_lock: &ObjectLock,
) -> Result<(), ObjectError> {
    if resource.uri() != object_lock.object().uri()
        || resource.name() != object_lock.object().name()
        || resource.object_type() != object_lock.object().object_type()
    {
        return Err(ObjectError::ObjectLockMismatch {
            expected: resource.to_string(),
            actual: object_lock.object().to_string(),
        });
    }
    if object_lock.access_mode() != AccessMode::Modify {
        return Err(ObjectError::ObjectLockNotModifiable);
    }
    Ok(())
}

// Shared helper for typed property decoding as both update and query
// receive the same response payload.
fn decode_query<T>(
    resource: &ObjectRef<()>,
    response: OperationResponse,
) -> Result<AdtObject<T::Properties>, ResponseError>
where
    T: ObjectType,
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
    let payload: T::Properties =
        serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;
    validate_payload_identity(resource, &payload)?;
    Ok(AdtObject::new(
        resource.clone(),
        T::Properties::media_type(media_version),
        etag,
        payload,
    ))
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
