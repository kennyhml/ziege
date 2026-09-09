use crate::ProjectionError;

pub(crate) fn validate_object_name(
    object_name: &str,
) -> Result<zadt::ObjectName<'_>, ProjectionError> {
    zadt::ObjectName::parse(object_name).map_err(|_| ProjectionError::InvalidObjectName {
        object_name: object_name.to_owned(),
    })
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

/// Requires a nonempty string of ASCII decimal digits.
pub(crate) fn numeric_string(value: &str, _: &()) -> garde::Result {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(garde::Error::new("expected ASCII decimal digits"));
    }
    Ok(())
}

/// Validates an optional RFC 3339 full-date, including Gregorian leap years.
pub(crate) fn optional_date(value: &Option<String>, _: &()) -> garde::Result {
    let Some(value) = value else {
        return Ok(());
    };
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return Err(garde::Error::new("expected a date in YYYY-MM-DD format"));
    }
    let year = value[..4].parse::<u32>().unwrap();
    let month = value[5..7].parse::<u32>().unwrap();
    let day = value[8..].parse::<u32>().unwrap();
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    };
    if day == 0 || day > days {
        return Err(garde::Error::new("expected a valid Gregorian date"));
    }
    Ok(())
}
