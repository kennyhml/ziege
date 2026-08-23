use std::marker::PhantomData;

use http::Method;

use super::{
    AccessControl, AnnotationDefinition, AnyObject, Class, DataDefinition, DataElement, Domain,
    GlobalWorkbenchType, Include, Interface, MetadataExtension, ObjectRef, ObjectType,
    ObjectVersion, Package, Program, PropertyModel, RunCapability, ServiceDefinition,
};
use crate::{
    CategoryId,
    error::{EncodeError, ObjectError, ResponseError},
    operation::{EncodedOperation, Operation, OperationResponse, Owned},
};

/// Runtime capabilities for one modeled ADT object type.
pub(crate) trait RuntimeObjectTypeDescriptor: std::fmt::Debug + Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn category(&self) -> CategoryId;

    fn create_media_type(&self) -> Option<&'static str>;

    fn creation_properties_to_xml(
        &self,
        reference: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError>;

    fn run(&self) -> Option<RunCapability>;

    fn source(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
    ) -> Result<Option<crate::SourceRef>, ObjectError>;

    fn source_component(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
        name: &str,
    ) -> Result<Option<crate::SourceRef>, ObjectError>;

    fn object_structure(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
    ) -> Result<Option<crate::ObjectStructureRef>, ObjectError>;

    fn properties_request(
        &self,
        object: &ObjectRef<()>,
        version: Option<ObjectVersion>,
    ) -> Result<EncodedOperation<Owned>, EncodeError>;

    fn properties_to_json(
        &self,
        object: &ObjectRef<()>,
        response: OperationResponse,
    ) -> Result<AnyObject, ResponseError>;

    fn properties_to_xml(
        &self,
        object: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError>;

    fn properties_media_type(&self, media_type: &str) -> Option<&'static str>;
}

pub(crate) trait RuntimeObjectType: ObjectType {
    fn create_media_type() -> Option<&'static str>;

    fn creation_properties_to_xml(
        reference: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError>;

    fn run() -> Option<RunCapability>;

    fn source_uri(_properties: &Self::Properties) -> Option<&str> {
        None
    }

    fn source_component_uri<'a>(_properties: &'a Self::Properties, _name: &str) -> Option<&'a str> {
        None
    }

    fn has_object_structure() -> bool {
        false
    }
}

pub(crate) struct ObjectTypeDescriptor<T>(PhantomData<fn() -> T>);

impl<T> ObjectTypeDescriptor<T> {
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> std::fmt::Debug for ObjectTypeDescriptor<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ObjectTypeDescriptor")
            .field(&std::any::type_name::<T>())
            .finish()
    }
}

