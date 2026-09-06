use super::{
    AccessControl, AnnotationDefinition, Class, Create, DataDefinition, DataElement, Domain,
    ErasedProperties, FunctionGroup, FunctionGroupInclude, FunctionModule, GlobalWorkbenchType,
    Identity, Include, Interface, MetadataExtension, ObjectKey, ObjectRef, ObjectType, Package,
    Program, Resources, RunCapability, ServiceDefinition, ToXml, XmlCodec,
};
use crate::{CategoryId, ResourceView, compatibility::MediaTypes, error::ObjectError};

/// Runtime descriptor for one modeled object type.
///
/// Typed APIs use `T: ObjectType` to select properties and capabilities at
/// compile time. Some consumers, however, identify objects only by their
/// runtime Workbench type, such as objects supplied through a command-line
/// interface or opened dynamically in an editor.
///
/// This descriptor bridges those erased references to their concrete object
/// families. It stores required addressing and properties metadata together
/// with metadata for capabilities supported by the object family.
/// Consequently, erased operations validate object types and capabilities at
/// runtime, while typed operations retain their compile-time guarantees.
///
/// Property codecs and resource extraction use common erased signatures. Generic
/// implementations are monomorphized for each registered object type and then
/// stored using those signatures. For example, [`PropertiesCodec::for_type`]
/// stores `PropertiesCodec::decode_xml::<T>` as:
///
/// ```ignore
/// type DecodeXmlFn =
///     fn(&ObjectKey<()>, &[u8]) -> Result<ErasedProperties, ObjectError>;
/// ```
///
/// For `T = Class`, the stored function pointer targets the `Class`
/// monomorphization. It deserializes `Class::Properties`, validates the
/// resulting identity, and returns the properties behind the erased handle.
///
/// This resembles a manually constructed vtable - shared generic adapters
/// provide the implementations, while each descriptor selects the functions
/// appropriate for one concrete object family.
#[derive(Clone, Debug)]
pub(crate) struct ObjectTypeDescriptor {
    workbench_type: GlobalWorkbenchType,
    addressing: ObjectAddressing,
    properties: PropertiesCodec,
    capabilities: RuntimeCapabilities,
}

impl ObjectTypeDescriptor {
    pub(crate) const fn new(
        workbench_type: GlobalWorkbenchType,
        addressing: ObjectAddressing,
        properties: PropertiesCodec,
        capabilities: RuntimeCapabilities,
    ) -> Self {
        Self {
            workbench_type,
            addressing,
            properties,
            capabilities,
        }
    }

    pub(crate) fn workbench_type(&self) -> &GlobalWorkbenchType {
        &self.workbench_type
    }

    pub(crate) const fn category(&self) -> Option<CategoryId> {
        match self.addressing {
            ObjectAddressing::Primary { category, .. } => Some(category),
            ObjectAddressing::Child => None,
        }
    }

    pub(crate) const fn subobjects(&self) -> &'static [SubObjectDescriptor] {
        match self.addressing {
            ObjectAddressing::Primary { subobjects, .. } => subobjects,
            ObjectAddressing::Child => &[],
        }
    }

    pub(crate) fn creation_media_types(&self) -> Option<MediaTypes> {
        self.capabilities
            .create
            .map(|_| self.properties.media_types)
    }

    pub(crate) fn creation_payload_to_xml(
        &self,
        reference: &ObjectRef<()>,
        payload: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError> {
        let create = self
            .capabilities
            .create
            .ok_or_else(|| reference.key().unsupported_capability("object creation"))?;
        (create.encode)(reference, payload)
    }

    pub(crate) const fn run(&self) -> Option<RunCapability> {
        self.capabilities.run
    }

    pub(crate) fn supports_source(&self) -> bool {
        self.capabilities.source
    }

    pub(crate) fn supports_source_components(&self) -> bool {
        self.capabilities.source_component
    }

    pub(crate) fn supports_structure(&self) -> bool {
        self.capabilities.object_structure
    }

    pub(crate) fn resources<'a>(&self, properties: &'a ErasedProperties) -> ResourceView<'a> {
        (self.properties.resources)(properties)
    }

    pub(crate) fn properties_from_xml(
        &self,
        object: &ObjectKey<()>,
        body: &[u8],
    ) -> Result<ErasedProperties, ObjectError> {
        (self.properties.decode_xml)(object, body)
    }

    pub(crate) fn properties_to_xml(
        &self,
        object: &ObjectKey<()>,
        properties: &ErasedProperties,
    ) -> Result<Vec<u8>, ObjectError> {
        (self.properties.encode_xml)(object, properties)
    }

    pub(crate) fn properties_from_json(
        &self,
        object: &ObjectKey<()>,
        properties: serde_json::Value,
    ) -> Result<ErasedProperties, ObjectError> {
        (self.properties.decode_json)(object, properties)
    }

    pub(crate) fn properties_to_json(
        &self,
        object: &ObjectKey<()>,
        properties: &ErasedProperties,
    ) -> Result<serde_json::Value, ObjectError> {
        (self.properties.encode_json)(object, properties)
    }

    pub(crate) const fn properties_media_types(&self) -> MediaTypes {
        self.properties.media_types
    }
}

