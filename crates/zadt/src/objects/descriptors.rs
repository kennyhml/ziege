use super::{Class, DataElement, GlobalWorkbenchType, Include, Package, Program, RunCapability};
use crate::vocabulary::CategoryId;

/// A type erased object type that provides capabilities at runtime.
///
/// These descriptors are mapped to workbench types at compile time to
/// serve as a capability registry.
pub(crate) trait RuntimeObjectTypeDescriptor: Sync {
    fn object_type(&self) -> GlobalWorkbenchType;

    fn category(&self) -> CategoryId;

    fn source_path(&self) -> Option<&'static [&'static str]>;

    fn source_component_paths(&self) -> &'static [&'static [&'static str]];

    fn run(&self) -> Option<RunCapability>;
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
