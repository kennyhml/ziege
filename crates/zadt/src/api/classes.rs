use super::properties::ObjectPropertiesQuery;
use crate::Class;

/// Fetches class properties using the generic object-properties protocol.
pub type ClassPropertiesQuery = ObjectPropertiesQuery<Class>;
