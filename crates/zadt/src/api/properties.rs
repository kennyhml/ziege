use std::marker::PhantomData;

use http::{Method, StatusCode, header};

use crate::{
    AccessMode, Erased, ObjectError, ObjectLock, TransportNumber,
    client::{Client, ClientState, Ready},
    compatibility::MediaVersionNegotiation,
    error::{OperationError, ResponseError},
    objects::{ObjectRef, ObjectVersion, ReadProperties, UpdateProperties, WritableProperties},
    operation::{IfNoneMatch, Operation, OperationResponse, Stateful, Stateless},
    protocol::{AdtRequest, EntityTag},
    target::CollectionTarget,
    vocabulary::query_parameter,
};

/// A fetched object-properties payload and its transport metadata.
#[derive(Clone, Debug)]
pub struct ObjectProperties<T>
where
    T: ReadProperties,
{
    object: ObjectRef<T>,
    media_version: T::MediaVersion,
    etag: Option<EntityTag>,
    payload: T::Properties,
}

impl<T> ObjectProperties<T>
where
    T: ReadProperties,
{
    pub(crate) fn new(
        object: ObjectRef<T>,
        media_version: T::MediaVersion,
        etag: Option<EntityTag>,
        payload: T::Properties,
    ) -> Self {
        Self {
            object,
            media_version,
            etag,
            payload,
        }
    }

    /// Returns the object whose properties were fetched.
    pub fn object(&self) -> &ObjectRef<T> {
        &self.object
    }

    /// Returns the media-type version used by this representation.
    pub fn media_version(&self) -> T::MediaVersion {
        self.media_version
    }

    /// Returns the response entity tag, when present.
    pub fn etag(&self) -> Option<&EntityTag> {
        self.etag.as_ref()
    }

    /// Returns the complete ADT properties payload.
    pub fn payload(&self) -> &T::Properties {
        &self.payload
    }

    /// Returns the mutable ADT properties payload.
    pub fn payload_mut(&mut self) -> &mut T::Properties {
        &mut self.payload
    }

    /// Consumes the envelope and returns its payload.
    pub fn into_payload(self) -> T::Properties {
        self.payload
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ObjectRef<T>,
        T::MediaVersion,
        Option<EntityTag>,
        T::Properties,
    ) {
        (self.object, self.media_version, self.etag, self.payload)
    }
}

/// A fetched object-properties payload exposed through its runtime JSON form.
#[derive(Clone, Debug)]
pub struct JsonObjectProperties {
    object: ObjectRef<Erased>,
    media_type: &'static str,
    etag: Option<EntityTag>,
    payload: serde_json::Value,
}

/// The canonical properties returned after an object-properties update.
#[derive(Debug)]
pub struct ObjectPropertiesUpdateResult<P> {
    pub properties: Option<P>,
    pub etag: Option<EntityTag>,
}

impl JsonObjectProperties {
    pub(crate) fn new(
        object: ObjectRef<Erased>,
        media_type: &'static str,
        etag: Option<EntityTag>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            object,
            media_type,
            etag,
            payload,
        }
    }

    pub fn object(&self) -> &ObjectRef<Erased> {
        &self.object
    }

    pub fn media_type(&self) -> &'static str {
        self.media_type
    }

    pub fn etag(&self) -> Option<&EntityTag> {
        self.etag.as_ref()
    }

    pub fn payload(&self) -> &serde_json::Value {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut serde_json::Value {
        &mut self.payload
    }
}

/// Fetches a versioned, statically typed object-properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    pub resource: ObjectRef<T>,
    pub priority: Vec<T::MediaVersion>,
    pub version: Option<ObjectVersion>,
}

