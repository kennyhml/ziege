use crate::ProjectionError;

pub(crate) fn required<T>(value: Option<T>, field: &'static str) -> Result<T, ProjectionError> {
    value.ok_or_else(|| ProjectionError::InvalidDataElementField {
        field,
        message: "is required by AFF".to_owned(),
    })
}

pub(crate) fn nonempty(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(str::to_owned)
}

pub(crate) fn nonzero(value: Option<u32>) -> Option<u32> {
    value.filter(|value| *value != 0)
}

pub(crate) fn is_false(value: &bool) -> bool {
    !value
}