impl<T> RuntimeObjectTypeDescriptor for ObjectTypeDescriptor<T>
where
    T: RuntimeObjectType,
{
    fn object_type(&self) -> GlobalWorkbenchType {
        T::WORKBENCH_TYPE
    }

    fn category(&self) -> CategoryId {
        T::CATEGORY
    }

    fn create_media_type(&self) -> Option<&'static str> {
        T::create_media_type()
    }

    fn creation_properties_to_xml(
        &self,
        reference: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError> {
        T::creation_properties_to_xml(reference, properties)
    }

    fn run(&self) -> Option<RunCapability> {
        T::run()
    }

    fn source(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
    ) -> Result<Option<crate::SourceRef>, ObjectError> {
        let properties: T::Properties = serde_json::from_value(properties.clone())
            .map_err(ObjectError::InvalidPropertiesJson)?;
        T::source_uri(&properties)
            .map(|href| crate::resource::refs::source_from_href(object.clone(), href))
            .transpose()
    }

    fn source_component(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
        name: &str,
    ) -> Result<Option<crate::SourceRef>, ObjectError> {
        let properties: T::Properties = serde_json::from_value(properties.clone())
            .map_err(ObjectError::InvalidPropertiesJson)?;
        T::source_component_uri(&properties, name)
            .map(|href| crate::resource::refs::source_from_href(object.clone(), href))
            .transpose()
    }

    fn object_structure(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
    ) -> Result<Option<crate::ObjectStructureRef>, ObjectError> {
        let properties: T::Properties = serde_json::from_value(properties.clone())
            .map_err(ObjectError::InvalidPropertiesJson)?;
        if !T::has_object_structure() {
            return Ok(None);
        }
        crate::ObjectStructureRef::from_relations(object.clone(), properties.links())
    }

    fn properties_request(
        &self,
        object: &ObjectRef<()>,
        version: Option<ObjectVersion>,
    ) -> Result<EncodedOperation<Owned>, EncodeError> {
        if object.object_type() != &T::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: object.object_type().clone(),
            }
            .into());
        }
        let mut request = EncodedOperation::owned(Method::GET, object.uri().clone());
        if let Some(version) = version {
            request.push_query(ObjectVersion::QUERY_PARAMETER, version.as_str());
        }
        let media_types = T::Properties::SUPPORTED_VERSIONS
            .iter()
            .map(|version| T::Properties::media_type(*version))
            .collect::<Vec<_>>();
        request.set_accepts(&media_types);
        request.set_cache_revalidation(None);
        Ok(request)
    }

    fn properties_to_json(
        &self,
        object: &ObjectRef<()>,
        response: OperationResponse,
    ) -> Result<AnyObject, ResponseError> {
        let resource = object
            .typed::<T>()
            .ok_or_else(|| ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: object.object_type().clone(),
            })?;
        let loaded = resource.query().decode(response)?;
        let (_reference, media_type, etag, properties) = loaded.into_parts();
        Ok(AnyObject::new(
            resource.erase(),
            media_type,
            etag,
            serde_json::to_value(properties)?,
        ))
    }

    fn properties_to_xml(
        &self,
        object: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError> {
        let properties: T::Properties =
            serde_json::from_value(properties).map_err(ObjectError::InvalidPropertiesJson)?;
        properties.to_xml_for(object)
    }

    fn properties_media_type(&self, media_type: &str) -> Option<&'static str> {
        T::Properties::version_from_media_type(media_type).map(T::Properties::media_type)
    }
}

static OBJECT_TYPES: &[&dyn RuntimeObjectTypeDescriptor] = &[
    Program::DESCRIPTOR,
    Include::DESCRIPTOR,
    Class::DESCRIPTOR,
    Package::DESCRIPTOR,
    DataElement::DESCRIPTOR,
    DataDefinition::DESCRIPTOR,
    AccessControl::DESCRIPTOR,
    Interface::DESCRIPTOR,
    MetadataExtension::DESCRIPTOR,
    ServiceDefinition::DESCRIPTOR,
    AnnotationDefinition::DESCRIPTOR,
    Domain::DESCRIPTOR,
];

pub(crate) fn object_type_descriptor(
    object_type: &GlobalWorkbenchType,
) -> Option<&'static dyn RuntimeObjectTypeDescriptor> {
    OBJECT_TYPES
        .iter()
        .copied()
        .find(|descriptor| &descriptor.object_type() == object_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ClassProperties};

    #[test]
    fn registered_object_types_are_unique() {
        for (index, descriptor) in OBJECT_TYPES.iter().enumerate() {
            let object_type = descriptor.object_type();
            assert!(
                OBJECT_TYPES[index + 1..]
                    .iter()
                    .all(|other| other.object_type() != object_type),
                "registered `{object_type}` more than once"
            );
        }
    }

    #[test]
    fn runtime_source_resolution_uses_registered_properties() {
        let properties: ClassProperties = serde_xml_rs::from_str(include_str!(
            "../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml"
        ))
        .unwrap();
        let properties = serde_json::to_value(properties).unwrap();
        let object = ObjectRef::<Class>::new(
            "CL_ADT_URI_MAPPER".to_owned(),
            AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
        )
        .erase();

        let source = Class::DESCRIPTOR
            .source(&object, &properties)
            .unwrap()
            .unwrap();

        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main"
        );
    }
}
