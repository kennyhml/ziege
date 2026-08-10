use super::{
    Class, GlobalWorkbenchType, Include, ObjectNamePolicy, ObjectProperties, ObjectRef,
    ObjectVersion, Package, Program, SourceComponent,
};
use crate::{
    api::properties::ObjectPropertiesQuery,
    client::{Client, Ready},
    error::{OperationError, ResponseError},
    operation::{Operation, OperationResponse},
    protocol::AdtRequest,
    vocabulary::CategoryId,
};

/// A type erased object type that provides capabilities at runtime.
///
/// These descriptors are mapped to workbench types at compile time to
/// serve as a capability registry.
pub(crate) trait RuntimeObjectTypeDescriptor: Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn naming_policy(&self) -> ObjectNamePolicy;

    fn category(&self) -> CategoryId;

    fn source_path(&self) -> Option<&'static [&'static str]>;

    fn source_components(&self) -> &'static [&'static dyn SourceComponent];

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

// Generated descriptors forward type-specific properties behavior here.
pub(crate) fn properties_request<T: ObjectProperties>(
    resource: &ObjectRef,
    version: Option<ObjectVersion>,
    client: &Client<Ready>,
) -> Result<AdtRequest, OperationError> {
    let mut query = ObjectPropertiesQuery::<T>::new(resource.retype());
    if let Some(version) = version {
        query = query.version(version);
    }
    query.request(client)
}

pub(crate) fn properties_decode<T: ObjectProperties>(
    resource: &ObjectRef,
    response: OperationResponse,
) -> Result<serde_json::Value, ResponseError> {
    let query = ObjectPropertiesQuery::<T>::new(resource.retype());
    serde_json::to_value(query.decode(response)?).map_err(Into::into)
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
