use crate::models::DataElementProperties;
use zadt_macros::object_type;

/// The ABAP Dictionary Data Element object type.
#[object_type(
    workbench_type = "DTEL/DE",
    collection(
        scheme = "http://www.sap.com/wbobj/dictionary",
        term = "dtelde",
    ),
    capabilities(
        ReadProperties(model = DataElementProperties),
        UpdateProperties,
    ),
)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DataElement;