type DecodeXmlFn = fn(&ObjectKey, &[u8]) -> Result<ErasedProperties, ObjectError>;
type DecodeJsonFn = fn(&ObjectKey, serde_json::Value) -> Result<ErasedProperties, ObjectError>;
type EncodeXmlFn = fn(&ObjectKey, &ErasedProperties) -> Result<Vec<u8>, ObjectError>;
type EncodeJsonFn = fn(&ObjectKey, &ErasedProperties) -> Result<serde_json::Value, ObjectError>;
type EncodeCreationFn = fn(&ObjectRef, serde_json::Value) -> Result<Vec<u8>, ObjectError>;
type ResourcesFn = for<'a> fn(&'a ErasedProperties) -> ResourceView<'a>;

/// Type-erased codecs and resource extraction for one complete properties representation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PropertiesCodec {
    media_types: MediaTypes,
    decode_xml: DecodeXmlFn,
    decode_json: DecodeJsonFn,
    encode_xml: EncodeXmlFn,
    encode_json: EncodeJsonFn,
    resources: ResourcesFn,
}

impl PropertiesCodec {
    pub(crate) const fn for_type<T: ObjectType>() -> Self {
        Self {
            media_types: T::MEDIA_TYPES,
            decode_xml: Self::decode_xml::<T>,
            decode_json: Self::decode_json::<T>,
            encode_xml: Self::encode_xml::<T>,
            encode_json: Self::encode_json::<T>,
            resources: Self::resources::<T>,
        }
    }

    fn decode_xml<T: ObjectType>(
        object: &ObjectKey,
        body: &[u8],
    ) -> Result<ErasedProperties, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties = T::Properties::from_xml(body)?;
        properties.validate_for(object)?;
        Ok(std::sync::Arc::new(properties))
    }

    fn decode_json<T: ObjectType>(
        object: &ObjectKey,
        properties: serde_json::Value,
    ) -> Result<ErasedProperties, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties: T::Properties =
            serde_json::from_value(properties).map_err(ObjectError::InvalidPropertiesJson)?;
        properties.validate_for(object)?;
        Ok(std::sync::Arc::new(properties))
    }

    fn encode_xml<T: ObjectType>(
        object: &ObjectKey,
        properties: &ErasedProperties,
    ) -> Result<Vec<u8>, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties = Self::properties::<T>(properties);
        properties.to_xml()
    }

    fn encode_json<T: ObjectType>(
        object: &ObjectKey,
        properties: &ErasedProperties,
    ) -> Result<serde_json::Value, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties = Self::properties::<T>(properties);
        properties.validate_for(object)?;
        serde_json::to_value(properties).map_err(ObjectError::InvalidPropertiesJson)
    }

    fn resources<T: ObjectType>(properties: &ErasedProperties) -> ResourceView<'_> {
        Self::properties::<T>(properties).resources()
    }

    fn properties<T: ObjectType>(properties: &ErasedProperties) -> &T::Properties {
        properties
            .downcast_ref::<T::Properties>()
            .expect("registered descriptor must retain its concrete property type")
    }
}

/// Type-erased codec and media types for an object-creation payload.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CreateCodec {
    encode: EncodeCreationFn,
}

impl CreateCodec {
    pub(crate) const fn for_type<T>() -> Self
    where
        T: Create,
        T::Payload: serde::de::DeserializeOwned,
    {
        Self {
            encode: Self::encode::<T>,
        }
    }

