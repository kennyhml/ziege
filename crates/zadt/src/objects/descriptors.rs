use std::marker::PhantomData;

use super::{
    Class, GlobalWorkbenchType, Include, ObjectNamePolicy, ObjectProperties, ObjectRef,
    ObjectVersion, Package, Program, SourceComponent,
};
use crate::{
    api::properties::ObjectPropertiesQuery,
    client::{Client, Ready},
    error::{ObjectError, OperationError, ResponseError},
    operation::{Operation, OperationResponse},
    protocol::AdtRequest,
};

/// A type erased object type that provides capabilities at runtime.
///
/// These descriptors are mapped to workbench types at compile time to
/// serve as a capability registry.
pub(crate) trait RuntimeObjectTypeDescriptor: Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn naming_policy(&self) -> ObjectNamePolicy;

    fn source_components(&self) -> &'static [&'static dyn SourceComponent];

    fn resolve(&self, client: &Client<Ready>, name: &str) -> Result<ObjectRef, ObjectError>;

    fn normalize_reference(&self, reference: &ObjectRef) -> Result<ObjectRef, ObjectError>;

    fn properties(&self) -> &dyn RuntimeObjectProperties;
}

static OBJECT_TYPES: &[&dyn RuntimeObjectTypeDescriptor] = &[
    &ObjectTypeDescriptor::of::<Program>(),
    &ObjectTypeDescriptor::of::<Include>(),
    &ObjectTypeDescriptor::of::<Class>(),
    &ObjectTypeDescriptor::of::<Package>(),
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

/// Namespace for compile-time descriptor construction.
struct ObjectTypeDescriptor;

/// Runtime descriptor for one modeled object family.
struct TypedObjectTypeDescriptor<T> {
    properties: TypedProperties<T>,
    marker: PhantomData<fn() -> T>,
}

impl ObjectTypeDescriptor {
    const fn of<T>() -> TypedObjectTypeDescriptor<T>
    where
        T: ObjectProperties,
    {
        TypedObjectTypeDescriptor {
            properties: TypedProperties(PhantomData),
            marker: PhantomData,
        }
    }
}

struct TypedProperties<T>(PhantomData<fn() -> T>);

impl<T> RuntimeObjectTypeDescriptor for TypedObjectTypeDescriptor<T>
where
    T: ObjectProperties,
{
    fn object_type(&self) -> GlobalWorkbenchType {
        T::WORKBENCH_TYPE
    }

    fn naming_policy(&self) -> ObjectNamePolicy {
        T::NAMING_POLICY
    }

    fn source_components(&self) -> &'static [&'static dyn SourceComponent] {
        T::SOURCE_COMPONENTS
    }

    fn resolve(&self, client: &Client<Ready>, name: &str) -> Result<ObjectRef, ObjectError> {
        client.object::<T>(name).map(|reference| reference.erase())
    }

    fn normalize_reference(&self, reference: &ObjectRef) -> Result<ObjectRef, ObjectError> {
        ObjectRef::<T>::from_parts(reference.raw_name().to_owned(), reference.uri().clone())
            .map(|reference| reference.erase())
    }

    fn properties(&self) -> &dyn RuntimeObjectProperties {
        &self.properties
    }
}

impl<T> RuntimeObjectProperties for TypedProperties<T>
where
    T: ObjectProperties,
{
    fn request(
        &self,
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

    fn decode(
        &self,
        resource: &ObjectRef,
        response: OperationResponse,
    ) -> Result<serde_json::Value, ResponseError> {
        let query = ObjectPropertiesQuery::<T>::new(resource.retype());
        serde_json::to_value(query.decode(response)?).map_err(Into::into)
    }
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
