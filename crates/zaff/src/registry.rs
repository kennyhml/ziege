use zadt::GlobalWorkbenchType;

use crate::{
    ProjectionError,
    format::{FormatDescriptor, ObjectFormat},
    formats::{ClassDescriptor, DataElementDescriptor, ProgramDescriptor},
};

static FORMATS: &[&dyn FormatDescriptor] =
    &[&ProgramDescriptor, &ClassDescriptor, &DataElementDescriptor];

pub(crate) fn for_workbench_type(
    object_type: &GlobalWorkbenchType,
) -> Result<&'static dyn FormatDescriptor, ProjectionError> {
    FORMATS
        .iter()
        .copied()
        .find(|descriptor| descriptor.repository_types().contains(object_type))
        .ok_or_else(|| ProjectionError::UnsupportedRepositoryType {
            object_type: object_type.clone(),
        })
}

pub(crate) fn by_format(format: ObjectFormat) -> &'static dyn FormatDescriptor {
    FORMATS
        .iter()
        .copied()
        .find(|descriptor| descriptor.format() == format)
        .expect("public AFF formats are registered")
}

pub(crate) fn descriptors() -> &'static [&'static dyn FormatDescriptor] {
    FORMATS
}

/// Returns every registered AFF family.
pub fn formats() -> impl ExactSizeIterator<Item = ObjectFormat> {
    FORMATS.iter().map(|descriptor| descriptor.format())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_formats_have_unique_identities() {
        for (index, descriptor) in FORMATS.iter().enumerate() {
            assert!(
                FORMATS[index + 1..]
                    .iter()
                    .all(|other| other.format() != descriptor.format()),
                "registered {:?} more than once",
                descriptor.format()
            );
        }
    }

    #[test]
    fn registered_formats_have_unique_repository_types() {
        for (index, descriptor) in FORMATS.iter().enumerate() {
            for object_type in descriptor.repository_types() {
                assert!(
                    FORMATS[index + 1..]
                        .iter()
                        .all(|other| !other.repository_types().contains(object_type)),
                    "registered `{object_type}` for more than one AFF format"
                );
            }
        }
    }

    #[test]
    fn registered_formats_have_unique_file_components() {
        for descriptor in FORMATS {
            let files = descriptor.files();
            for (index, file) in files.iter().enumerate() {
                assert!(
                    files[index + 1..]
                        .iter()
                        .all(|other| other.component() != file.component()),
                    "registered component `{}` more than once for {:?}",
                    file.component(),
                    descriptor.format()
                );
            }
        }
    }
}