    fn encode<T>(reference: &ObjectRef, payload: serde_json::Value) -> Result<Vec<u8>, ObjectError>
    where
        T: Create,
        T::Payload: serde::de::DeserializeOwned,
    {
        validate_object_type::<T>(reference.key())?;
        let mut payload: T::Payload =
            serde_json::from_value(payload).map_err(ObjectError::InvalidPropertiesJson)?;
        T::prepare_payload(&mut payload, reference);
        payload.to_xml()
    }
}

/// Runtime addressing metadata for one object type.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ObjectAddressing {
    Primary {
        category: CategoryId,
        subobjects: &'static [SubObjectDescriptor],
    },
    Child,
}

/// Optional operations available through a type-erased object reference.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeCapabilities {
    create: Option<CreateCodec>,
    run: Option<RunCapability>,
    source: bool,
    source_component: bool,
    object_structure: bool,
}

impl RuntimeCapabilities {
    pub(crate) const fn new(
        create: Option<CreateCodec>,
        run: Option<RunCapability>,
        source: bool,
        source_component: bool,
        object_structure: bool,
    ) -> Self {
        Self {
            create,
            run,
            source,
            source_component,
            object_structure,
        }
    }
}

fn validate_object_type<T: ObjectType>(object: &ObjectKey<()>) -> Result<(), ObjectError> {
    if object.workbench_type() == &T::WORKBENCH_TYPE {
        return Ok(());
    }
    Err(ObjectError::UnexpectedObjectType {
        expected: T::WORKBENCH_TYPE,
        actual: object.workbench_type().clone(),
    })
}

static OBJECT_TYPES: &[&ObjectTypeDescriptor] = &[
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
    workbench_type: &GlobalWorkbenchType,
) -> Option<&'static ObjectTypeDescriptor> {
    OBJECT_TYPES
        .iter()
        .copied()
        .find(|descriptor| descriptor.workbench_type() == workbench_type)
}

pub(crate) fn requires_parent(workbench_type: &GlobalWorkbenchType) -> bool {
    object_type_descriptor(workbench_type).is_some_and(|descriptor| descriptor.category().is_none())
}

pub(crate) fn supports_subobject(
    parent_type: &GlobalWorkbenchType,
    child_type: &GlobalWorkbenchType,
) -> bool {
    object_type_descriptor(parent_type).is_some_and(|descriptor| {
        descriptor
            .subobjects()
            .iter()
            .any(|subobject| subobject.workbench_type() == child_type)
    })
}

/// Runtime metadata for one statically declared parent-child relationship.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct SubObjectDescriptor {
    workbench_type: GlobalWorkbenchType,
    relation: &'static str,
    parent_variable: &'static str,
}

impl SubObjectDescriptor {
    pub(crate) const fn new(
        workbench_type: GlobalWorkbenchType,
        relation: &'static str,
        parent_variable: &'static str,
    ) -> Self {
        Self {
            workbench_type,
            relation,
            parent_variable,
        }
    }

    pub(crate) fn workbench_type(&self) -> &GlobalWorkbenchType {
        &self.workbench_type
    }

    pub(crate) const fn relation(&self) -> &'static str {
        self.relation
    }

    pub(crate) const fn parent_variable(&self) -> &'static str {
        self.parent_variable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, ClassProperties};

    #[test]
    fn registered_object_types_are_unique() {
        for (index, descriptor) in OBJECT_TYPES.iter().enumerate() {
            let workbench_type = descriptor.workbench_type();
            assert!(
                OBJECT_TYPES[index + 1..]
                    .iter()
                    .all(|other| other.workbench_type() != workbench_type),
                "registered `{workbench_type}` more than once"
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
                .filter(|subobject| subobject.workbench_type() == child.workbench_type())
                .count();
            assert_eq!(
                parent_count,
                1,
                "subobject `{}` must have exactly one declared parent",
                child.workbench_type()
            );
        }
    }

    #[test]
    fn declared_subobjects_are_registered_and_not_primary() {
        for subobject in OBJECT_TYPES
            .iter()
            .flat_map(|descriptor| descriptor.subobjects())
        {
            let descriptor = object_type_descriptor(subobject.workbench_type())
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
        let object = crate::ObjectSnapshot::new(
            ObjectRef::new(
                ObjectKey::<Class>::new("CL_ADT_URI_MAPPER"),
                AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            ),
            crate::WorkbenchVersion::Active,
            "application/vnd.sap.adt.oo.classes.v4+xml",
            None,
            properties,
        )
        .into_erased();

        assert!(Class::DESCRIPTOR.supports_source());
        let source = object.source().unwrap();

        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main"
        );
    }
}
