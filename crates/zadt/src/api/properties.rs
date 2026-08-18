use http::{Method, StatusCode};

use crate::{
    AdtObject, ObjectError, ObjectLock, TransportNumber,
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

/// Fetches a versioned object-properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesQuery<T> {
    pub resource: ObjectRef<T>,
    pub version: Option<ObjectVersion>,
}

impl<T> ObjectPropertiesQuery<T> {
    fn descriptor(&self) -> Result<&'static dyn RuntimeObjectTypeDescriptor, ObjectError> {
        self.resource
            .descriptor()
            .ok_or_else(|| self.resource.unsupported_capability("object properties"))
    }

    fn build_request(&self) -> Result<AdtRequest, OperationError> {
        self.descriptor()?
            .properties_request(&self.resource.erase(), self.version)
    }

    pub fn version(mut self, version: ObjectVersion) -> Self {
        self.version = Some(version);
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
            version: None,
        }
    }
}

impl<T> Operation<Ready> for ObjectPropertiesQuery<T>
where
    T: ObjectType,
{
    type Response = AdtObject<T::Properties>;
    type Kind = Stateless;

    fn request(&self, _client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        self.build_request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_properties::<T>(&self.resource.erase(), response)
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

impl Operation<Ready> for ObjectPropertiesQuery<()> {
    type Response = AdtObject;
    type Kind = Stateless;

    fn request(&self, _client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        self.build_request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        self.descriptor()?
            .properties_to_json(&self.resource, response)
    }
}

impl ObjectRef<()> {
    pub fn query(&self) -> Result<ObjectPropertiesQuery<()>, ObjectError> {
        self.descriptor()
            .ok_or_else(|| self.unsupported_capability("object properties"))?;
        Ok(ObjectPropertiesQuery {
            resource: self.clone(),
            version: None,
        })
    }
}

/// Replaces an object's properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesUpdate<T> {
    resource: ObjectRef<T>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
}

impl<T> ObjectPropertiesUpdate<T> {
    #[must_use]
    pub fn transport(mut self, transport_request: impl Into<TransportNumber>) -> Self {
        self.transport_request = Some(transport_request.into());
        self
    }

    fn build_request(&self) -> Result<AdtRequest, OperationError> {
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

impl<S, T> Operation<S> for ObjectPropertiesUpdate<T>
where
    S: ClientState,
    T: ObjectType,
{
    type Response = Option<AdtObject<T::Properties>>;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        self.build_request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if !response.status().is_success() {
            return Err(ResponseError::unexpected_status(response.response()));
        }
        if response.body().is_empty() {
            return Ok(None);
        }
        decode_properties::<T>(&self.resource.erase(), response).map(Some)
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
        let erased = self.erase();
        if !erased.same_identity(properties.reference()) {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: erased.to_string(),
                actual: properties.reference().to_string(),
            });
        }
        object_lock.validate_modification_for(&erased)?;
        let media_type = T::Properties::media_type(properties.media_version());
        let body = properties.properties.to_xml_for(&erased)?;
        Ok(ObjectPropertiesUpdate {
            resource: self.clone(),
            object_lock: object_lock.clone(),
            media_type,
            body,
            transport_request: object_lock.transport_request().cloned(),
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

impl<S> Operation<S> for ObjectPropertiesUpdate<()>
where
    S: ClientState,
{
    type Response = Option<AdtObject>;
    type Kind = Stateful;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        self.build_request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if !response.status().is_success() {
            return Err(ResponseError::unexpected_status(response.response()));
        }
        if response.body().is_empty() {
            return Ok(None);
        }
        let descriptor = self.resource.descriptor().ok_or_else(|| {
            self.resource
                .unsupported_capability("object properties update")
        })?;
        descriptor
            .properties_to_json(&self.resource, response)
            .map(Some)
    }
}

impl ObjectRef<()> {
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: AdtObject,
    ) -> Result<ObjectPropertiesUpdate<()>, ObjectError> {
        if !self.same_identity(properties.reference()) {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: self.to_string(),
                actual: properties.reference().to_string(),
            });
        }
        object_lock.validate_modification_for(self)?;
        let descriptor = self
            .descriptor()
            .ok_or_else(|| self.unsupported_capability("object properties update"))?;
        let media_type = descriptor
            .properties_media_type(properties.media_type())
            .ok_or_else(|| ObjectError::UnsupportedPropertiesMediaType {
                object_type: self.object_type().clone(),
                media_type: properties.media_type().to_owned(),
            })?;
        let body = descriptor.properties_to_xml(self, media_type, properties.properties)?;
        Ok(ObjectPropertiesUpdate {
            resource: self.clone(),
            object_lock: object_lock.clone(),
            media_type,
            body,
            transport_request: object_lock.transport_request().cloned(),
        })
    }
}

/// A helper function to decode the object properties from an [`OperationResponse`]
pub(crate) fn decode_properties<T>(
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
    let content_type = response.content_type(T::CATEGORY)?;
    let media_version = T::Properties::require_version_from_media_type(content_type, T::CATEGORY)?;
    let etag = response.entity_tag();
    let payload = T::Properties::from_xml_for(response.body(), resource)?;
    Ok(AdtObject::new(
        resource.clone(),
        T::Properties::media_type(media_version),
        etag,
        payload,
    ))
}
