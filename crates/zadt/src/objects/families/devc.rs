use super::super::ObjectRef;
use crate::models::{PackageProperties, PackagePropertiesVersion};
use zadt_macros::object_type;

/// The package (devclass) object type.
#[object_type(
    workbench_type = "DEVC/K",
    collection(
        scheme = "http://www.sap.com/wbobj/packages",
        term = "devck",
    ),
    capabilities(Properties(
        media_version = PackagePropertiesVersion,
        model = PackageProperties,
    )),
)]
#[derive(Debug)]
pub enum Package {}

impl ObjectRef<Package> {
    #[cfg(test)]
    pub(crate) fn for_test(name: &str, uri: crate::AdtUri) -> Self {
        Self::typed(name.to_ascii_uppercase(), uri)
    }
}
