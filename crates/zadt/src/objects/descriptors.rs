use super::{
    Class, GlobalWorkbenchType, Include, ObjectNamePolicy, ObjectRef, ObjectVersion, Package,
    Program, SourceComponent,
};
use crate::{
    client::{Client, Ready},
    error::{ObjectError, OperationError, ResponseError},
    operation::OperationResponse,
    protocol::AdtRequest,
};

/// A type erased object type that provides capabilities at runtime.
///
/// These descriptors are mapped to workbench types at compile time to
/// serve as a capability registry.
pub(crate) trait RuntimeObjectTypeDescriptor: Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn naming_policy(&self) -> ObjectNamePolicy;

    fn source_path(&self) -> Option<&'static [&'static str]>;

    fn source_components(&self) -> &'static [&'static dyn SourceComponent];

    fn resolve(&self, client: &Client<Ready>, name: &str) -> Result<ObjectRef, ObjectError>;

    fn normalize_reference(&self, reference: &ObjectRef) -> Result<ObjectRef, ObjectError>;

    fn properties(&self) -> &dyn RuntimeObjectProperties;
}

static OBJECT_TYPES: &[&dyn RuntimeObjectTypeDescriptor] = &[
    Program::DESCRIPTOR,
    Include::DESCRIPTOR,
    Class::DESCRIPTOR,
    Package::DESCRIPTOR,
];

pub(crate) fn object_type_descriptor(
    object_type: &GlobalWorkbenchType,
) -> Option<&'static dyn RuntimeObjectTypeDescriptor> {
    OBJECT_TYPES
        .iter()
        .copied()
        .find(|descriptor| &descriptor.object_type() == object_type)
}

/// Runtime descriptor for object type erased properties, this enables
/// us to provide object properties in the form of JSON.
pub(crate) trait RuntimeObjectProperties: Sync {
    fn request(
        &self,
        resource: &ObjectRef,
        version: Option<ObjectVersion>,
        client: &Client<Ready>,
    ) -> Result<AdtRequest, OperationError>;

    fn decode(
        &self,
        resource: &ObjectRef,
        response: OperationResponse,
    ) -> Result<serde_json::Value, ResponseError>;
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
