use http::{Method, StatusCode};

use crate::{
    AnyObject, Object, ObjectError, ObjectLock, TransportNumber,
    error::{EncodeError, ResponseError},
    objects::{
        ObjectRef, ObjectType, ObjectVersion, PropertyModel, RuntimeObjectTypeDescriptor,
        UpdateProperties,
    },
    operation::{
        EncodedOperation, IfNoneMatch, Operation, OperationResponse, Owned, Stateful, Stateless,
    },
    protocol::EntityTag,
};

use super::{locking::LOCK_HANDLE_QUERY, transports::TRANSPORT_REQUEST_QUERY};

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

    fn build_request(&self) -> Result<EncodedOperation<Owned>, EncodeError> {
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

impl<T> Operation for ObjectPropertiesQuery<T>
where
    T: ObjectType,
{
    type Response = Object<T>;
    type Kind = Stateless;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        self.build_request()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_properties(&self.resource, response)
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
        self.etag
            .clone()
            .map(|etag| self.reference().query().if_none_match(etag))
    }
}

impl Operation for ObjectPropertiesQuery<()> {
    type Response = AnyObject;
    type Kind = Stateless;
    type Target = Owned;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
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
    type Response = Option<Object<T>>;
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
        decode_properties(&self.resource, response).map(Some)
    }
}

impl<T> ObjectRef<T>
where
    T: UpdateProperties,
{
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: Object<T>,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        if !self.same_identity(properties.reference()) {
            return Err(ObjectError::UnexpectedObjectReference {
                expected: self.to_string(),
                actual: properties.reference().to_string(),
            });
        }
        object_lock.validate_modification_for(self)?;
        let media_type = T::Properties::media_type(properties.media_version());
        let body = properties.properties.to_xml_for(self)?;
        Ok(ObjectPropertiesUpdate {
            resource: self.clone(),
            object_lock: object_lock.clone(),
            media_type,
            body,
            transport_request: object_lock.transport_request().cloned(),
        })
    }
}

impl<T: UpdateProperties> Object<T> {
    /// Replaces this loaded object's properties under the supplied lock.
    pub fn update(
        self,
        object_lock: &ObjectLock,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        let reference = self.reference().clone();
        reference.update(object_lock, self)
    }
}

impl Operation for ObjectPropertiesUpdate<()> {
    type Response = Option<AnyObject>;
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
        properties: AnyObject,
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
    resource: &ObjectRef<T>,
    response: OperationResponse,
) -> Result<Object<T>, ResponseError>
where
    T: ObjectType,
{
    if response.status() == StatusCode::NOT_MODIFIED {
        return Err(ResponseError::UnexpectedNotModified);
    }
    response.require_success()?;
    let supported = T::Properties::SUPPORTED_VERSIONS
        .iter()
        .map(|version| T::Properties::media_type(*version))
        .collect::<Vec<_>>();
    let content_type = response.require_content_type(&supported)?;
    let media_version = T::Properties::version_from_media_type(content_type)
        .expect("validated properties Content-Type must have a media version");
    let etag = response.entity_tag();
    let payload = T::Properties::from_xml_for(response.body(), resource)?;
    Ok(Object::new(
        resource.clone(),
        T::Properties::media_type(media_version),
        etag,
        payload,
    ))
}
