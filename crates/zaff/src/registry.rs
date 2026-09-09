use zadt::GlobalWorkbenchType;

use crate::{
    ProjectionError,
    formats::{ObjectFormat, clas, dtel, fugr, intf, prog},
};

static FORMATS: &[&ObjectFormat] = &[
    &prog::PROGRAM_FORMAT,
    &clas::CLASS_FORMAT,
    &intf::INTERFACE_FORMAT,
    &dtel::DATA_ELEMENT_FORMAT,
    &fugr::FUNCTION_GROUP_FORMAT,
    &fugr::FUNCTION_MODULE_FORMAT,
    &fugr::FUNCTION_GROUP_INCLUDE_FORMAT,
];

pub(crate) fn for_workbench_type(
    workbench_type: &GlobalWorkbenchType,
) -> Result<&'static ObjectFormat, ProjectionError> {
    FORMATS
        .iter()
        .copied()
        .find(|format| format.workbench_types().contains(workbench_type))
        .ok_or_else(|| ProjectionError::UnsupportedRepositoryType {
            workbench_type: workbench_type.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups_return_the_registered_static_format() {
        for &format in FORMATS {
            for workbench_type in format.workbench_types() {
                assert!(std::ptr::eq(
                    for_workbench_type(workbench_type).unwrap(),
                    format
                ));
            }
        }
    }

    #[test]
    fn registered_formats_have_unique_identities() {
        for (index, format) in FORMATS.iter().enumerate() {
            assert!(
                FORMATS[index + 1..].iter().all(|other| other != format),
                "registered {:?} more than once",
                format
            );
        }
    }

    #[test]
    fn registered_formats_have_unique_repository_types() {
        for (index, format) in FORMATS.iter().enumerate() {
            for workbench_type in format.workbench_types() {
                assert!(
                    FORMATS[index + 1..]
                        .iter()
                        .all(|other| !other.workbench_types().contains(workbench_type)),
                    "registered `{workbench_type}` for more than one AFF format"
                );
            }
        }
    }

    #[test]
    fn registered_formats_have_unique_file_templates() {
        for format in FORMATS {
            let files = format.files();
            for (index, file) in files.iter().enumerate() {
                assert!(
                    files[index + 1..]
                        .iter()
                        .all(|other| other.template() != file.template()),
                    "registered file template `{}` more than once for {:?}",
                    file.template(),
                    format
                );
            }
        }
    }
}
