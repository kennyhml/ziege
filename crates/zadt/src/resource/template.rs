use std::collections::HashMap;

use stduritemplate::Value;
use url::Url;

use crate::{AdtUri, AdtUriError, ObjectError};

/// A URI template advertised by an ADT resource or discovery collection.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AdtUriTemplate<'a> {
    template: &'a str,
}

impl<'a> AdtUriTemplate<'a> {
    pub(crate) const fn new(template: &'a str) -> Self {
        Self { template }
    }

    pub(crate) fn as_str(self) -> &'a str {
        self.template
    }

    pub(crate) fn has_variable(self, expected: &str) -> bool {
        let mut remaining = self.template;
        while let Some(start) = remaining.find('{') {
            remaining = &remaining[start + 1..];
            let Some(end) = remaining.find('}') else {
                return false;
            };
            let expression = &remaining[..end];
            let expression = expression
                .chars()
                .next()
                .filter(|operator| "+#./;?&".contains(*operator))
                .map_or(expression, |operator| &expression[operator.len_utf8()..]);
            if expression.split(',').any(|variable| {
                let variable = variable.strip_suffix('*').unwrap_or(variable);
                variable.split_once(':').map_or(variable, |(name, _)| name) == expected
            }) {
                return true;
            }
            remaining = &remaining[end + 1..];
        }
        false
    }

    pub(crate) fn expand(
        self,
        variables: &HashMap<String, Value>,
    ) -> Result<(AdtUri, Vec<(String, String)>), ObjectError> {
        let expanded = stduritemplate::expand(self.template, variables).map_err(|error| {
            ObjectError::InvalidTemplate {
                template: self.template.to_owned(),
                reason: error.to_string(),
            }
        })?;
        let invalid_target = |source| ObjectError::InvalidExpandedTarget {
            target: expanded.clone(),
            source,
        };
        let (path, query) = match Url::parse(&expanded) {
            Ok(url) => {
                if !matches!(url.scheme(), "http" | "https")
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return Err(invalid_target(AdtUriError::Absolute));
                }
                if url.fragment().is_some() {
                    return Err(invalid_target(AdtUriError::QueryOrFragment));
                }
                (url.path().to_owned(), url.query().map(str::to_owned))
            }
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                if expanded.starts_with("//") {
                    return Err(invalid_target(AdtUriError::Absolute));
                }
                if expanded.contains('#') {
                    return Err(invalid_target(AdtUriError::QueryOrFragment));
                }
                expanded.split_once('?').map_or_else(
                    || (expanded.clone(), None),
                    |(path, query)| (path.to_owned(), Some(query.to_owned())),
                )
            }
            Err(error) => return Err(invalid_target(AdtUriError::Url(error))),
        };
        let target = AdtUri::parse(&path).map_err(invalid_target)?;
        let query = query
            .map(|query| {
                url::form_urlencoded::parse(query.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_default();
        Ok((target, query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_path_and_query_variables() {
        let template = AdtUriTemplate::new("/sap/bc/adt/oo/classrun/{classname}{?profilerId}");
        let variables = HashMap::from([
            (
                "classname".to_owned(),
                Value::String("/DMO/CLASS".to_owned()),
            ),
            (
                "profilerId".to_owned(),
                Value::String("TRACE ID".to_owned()),
            ),
        ]);

        let (target, query) = template.expand(&variables).unwrap();

        assert_eq!(target.as_str(), "/sap/bc/adt/oo/classrun/%2FDMO%2FCLASS");
        assert_eq!(query, [("profilerId".to_owned(), "TRACE ID".to_owned())]);
        assert!(template.has_variable("classname"));
        assert!(template.has_variable("profilerId"));
        assert!(!template.has_variable("programname"));
    }

    #[test]
    fn keeps_absolute_templates_bound_to_the_configured_destination() {
        let template = AdtUriTemplate::new(
            "https://backend.example/sap/bc/adt/programs/programrun/{programname}",
        );
        let variables =
            HashMap::from([("programname".to_owned(), Value::String("Z_TEST".to_owned()))]);

        let (target, query) = template.expand(&variables).unwrap();

        assert_eq!(target.as_str(), "/sap/bc/adt/programs/programrun/Z_TEST");
        assert!(query.is_empty());
    }
}
