use crate::ProjectionError;

pub(crate) fn validate_object_name(object_name: &str) -> Result<(), ProjectionError> {
    if object_name.is_empty()
        || object_name.trim() != object_name
        || object_name.chars().any(char::is_control)
        || object_name.contains(['\\', '<', '>'])
        || matches!(object_name, "." | "..")
    {
        return Err(ProjectionError::InvalidObjectName {
            object_name: object_name.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn validate_language(language: &str) -> Result<(), ProjectionError> {
    if language.is_empty()
        || language
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    {
        return Err(ProjectionError::InvalidLanguage {
            language: language.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn one_of<'a>(allowed: impl AsRef<[&'a str]>) -> impl Fn(&str, &()) -> garde::Result {
    move |value, _| {
        let allowed = allowed.as_ref();
        if allowed.contains(&value) {
            Ok(())
        } else {
            Err(garde::Error::new(format!(
                "expected one of: {}",
                allowed.join(", ")
            )))
        }
    }
}

/// JSON Schema uniqueness compares complete entries, not just their names.
pub(crate) fn unique_items<T: PartialEq>(values: &[T], _: &()) -> garde::Result {
    if values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
    {
        return Err(garde::Error::new("duplicate entries"));
    }
    Ok(())
}
