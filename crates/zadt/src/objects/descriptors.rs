use std::marker::PhantomData;

use http::Method;

use super::{
    AccessControl, AnnotationDefinition, AnyObject, Class, DataDefinition, DataElement, Domain,
    FunctionGroup, FunctionGroupInclude, FunctionModule, GlobalWorkbenchType, Include, Interface,
    MetadataExtension, ObjectRef, ObjectType, ObjectVersion, Package, Program, PropertyModel,
    RunCapability, ServiceDefinition,
};
use crate::{
    CategoryId,
    error::{EncodeError, ObjectError, ResponseError},
    operation::{EncodedOperation, Operation, OperationResponse, Owned},
};

/// Runtime metadata for one statically declared parent-child relationship.
#[derive(Clone, Debug)]
pub struct SubObjectDescriptor {
    object_type: GlobalWorkbenchType,
    relation: &'static str,
    parent_variable: &'static str,
}

impl SubObjectDescriptor {
    pub(crate) const fn new(
        object_type: GlobalWorkbenchType,
        relation: &'static str,
        parent_variable: &'static str,
    ) -> Self {
        Self {
            object_type,
            relation,
            parent_variable,
        }
    }

    pub(crate) fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
    }

    pub(crate) const fn relation(&self) -> &'static str {
        self.relation
    }

    pub(crate) const fn parent_variable(&self) -> &'static str {
        self.parent_variable
    }
}

/// Runtime capabilities for one modeled ADT object type.
pub(crate) trait RuntimeObjectTypeDescriptor: std::fmt::Debug + Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn category(&self) -> Option<CategoryId>;

    fn subobjects(&self) -> &'static [SubObjectDescriptor];

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
    fn category() -> Option<CategoryId>;

    fn subobjects() -> &'static [SubObjectDescriptor];

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

    fn category(&self) -> Option<CategoryId> {
        T::category()
    }

    fn subobjects(&self) -> &'static [SubObjectDescriptor] {
        T::subobjects()
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
        let properties = validated_properties::<T>(object, properties)?;
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
        let properties = validated_properties::<T>(object, properties)?;
        T::source_component_uri(&properties, name)
            .map(|href| crate::resource::refs::source_from_href(object.clone(), href))
            .transpose()
    }

    fn object_structure(
        &self,
        object: &ObjectRef<()>,
        properties: &serde_json::Value,
    ) -> Result<Option<crate::ObjectStructureRef>, ObjectError> {
        let properties = validated_properties::<T>(object, properties)?;
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

fn validated_properties<T: ObjectType>(
    object: &ObjectRef<()>,
    properties: &serde_json::Value,
) -> Result<T::Properties, ObjectError> {
    let properties: T::Properties =
        serde_json::from_value(properties.clone()).map_err(ObjectError::InvalidPropertiesJson)?;
    if !properties.belongs_to(object) {
        return Err(ObjectError::UnexpectedObjectReference {
            expected: object.to_string(),
            actual: properties.object_description(),
        });
    }
    Ok(properties)
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
    FunctionGroup::DESCRIPTOR,
    FunctionModule::DESCRIPTOR,
    FunctionGroupInclude::DESCRIPTOR,
];

pub(crate) fn object_type_descriptor(
    object_type: &GlobalWorkbenchType,
) -> Option<&'static dyn RuntimeObjectTypeDescriptor> {
    OBJECT_TYPES
        .iter()
        .copied()
        .find(|descriptor| &descriptor.object_type() == object_type)
}

pub(crate) fn requires_parent(object_type: &GlobalWorkbenchType) -> bool {
    object_type_descriptor(object_type).is_some_and(|descriptor| descriptor.category().is_none())
}

pub(crate) fn supports_subobject(
    parent_type: &GlobalWorkbenchType,
    child_type: &GlobalWorkbenchType,
) -> bool {
    object_type_descriptor(parent_type).is_some_and(|descriptor| {
        descriptor
            .subobjects()
            .iter()
            .any(|subobject| subobject.object_type() == child_type)
    })
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
    fn every_registered_subobject_has_one_declared_parent() {
        for child in OBJECT_TYPES
            .iter()
            .filter(|descriptor| descriptor.category().is_none())
        {
            let parent_count = OBJECT_TYPES
                .iter()
                .flat_map(|parent| parent.subobjects())
                .filter(|subobject| subobject.object_type() == &child.object_type())
                .count();
            assert_eq!(
                parent_count,
                1,
                "subobject `{}` must have exactly one declared parent",
                child.object_type()
            );
        }
    }

    #[test]
    fn declared_subobjects_are_registered_and_not_primary() {
        for subobject in OBJECT_TYPES
            .iter()
            .flat_map(|descriptor| descriptor.subobjects())
        {
            let descriptor = object_type_descriptor(subobject.object_type())
                .expect("declared subobject type must be registered");
            assert!(descriptor.category().is_none());
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
