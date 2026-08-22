use http::Method;

use crate::{
    Advertised, AnyObject, CategoryId, EncodeError, EncodedOperation, Object, ObjectError,
    ObjectRef, Operation, OperationResponse, PropertyModel, ResponseError, Stateless,
    TransportNumber,
    objects::{Create, CreationPropertyModel},
    operation::CollectionTarget,
};

use super::{properties::decode_properties, transports::TRANSPORT_REQUEST_QUERY};

/// Creates a repository object from a family-specific creation payload.
///
/// Successful responses without a representation decode to `None`. Object
/// families that return their properties decode to a loaded object.
///
/// The operation supports both typed and generic JSON payloads. In a typed
/// context, the response is also typed. Otherwise, it is parsed through
/// the object descriptor but is then normalized to the JSON response.
///
/// No single handler can be named, because each object type implements
/// its own handler. This also causes the inconsistency in responses.
#[derive(Debug)]
pub struct CreateObjectRequest<T, P> {
    reference: ObjectRef<T>,
    payload: P,
    media_type: &'static str,
    /// Transport request for the creation if not a local object
    transport_request: Option<TransportNumber>,
}

impl<T, P> CreateObjectRequest<T, P> {
    /// The transport this object will become a part of, not required for local objects.
    pub fn transport(&mut self, transport_request: TransportNumber) -> &mut Self {
        self.transport_request = Some(transport_request);
        self
    }

    /// Creates the request and attaches the normalized body bytes.
    fn build_request(
        &self,
        object_category: CategoryId,
        body: Vec<u8>,
    ) -> Result<EncodedOperation<Advertised>, EncodeError> {
        let mut request = CollectionTarget::new(object_category).operation(Method::POST);
        request.set_content_type(self.media_type);
        request.set_body(body);
        if let Some(transport) = &self.transport_request {
            request.push_query(TRANSPORT_REQUEST_QUERY, transport.as_str());
        }
        Ok(request)
    }
}

/// Implementation for typed objects
impl<T, P> Operation for CreateObjectRequest<T, P>
where
    T: Create<CreateProperties = P>,
    P: PropertyModel,
{
    type Kind = Stateless;
    type Response = Option<Object<T>>;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        self.build_request(T::CATEGORY, self.payload.to_xml_for(&self.reference)?)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }
        decode_properties(&self.reference, response).map(Some)
    }
}

/// Implementation for runtime-typed objects.
impl Operation for CreateObjectRequest<(), serde_json::Value> {
    type Kind = Stateless;
    type Response = Option<AnyObject>;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        let descriptor = self.reference.require_descriptor()?;
        self.build_request(
            descriptor.category(),
            descriptor.creation_properties_to_xml(&self.reference, self.payload.clone())?,
        )
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_success()?;
        if response.body().is_empty() {
            return Ok(None);
        }
        self.reference
            .require_descriptor()?
            .properties_to_json(&self.reference, response)
            .map(Some)
    }
}

/// Implementation for typed objects
impl<T> ObjectRef<T>
where
    T: Create,
{
    /// Creates a typed [`CreateObjectRequest`] for this object.
    ///
    /// The properties to create the object with should be set prior
    /// to calling this method. Identitify related properties, such as
    /// name and object type, are set from the identify of the object
    /// reference the method is called on.
    pub fn create(
        &self,
        mut payload: T::CreateProperties,
    ) -> CreateObjectRequest<T, T::CreateProperties> {
        payload.set_identity(self);
        CreateObjectRequest {
            reference: self.clone(),
            payload,
            transport_request: None,
            media_type: T::CreateProperties::media_type(T::CREATE_VERSION),
        }
    }
}

/// Implementation for erased object references.
impl ObjectRef<()> {
    /// Creates a runtime [`CreateObjectRequest`] for this object.
    ///
    /// The properties to create the object with should be set prior
    /// to calling this method. Identitify related properties, such as
    /// name and object type, are set from the identify of the object
    /// reference the method is called on when the request is built.
    pub fn create(
        &self,
        payload: serde_json::Value,
    ) -> Result<CreateObjectRequest<(), serde_json::Value>, ObjectError> {
        let descriptor = self
            .descriptor()
            .ok_or_else(|| ObjectError::UnsupportedObjectType {
                object_type: self.object_type().clone(),
            })?;
        let media_type =
            descriptor
                .create_media_type()
                .ok_or_else(|| ObjectError::UnsupportedCapability {
                    object_type: self.object_type().clone(),
                    capability: "object creation",
                })?;
        Ok(CreateObjectRequest {
            reference: self.clone(),
            payload,
            transport_request: None,
            media_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use super::*;
    use crate::{
        AbapLanguageVersion, AdtResponse, AdtUri, AdvertisedObjectReference, Class, ClassCategory,
        ClassCreateProperties, ClassPropertiesVersion, ClassTemplate, ObjectType, Package,
    };

    const CLASS_XML: &[u8] = include_bytes!("../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");

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
            typed_request.headers()[header::CONTENT_TYPE],
            ClassPropertiesVersion::V4.media_type()
        );
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

        assert_eq!(created.properties.name, "CL_ADT_URI_MAPPER");
        assert_eq!(
            runtime_created.properties["@adtcore:name"],
            "CL_ADT_URI_MAPPER"
        );
    }
}
