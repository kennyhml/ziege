use serde::{Deserialize, de::DeserializeOwned};

use crate::ProjectionError;

#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct JsonObject<T>(#[serde(deserialize_with = "object")] T);

/// Parses an AFF document as an object, never a positional struct array.
pub(crate) fn parse_object<T: DeserializeOwned>(content: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str::<JsonObject<T>>(content).map(|object| object.0)
}

/// Restricts a nested struct to its JSON object representation.
pub(crate) fn object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Visitor<T>(std::marker::PhantomData<T>);

    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<T> {
        type Value = T;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an AFF object")
        }

        fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_map(Visitor(std::marker::PhantomData))
}

pub(crate) fn optional_object<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    object(deserializer).map(Some)
}

pub(crate) fn objects<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Vec::<JsonObject<T>>::deserialize(deserializer)
        .map(|values| values.into_iter().map(|value| value.0).collect())
}

/// AFF enums are strings, not externally tagged JSON objects.
pub(crate) fn string_enum<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = String::deserialize(deserializer)?;
    T::deserialize(serde::de::value::StringDeserializer::new(value))
}

/// Allows omission while rejecting explicit JSON null for optional schema fields.
pub(crate) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

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
