use super::super::ObjectRef;
use crate::models::{DataElementProperties, DataElementPropertiesVersion};
use zadt_macros::object_type;

/// The ABAP Dictionary Data Element object type.
#[object_type(
    workbench_type = "DTEL/DE",
    collection(
        scheme = "http://www.sap.com/wbobj/dictionary",
        term = "dtelde",
    ),
    capabilities(
        ReadProperties(
            media_version = DataElementPropertiesVersion,
            model = DataElementProperties,
        ),
        UpdateProperties,
    ),
)]
#[derive(Debug)]
pub enum DataElement {}

impl ObjectRef<DataElement> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::typed(name.to_ascii_uppercase(), uri)
    }
}
