use super::{
    AccessControl, AnnotationDefinition, AssignObjectIdentity, Class, Create, DataDefinition,
    DataElement, Domain, ErasedProperties, FunctionGroup, FunctionGroupInclude, FunctionModule,
    GlobalWorkbenchType, Include, Interface, MediaTyped, MetadataExtension, ObjectIdentity,
    ObjectRef, ObjectSnapshot, ObjectType, Package, Program, RunCapability, ServiceDefinition,
    Source, SourceComponents, Structure, ToXml, XmlConversion,
};
use crate::{CategoryId, ObjectStructureQuery, SourceRef, error::ObjectError};

/// Runtime descriptor for one modeled object type.
///
/// Typed APIs use `T: ObjectType` to select properties and capabilities at
/// compile time. Some consumers, however, identify objects only by their
/// runtime Workbench type, such as objects supplied through a command-line
/// interface or opened dynamically in an editor.
///
/// This descriptor bridges those erased references to their concrete object
/// families. It stores required addressing and properties metadata together
/// with optional adapters for capabilities supported by the object family.
/// Consequently, erased operations validate object types and capabilities at
/// runtime, while typed operations retain their compile-time guarantees.
///
/// The adapters are function pointers with common erased signatures. Generic
/// implementations are monomorphized for each registered object type and then
/// stored using those signatures. For example, [`PropertiesCodec::for_type`]
/// stores `PropertiesCodec::decode_xml::<T>` as:
///
/// ```ignore
/// type DecodeXmlFn =
///     fn(&ObjectRef<()>, &[u8]) -> Result<ErasedProperties, ObjectError>;
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
    object_type: GlobalWorkbenchType,
    addressing: ObjectAddressing,
    properties: PropertiesCodec,
    capabilities: RuntimeCapabilities,
}

impl ObjectTypeDescriptor {
    pub(crate) const fn new(
        object_type: GlobalWorkbenchType,
        addressing: ObjectAddressing,
        properties: PropertiesCodec,
        capabilities: RuntimeCapabilities,
    ) -> Self {
        Self {
            object_type,
            addressing,
            properties,
            capabilities,
        }
    }

    pub(crate) fn object_type(&self) -> &GlobalWorkbenchType {
        &self.object_type
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

    pub(crate) fn creation_media_types(&self) -> Option<&'static [&'static str]> {
        self.capabilities.create.map(|create| create.media_types)
    }

    pub(crate) fn creation_payload_to_xml(
        &self,
        reference: &ObjectRef<()>,
        payload: serde_json::Value,
    ) -> Result<Vec<u8>, ObjectError> {
        let create = self
            .capabilities
            .create
            .ok_or_else(|| reference.unsupported_capability("object creation"))?;
        (create.encode)(reference, payload)
    }

    pub(crate) const fn run(&self) -> Option<RunCapability> {
        self.capabilities.run
    }

    pub(crate) fn source(&self, object: &ObjectSnapshot<()>) -> Result<SourceRef, ObjectError> {
        let source = self
            .capabilities
            .source
            .ok_or_else(|| object.reference().unsupported_capability("source"))?;
        source(object)
    }

    pub(crate) fn source_component(
        &self,
        object: &ObjectSnapshot<()>,
        name: &str,
    ) -> Result<Option<crate::SourceRef>, ObjectError> {
        let source_component = self.capabilities.source_component.ok_or_else(|| {
            object
                .reference()
                .unsupported_capability("source components")
        })?;
        source_component(object, name)
    }

    pub(crate) fn object_structure(
        &self,
        object: &ObjectSnapshot<()>,
    ) -> Result<ObjectStructureQuery, ObjectError> {
        let object_structure = self.capabilities.object_structure.ok_or_else(|| {
            object
                .reference()
                .unsupported_capability("object structure")
        })?;
        object_structure(object)
    }

    pub(crate) fn properties_from_xml(
        &self,
        object: &ObjectRef<()>,
        body: &[u8],
    ) -> Result<ErasedProperties, ObjectError> {
        (self.properties.decode_xml)(object, body)
    }

    pub(crate) fn properties_to_xml(
        &self,
        object: &ObjectRef<()>,
        properties: &ErasedProperties,
    ) -> Result<Vec<u8>, ObjectError> {
        (self.properties.encode_xml)(object, properties)
    }

    pub(crate) fn properties_from_json(
        &self,
        object: &ObjectRef<()>,
        properties: serde_json::Value,
    ) -> Result<ErasedProperties, ObjectError> {
        (self.properties.decode_json)(object, properties)
    }

    pub(crate) fn properties_to_json(
        &self,
        object: &ObjectRef<()>,
        properties: &ErasedProperties,
    ) -> Result<serde_json::Value, ObjectError> {
        (self.properties.encode_json)(object, properties)
    }

    pub(crate) const fn properties_media_types(&self) -> &'static [&'static str] {
        self.properties.media_types
    }
}

type DecodeXmlFn = fn(&ObjectRef, &[u8]) -> Result<ErasedProperties, ObjectError>;
type DecodeJsonFn = fn(&ObjectRef, serde_json::Value) -> Result<ErasedProperties, ObjectError>;
type EncodeXmlFn = fn(&ObjectRef, &ErasedProperties) -> Result<Vec<u8>, ObjectError>;
type EncodeJsonFn = fn(&ObjectRef, &ErasedProperties) -> Result<serde_json::Value, ObjectError>;
type EncodeCreationFn = fn(&ObjectRef, serde_json::Value) -> Result<Vec<u8>, ObjectError>;
type SourceFn = fn(&ObjectSnapshot<()>) -> Result<SourceRef, ObjectError>;
type SourceComponentFn = fn(&ObjectSnapshot<()>, &str) -> Result<Option<SourceRef>, ObjectError>;
type ObjectStructureFn = fn(&ObjectSnapshot<()>) -> Result<ObjectStructureQuery, ObjectError>;