impl<T> ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    pub fn new(resource: ObjectRef<T>) -> Self {
        Self {
            resource,
            priority: T::MediaVersion::SUPPORTED.to_vec(),
            version: None,
        }
    }

    pub fn priority(mut self, priority: impl Into<Vec<T::MediaVersion>>) -> Self {
        self.priority = priority.into();
        self
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
    type Response = ObjectProperties<T>;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        properties_request::<T>(&self.resource, &self.priority, self.version, client)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        let raw = RawObjectProperties::from_response(&self.resource, response)?;
        let media_version = raw.version;
        let etag = raw.etag.clone();
        let payload = T::Properties::try_from(raw)?;
        Ok(ObjectProperties::new(
            self.resource.clone(),
            media_version,
            etag,
            payload,
        ))
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
    version: Option<ObjectVersion>,
}

impl JsonObjectPropertiesQuery {
    pub(crate) fn new(resource: ObjectRef<Erased>) -> Self {
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

impl Operation<Ready> for JsonObjectPropertiesQuery {
    type Response = JsonObjectProperties;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        self.resource
            .descriptor()
            .ok_or_else(|| {
                OperationError::Response(ResponseError::Object(unsupported(
                    &self.resource,
                    "object properties",
                )))
            })?
            .properties_request(&self.resource, self.version, client)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        self.resource
            .descriptor()
            .ok_or_else(|| unsupported(&self.resource, "object properties"))?
            .properties_to_json(&self.resource, response)
    }
}

impl ObjectRef<Erased> {
    pub fn query(&self) -> Result<JsonObjectPropertiesQuery, ObjectError> {
        self.descriptor()
            .ok_or_else(|| unsupported(self, "object properties"))?;
        Ok(JsonObjectPropertiesQuery::new(self.clone()))
    }
}

#[derive(Debug)]
struct PropertiesUpdateRequest {
    resource: ObjectRef<Erased>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
}

impl PropertiesUpdateRequest {
    fn request(&self) -> Result<AdtRequest, OperationError> {
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
}

/// Replaces an object's statically typed properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesUpdate<T>
where
    T: ReadProperties,
{
    request: PropertiesUpdateRequest,
    resource: ObjectRef<T>,
    marker: PhantomData<fn() -> T>,
}

impl<T> ObjectPropertiesUpdate<T>
where
    T: ReadProperties,
{
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.request.transport_request = Some(transport_request.into());
        self
    }
}

impl<S, T> Operation<S> for ObjectPropertiesUpdate<T>
where
    S: ClientState,
    T: ReadProperties,
{
    type Response = ObjectPropertiesUpdateResult<ObjectProperties<T>>;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        self.request.request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_update(response, |response| {
            let raw = RawObjectProperties::from_response(&self.resource, response)?;
            let media_version = raw.version;
            let etag = raw.etag.clone();
            let payload = T::Properties::try_from(raw)?;
            Ok(ObjectProperties::new(
                self.resource.clone(),
                media_version,
                etag,
                payload,
            ))
        })
    }
}

impl<T> ObjectRef<T>
where
    T: UpdateProperties,
    T::Properties: WritableProperties<T>,
{
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: ObjectProperties<T>,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        validate_update(self.uri(), object_lock)?;
        if properties.object != *self {
            return Err(ObjectError::ObjectPropertiesMismatch {
                expected: self.to_string(),
                actual: properties.object.to_string(),
            });
        }

        let media_type = properties.media_version.media_type();
        let body = properties.payload.to_xml(self)?.into_bytes();
        Ok(ObjectPropertiesUpdate {
            request: PropertiesUpdateRequest {
                resource: self.erase(),
                object_lock: object_lock.clone(),
                media_type,
                body,
                transport_request: object_lock.transport_request().cloned(),
            },
            resource: self.clone(),
            marker: PhantomData,
        })
    }
}

/// Replaces one runtime-typed object's JSON properties representation.
#[derive(Debug)]
pub struct JsonObjectPropertiesUpdate {
    request: PropertiesUpdateRequest,
}

impl JsonObjectPropertiesUpdate {
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.request.transport_request = Some(transport_request.into());
        self
    }
}

