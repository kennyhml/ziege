use http::{Method, StatusCode, header};

use crate::{
    AccessMode, ObjectError, ObjectLock, ObjectPropertiesUpdateResult, TransportNumber,
    client::{Client, ClientState, Ready},
    compatibility::MediaVersionNegotiation,
    error::{OperationError, ResponseError},
    objects::{ObjectRef, ObjectVersion, ReadProperties, UpdateProperties, WritableProperties},
    operation::{IfNoneMatch, Operation, OperationResponse, Stateful, Stateless},
    protocol::{AdtRequest, EntityTag},
    target::CollectionTarget,
    vocabulary::query_parameter,
};

/// Fetches a versioned ADT object-properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    /// The typed object reference whose properties will be fetched.
    pub resource: ObjectRef<T>,

    /// Media-type versions in descending caller preference.
    pub priority: Vec<T::MediaVersion>,

    /// The repository-object version to request.
    pub version: Option<ObjectVersion>,
}

impl<T> ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    /// Creates an unconditional properties query using the family's default priority.
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

    /// Selects the repository-object version to request.
    pub fn version(mut self, version: ObjectVersion) -> Self {
        self.version = Some(version);
        self
    }

    /// Makes this query conditional on the supplied properties ETag.
    ///
    /// This setter must be called last.
    pub fn if_none_match(self, etag: EntityTag) -> IfNoneMatch<Self> {
        IfNoneMatch::new(self, etag)
    }
}

impl<T> Operation<Ready> for ObjectPropertiesQuery<T>
where
    T: ReadProperties,
{
    type Response = T::Properties;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let collection = CollectionTarget::new(T::CATEGORY).collection(client)?;
        let accept = crate::negotiate(&self.priority, collection.accepted_media_types())?;

        let mut request = AdtRequest::new(Method::GET, self.resource.uri().clone());
        if let Some(version) = self.version {
            request.push_query(query_parameter::VERSION, version.as_str());
        }
        request.set_accept(accept.media_type());
        request.set_cache_revalidation(None);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if response.status() == StatusCode::NOT_MODIFIED {
            return Err(ResponseError::UnexpectedNotModified);
        }
        if response.status() != StatusCode::OK {
            return Err(ResponseError::unexpected_status(response.response()));
        }

        T::Properties::try_from(RawObjectProperties::from_response(
            &self.resource,
            response,
        )?)
    }
}

impl<T> ObjectRef<T>
where
    T: ReadProperties,
{
    /// Creates a property query for this object.
    pub fn query(&self) -> ObjectPropertiesQuery<T> {
        ObjectPropertiesQuery::new(self.clone())
    }
}

/// Replaces an object's complete properties representation.
#[derive(Debug)]
pub struct ObjectPropertiesUpdate<T>
where
    T: ReadProperties,
{
    resource: ObjectRef<T>,
    object_lock: ObjectLock,
    media_type: &'static str,
    body: Vec<u8>,
    transport_request: Option<TransportNumber>,
}

impl<T> ObjectPropertiesUpdate<T>
where
    T: ReadProperties,
{
    /// Records this update in the supplied transport request.
    ///
    /// This replaces any transport request inherited from the lock.
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
    type Response = ObjectPropertiesUpdateResult<T::Properties>;
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
        if !response.status().is_success() {
            return Err(ResponseError::unexpected_status(response.response()));
        }

        let etag = response.entity_tag();
        let properties =
            if response.body().is_empty() || response.status() == StatusCode::NO_CONTENT {
                None
            } else {
                Some(
                    RawObjectProperties::from_response(&self.resource, response)
                        .and_then(T::Properties::try_from)?,
                )
            };
        Ok(ObjectPropertiesUpdateResult { properties, etag })
    }
}

impl<T> ObjectRef<T>
where
    T: UpdateProperties,
    T::Properties: WritableProperties<T>,
{
    /// Replaces this object's properties using a modification lock.
    pub fn update(
        &self,
        object_lock: &ObjectLock,
        properties: T::Properties,
    ) -> Result<ObjectPropertiesUpdate<T>, ObjectError> {
        if &self.erase() != object_lock.object() {
            return Err(ObjectError::ObjectLockMismatch {
                expected: self.to_string(),
                actual: object_lock.object().to_string(),
            });
        }
        if object_lock.access_mode() != AccessMode::Modify {
            return Err(ObjectError::ObjectLockNotModifiable);
        }

        let media_version = properties.media_version();
        Ok(ObjectPropertiesUpdate {
            resource: self.clone(),
            object_lock: object_lock.clone(),
            media_type: media_version.media_type(),
            body: properties.to_xml(self)?.into_bytes(),
            transport_request: object_lock.transport_request().cloned(),
        })
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
    fn from_response(
        resource: &ObjectRef<T>,
        response: OperationResponse,
    ) -> Result<Self, ResponseError> {
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

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::{
        AdtResponse, AdtUri, DataElement, DataElementProperties, DataElementPropertiesVersion,
        Initial, Transport, TransportError,
    };

    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../tests/fixtures/data-element-ztfrwtfrt-v2.xml");

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn reference(name: &str) -> ObjectRef<DataElement> {
        ObjectRef::<DataElement>::for_test(
            name,
            AdtUri::parse(&format!(
                "/sap/bc/adt/ddic/dataelements/{}",
                name.to_ascii_lowercase()
            ))
            .unwrap(),
        )
    }

    fn properties() -> DataElementProperties {
        DataElementProperties::try_from(RawObjectProperties {
            resource: reference("ZTFRWTFRT"),
            version: DataElementPropertiesVersion::V2,
            body: DATA_ELEMENT_XML.to_vec(),
            etag: None,
        })
        .unwrap()
    }

    #[test]
    fn update_inherits_and_can_override_the_locks_transport() {
        let reference = reference("ZTFRWTFRT");
        let object_lock = ObjectLock::for_test_with_transport(
            reference.erase(),
            AccessMode::Modify,
            "A4HK900001",
        );
        let client = Client::<Initial>::new(UnusedTransport);

        let inherited = reference
            .update(&object_lock, properties())
            .unwrap()
            .request(&client)
            .unwrap();
        assert_eq!(
            inherited.query(),
            [
                ("lockHandle".to_owned(), "LOCK-HANDLE".to_owned()),
                ("corrNr".to_owned(), "A4HK900001".to_owned()),
            ]
        );

        let overridden = reference
            .update(&object_lock, properties())
            .unwrap()
            .transport("A4HK900002")
            .request(&client)
            .unwrap();
        assert_eq!(
            overridden.query(),
            [
                ("lockHandle".to_owned(), "LOCK-HANDLE".to_owned()),
                ("corrNr".to_owned(), "A4HK900002".to_owned()),
            ]
        );
    }

    #[test]
    fn update_requires_a_matching_modification_lock() {
        let data_element = reference("ZTFRWTFRT");
        let other = reference("ZOTHER");
        let other_lock = ObjectLock::for_test(other.erase(), AccessMode::Modify);
        let show_lock = ObjectLock::for_test(data_element.erase(), AccessMode::Show);

        assert!(matches!(
            data_element.update(&other_lock, properties()),
            Err(ObjectError::ObjectLockMismatch { .. })
        ));
        assert!(matches!(
            data_element.update(&show_lock, properties()),
            Err(ObjectError::ObjectLockNotModifiable)
        ));
    }
}
