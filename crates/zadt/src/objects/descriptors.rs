use super::{
    Class, DataElement, Erased, GlobalWorkbenchType, Include, ObjectRef, ObjectVersion, Package,
    Program, ReadProperties, RunCapability, UpdateProperties, WritableProperties,
};
use crate::{
    JsonObjectProperties, MediaVersionNegotiation,
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

    fn source_path(&self) -> Option<&'static [&'static str]>;

    fn source_component_paths(&self) -> &'static [&'static [&'static str]];

    fn run(&self) -> Option<RunCapability>;

    fn properties_request(
        &self,
        object: &ObjectRef<Erased>,
        version: Option<ObjectVersion>,
        client: &Client<Ready>,
    ) -> Result<AdtRequest, OperationError>;

    fn properties_to_json(
        &self,
        object: &ObjectRef<Erased>,
        response: OperationResponse,
    ) -> Result<JsonObjectProperties, ResponseError>;

    fn properties_to_xml(
        &self,
        object: &ObjectRef<Erased>,
        media_type: &'static str,
        payload: serde_json::Value,
    ) -> Result<String, ObjectError>;
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

pub(crate) fn unsupported_update(object: &ObjectRef<Erased>) -> ObjectError {
    ObjectError::UnsupportedCapability {
        object_type: object.object_type().clone(),
        capability: "object properties update",
    }
}

pub(crate) fn properties_request<T>(
    object: &ObjectRef<Erased>,
    version: Option<ObjectVersion>,
    client: &Client<Ready>,
) -> Result<AdtRequest, OperationError>
where
    T: ReadProperties,
{
    let resource = object.typed::<T>().ok_or_else(|| {
        OperationError::Response(ResponseError::Object(ObjectError::UnexpectedObjectType {
            expected: T::WORKBENCH_TYPE,
            actual: object.object_type().clone(),
        }))
    })?;
    let mut query = resource.query();
    if let Some(version) = version {
        query = query.version(version);
    }
    query.request(client)
}

pub(crate) fn properties_to_json<T>(
    object: &ObjectRef<Erased>,
    response: OperationResponse,
) -> Result<JsonObjectProperties, ResponseError>
where
    T: ReadProperties,
{
    let resource = object
        .typed::<T>()
        .ok_or_else(|| ObjectError::UnexpectedObjectType {
            expected: T::WORKBENCH_TYPE,
            actual: object.object_type().clone(),
        })?;
    let properties = resource.query().decode(response)?;
    let (_, media_version, etag, payload) = properties.into_parts();
    Ok(JsonObjectProperties::new(
        object.clone(),
        media_version.media_type(),
        etag,
        serde_json::to_value(payload)?,
    ))
}

pub(crate) fn properties_to_xml<T>(
    object: &ObjectRef<Erased>,
    _media_type: &'static str,
    payload: serde_json::Value,
) -> Result<String, ObjectError>
where
    T: UpdateProperties,
    T::Properties: serde::de::DeserializeOwned + WritableProperties<T>,
{
    let resource = object
        .typed::<T>()
        .ok_or_else(|| ObjectError::UnexpectedObjectType {
            expected: T::WORKBENCH_TYPE,
            actual: object.object_type().clone(),
        })?;
    let properties: T::Properties =
        serde_json::from_value(payload).map_err(ObjectError::InvalidPropertiesJson)?;
    properties.to_xml(&resource)
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