impl<S> Operation<S> for JsonObjectPropertiesUpdate
where
    S: ClientState,
{
    type Response = ObjectPropertiesUpdateResult<JsonObjectProperties>;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        self.request.request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_update(response, |response| {
            self.request
                .resource
                .descriptor()
                .ok_or_else(|| unsupported(&self.request.resource, "object properties"))?
                .properties_to_json(&self.request.resource, response)
        })
    }
}

impl ObjectRef<Erased> {
    pub fn update_properties(
        &self,
        object_lock: &ObjectLock,
        properties: JsonObjectProperties,
    ) -> Result<JsonObjectPropertiesUpdate, ObjectError> {
        validate_update(self.uri(), object_lock)?;
        if properties.object != *self {
            return Err(ObjectError::ObjectPropertiesMismatch {
                expected: self.to_string(),
                actual: properties.object.to_string(),
            });
        }
        let descriptor = self
            .descriptor()
            .ok_or_else(|| unsupported(self, "object properties update"))?;
        let body = descriptor.properties_to_xml(self, properties.media_type, properties.payload)?;
        Ok(JsonObjectPropertiesUpdate {
            request: PropertiesUpdateRequest {
                resource: self.clone(),
                object_lock: object_lock.clone(),
                media_type: properties.media_type,
                body: body.into_bytes(),
                transport_request: object_lock.transport_request().cloned(),
            },
        })
    }
}

fn properties_request<T>(
    resource: &ObjectRef<T>,
    priority: &[T::MediaVersion],
    version: Option<ObjectVersion>,
    client: &Client<Ready>,
) -> Result<AdtRequest, OperationError>
where
    T: ReadProperties,
{
    let collection = CollectionTarget::new(T::CATEGORY).collection(client)?;
    let accept = crate::negotiate(priority, collection.accepted_media_types())?;
    let mut request = AdtRequest::new(Method::GET, resource.uri().clone());
    if let Some(version) = version {
        request.push_query(query_parameter::VERSION, version.as_str());
    }
    request.set_accept(accept.media_type());
    request.set_cache_revalidation(None);
    Ok(request)
}

fn decode_update<P>(
    response: OperationResponse,
    decode: impl FnOnce(OperationResponse) -> Result<P, ResponseError>,
) -> Result<ObjectPropertiesUpdateResult<P>, ResponseError> {
    if !response.status().is_success() {
        return Err(ResponseError::unexpected_status(response.response()));
    }
    let etag = response.entity_tag();
    let properties = if response.body().is_empty() || response.status() == StatusCode::NO_CONTENT {
        None
    } else {
        Some(decode(response)?)
    };
    Ok(ObjectPropertiesUpdateResult { properties, etag })
}

fn validate_update(resource: &crate::AdtUri, object_lock: &ObjectLock) -> Result<(), ObjectError> {
    if resource != object_lock.object().uri() {
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

fn unsupported(object: &ObjectRef<Erased>, capability: &'static str) -> ObjectError {
    ObjectError::UnsupportedCapability {
        object_type: object.object_type().clone(),
        capability,
    }
}

/// The typed context needed to decode one object-properties representation.
#[doc(hidden)]
pub struct RawObjectProperties<T>
where
    T: ReadProperties,
{
    pub resource: ObjectRef<T>,
    pub version: T::MediaVersion,
    pub body: Vec<u8>,
    pub etag: Option<EntityTag>,
}

impl<T> RawObjectProperties<T>
where
    T: ReadProperties,
{
    pub(crate) fn from_response(
        resource: &ObjectRef<T>,
        response: OperationResponse,
    ) -> Result<Self, ResponseError> {
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
        let media_version = T::MediaVersion::from_media_type(content_type).ok_or_else(|| {
            ResponseError::UnsupportedContentType {
                category: T::CATEGORY,
                content_type: content_type.to_owned(),
                supported: T::MediaVersion::SUPPORTED
                    .iter()
                    .map(|version| version.media_type().to_owned())
                    .collect(),
            }
        })?;
        Ok(Self {
            resource: resource.clone(),
            version: media_version,
            etag: response.entity_tag(),
            body: response.into_body(),
        })
    }
}
