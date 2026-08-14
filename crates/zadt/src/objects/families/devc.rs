use crate::models::PackageProperties;
use zadt_macros::object_type;

/// The package (devclass) object type.
#[object_type(
    workbench_type = "DEVC/K",
    collection(
        scheme = "http://www.sap.com/wbobj/packages",
        term = "devck",
    ),
    capabilities(ReadProperties(model = PackageProperties)),
)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Package;