/// Type-erased codecs for one complete properties representation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PropertiesCodec {
    media_types: &'static [&'static str],
    decode_xml: DecodeXmlFn,
    decode_json: DecodeJsonFn,
    encode_xml: EncodeXmlFn,
    encode_json: EncodeJsonFn,
}

impl PropertiesCodec {
    pub(crate) const fn for_type<T: ObjectType>() -> Self {
        Self {
            media_types: T::Properties::MEDIA_TYPES,
            decode_xml: Self::decode_xml::<T>,
            decode_json: Self::decode_json::<T>,
            encode_xml: Self::encode_xml::<T>,
            encode_json: Self::encode_json::<T>,
        }
    }

    fn decode_xml<T: ObjectType>(
        object: &ObjectRef,
        body: &[u8],
    ) -> Result<ErasedProperties, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties = T::Properties::from_xml(body)?;
        properties.validate_for(object)?;
        Ok(std::sync::Arc::new(properties))
    }

    fn decode_json<T: ObjectType>(
        object: &ObjectRef,
        properties: serde_json::Value,
    ) -> Result<ErasedProperties, ObjectError> {
        validate_object_type::<T>(object)?;
        let mut properties: T::Properties =
            serde_json::from_value(properties).map_err(ObjectError::InvalidPropertiesJson)?;
        properties.assign_identity(object);
        Ok(std::sync::Arc::new(properties))
    }

    fn encode_xml<T: ObjectType>(
        object: &ObjectRef,
        properties: &ErasedProperties,
    ) -> Result<Vec<u8>, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties = Self::properties::<T>(properties);
        properties.to_xml()
    }

    fn encode_json<T: ObjectType>(
        object: &ObjectRef,
        properties: &ErasedProperties,
    ) -> Result<serde_json::Value, ObjectError> {
        validate_object_type::<T>(object)?;
        let properties = Self::properties::<T>(properties);
        properties.validate_for(object)?;
        serde_json::to_value(properties).map_err(ObjectError::InvalidPropertiesJson)
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
    media_types: &'static [&'static str],
    encode: EncodeCreationFn,
}

impl CreateCodec {
    pub(crate) const fn for_type<T>() -> Self
    where
        T: Create,
        T::Payload: serde::de::DeserializeOwned,
    {
        Self {
            media_types: T::CREATE_MEDIA_TYPES,
            encode: Self::encode::<T>,
        }
    }

    fn encode<T>(reference: &ObjectRef, payload: serde_json::Value) -> Result<Vec<u8>, ObjectError>
    where
        T: Create,
        T::Payload: serde::de::DeserializeOwned,
    {
        validate_object_type::<T>(reference)?;
        let mut payload: T::Payload =
            serde_json::from_value(payload).map_err(ObjectError::InvalidPropertiesJson)?;
        payload.assign_identity(reference);
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
    source: Option<SourceFn>,
    source_component: Option<SourceComponentFn>,
    object_structure: Option<ObjectStructureFn>,
}

impl RuntimeCapabilities {
    pub(crate) const fn new(
        create: Option<CreateCodec>,
        run: Option<RunCapability>,
        source: Option<SourceFn>,
        source_component: Option<SourceComponentFn>,
        object_structure: Option<ObjectStructureFn>,
    ) -> Self {
        Self {
            create,
            run,
            source,
            source_component,
            object_structure,
        }
    }

    pub(crate) fn source_adapter<T: Source>(
        object: &ObjectSnapshot<()>,
    ) -> Result<SourceRef, ObjectError> {
        ObjectSnapshot::<T>::source_from_parts(
            &object.typed_reference::<T>()?,
            object.typed_properties::<T>(),
        )
    }

    pub(crate) fn source_component_adapter<T: SourceComponents>(
        object: &ObjectSnapshot<()>,
        name: &str,
    ) -> Result<Option<SourceRef>, ObjectError> {
        ObjectSnapshot::<T>::source_component_from_parts(
            &object.typed_reference::<T>()?,
            object.typed_properties::<T>(),
            name,
        )
    }

    pub(crate) fn object_structure_adapter<T: Structure>(
        object: &ObjectSnapshot<()>,
    ) -> Result<ObjectStructureQuery, ObjectError> {
        ObjectSnapshot::<T>::object_structure_from_parts(
            &object.typed_reference::<T>()?,
            object.typed_properties::<T>(),
        )
    }
}

fn validate_object_type<T: ObjectType>(object: &ObjectRef<()>) -> Result<(), ObjectError> {
    if object.object_type() == &T::WORKBENCH_TYPE {
        return Ok(());
    }
    Err(ObjectError::UnexpectedObjectType {
        expected: T::WORKBENCH_TYPE,
        actual: object.object_type().clone(),
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
    object_type: &GlobalWorkbenchType,
) -> Option<&'static ObjectTypeDescriptor> {
    OBJECT_TYPES
        .iter()
        .copied()
        .find(|descriptor| descriptor.object_type() == object_type)
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

/// Runtime metadata for one statically declared parent-child relationship.
#[doc(hidden)]
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
                .filter(|subobject| subobject.object_type() == child.object_type())
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
        let object = crate::ObjectSnapshot::new(
            ObjectRef::<Class>::new(
                "CL_ADT_URI_MAPPER".to_owned(),
                AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            ),
            crate::WorkbenchVersion::Active,
            "application/vnd.sap.adt.oo.classes.v4+xml",
            None,
            properties,
        )
        .into_erased();

        let source = Class::DESCRIPTOR.source(&object).unwrap();

        assert_eq!(
            source.uri.as_str(),
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main"
        );
    }
}
