use std::marker::PhantomData;

use super::{
    AdtObject, Class, DataElement, GlobalWorkbenchType, Include, ObjectRef, ObjectType,
    ObjectVersion, Package, Program, PropertyModel, RunCapability,
};
use crate::{
    client::{Client, Ready},
    error::{ObjectError, OperationError, ResponseError},
    operation::{Operation, OperationResponse},
    protocol::AdtRequest,
    vocabulary::CategoryId,
};

/// Runtime capabilities for one modeled ADT object type.
pub(crate) trait RuntimeObjectTypeDescriptor: std::fmt::Debug + Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn category(&self) -> CategoryId;

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

    fn properties_request(
        &self,
        object: &ObjectRef<()>,
        version: Option<ObjectVersion>,
        client: &Client<Ready>,
    ) -> Result<AdtRequest, OperationError>;

    fn properties_to_json(
        &self,
        object: &ObjectRef<()>,
        response: OperationResponse,
    ) -> Result<AdtObject, ResponseError>;

    fn properties_to_xml(
        &self,
        object: &ObjectRef<()>,
        media_type: &str,
        properties: serde_json::Value,
    ) -> Result<String, ObjectError>;

    fn properties_media_type(&self, media_type: &str) -> Option<&'static str>;
}

pub(crate) trait RuntimeObjectType: ObjectType {
    fn run() -> Option<RunCapability>;

    fn source_uri(_properties: &Self::Properties) -> Option<&str> {
        None
    }

    fn source_component_uri<'a>(_properties: &'a Self::Properties, _name: &str) -> Option<&'a str> {
        None
    }

    fn properties_to_xml(
        object: &ObjectRef<()>,
        media_type: &str,
        properties: serde_json::Value,
    ) -> Result<String, ObjectError>;
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

    fn properties_request(
        &self,
        object: &ObjectRef<()>,
        version: Option<ObjectVersion>,
        client: &Client<Ready>,
    ) -> Result<AdtRequest, OperationError> {
        let resource = object.typed::<T>().ok_or_else(|| {
            OperationError::Object(ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: object.object_type().clone(),
            })
        })?;
        let mut query = resource.query();
        if let Some(version) = version {
            query = query.version(version);
        }
        query.request(client)
    }

    fn properties_to_json(
        &self,
        object: &ObjectRef<()>,
        response: OperationResponse,
    ) -> Result<AdtObject, ResponseError> {
        let resource = object
            .typed::<T>()
            .ok_or_else(|| ObjectError::UnexpectedObjectType {
                expected: T::WORKBENCH_TYPE,
                actual: object.object_type().clone(),
            })?;
        let loaded = resource.query().decode(response)?;
        let (_reference, media_type, etag, properties) = loaded.into_parts();
        Ok(AdtObject::new(
            object.clone(),
            media_type,
            etag,
            serde_json::to_value(properties)?,
        ))
    }

    fn properties_to_xml(
        &self,
        object: &ObjectRef<()>,
        media_type: &str,
        properties: serde_json::Value,
    ) -> Result<String, ObjectError> {
        T::properties_to_xml(object, media_type, properties)
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
];

pub(crate) fn object_type_descriptor(
    object_type: &GlobalWorkbenchType,
) -> Option<&'static dyn RuntimeObjectTypeDescriptor> {
    OBJECT_TYPES
        .iter()
        .copied()
        .find(|descriptor| &descriptor.object_type() == object_type)
}

pub(crate) fn unsupported_update(object_type: GlobalWorkbenchType) -> ObjectError {
    ObjectError::UnsupportedCapability {
        object_type,
        capability: "object properties update",
    }
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
