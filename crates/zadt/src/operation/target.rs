use std::collections::HashMap;

use http::Method;
use stduritemplate::Value;

use super::{
    Advertised, AdvertisedCollection, AdvertisedTarget, AdvertisedTemplate, DiscoveryDocument,
    EncodedOperation,
};
use crate::{
    AdtUri, CategoryId, Client, Collection, CompatibilityError, ObjectError, Ready, ResolveError,
    compatibility::select_accepted_media_type, resource::AdtUriTemplate,
};

/// A collection identified by its stable category in central discovery.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CollectionTarget {
    document: DiscoveryDocument,
    category: CategoryId,
}

impl CollectionTarget {
    pub(crate) const fn new(category: CategoryId) -> Self {
        Self {
            document: DiscoveryDocument::Central,
            category,
        }
    }

    pub(crate) const fn core(category: CategoryId) -> Self {
        Self {
            document: DiscoveryDocument::Core,
            category,
        }
    }

    pub(crate) fn target(self) -> AdvertisedCollection {
        match self.document {
            DiscoveryDocument::Central => AdvertisedCollection::new(self.category),
            DiscoveryDocument::Core => AdvertisedCollection::core(self.category),
        }
    }

    pub(crate) fn operation(self, method: Method) -> EncodedOperation<Advertised> {
        EncodedOperation::advertised(method, self.target())
    }

    pub(crate) fn with_segment(self, segment: impl Into<String>) -> AdvertisedCollection {
        let mut target = self.target();
        target.push_segment(segment);
        target
    }
}

/// A relation template advertised by a discovered collection.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TemplateTarget {
    collection: CollectionTarget,
    relation: &'static str,
}

impl TemplateTarget {
    pub(crate) const fn new(category: CategoryId, relation: &'static str) -> Self {
        Self {
            collection: CollectionTarget::new(category),
            relation,
        }
    }

    pub(crate) fn target(self) -> AdvertisedTemplate {
        debug_assert_eq!(self.collection.document, DiscoveryDocument::Central);
        AdvertisedTemplate::new(self.collection.category, self.relation)
    }
}

pub(crate) struct ResolvedTarget {
    pub(crate) target: AdtUri,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) content_type: Option<&'static str>,
}

impl AdvertisedTarget {
    pub(crate) fn resolve(self, client: &Client<Ready>) -> Result<ResolvedTarget, ResolveError> {
        match self {
            AdvertisedTarget::Collection(target) => target.resolve(client),
            AdvertisedTarget::Template(target) => target.resolve(client),
        }
    }
}

impl AdvertisedCollection {
    fn resolve(self, client: &Client<Ready>) -> Result<ResolvedTarget, ResolveError> {
        let collection = collection(client, self.document, self.category)?;
        let content_type = if self.accepted_media_types.is_empty() {
            None
        } else {
            Some(
                select_accepted_media_type(
                    self.accepted_media_types,
                    collection.accepted_media_types(),
                )
                .ok_or_else(|| CompatibilityError::NoCompatibleMediaType {
                    supported: self
                        .accepted_media_types
                        .iter()
                        .map(|media_type| (*media_type).to_owned())
                        .collect(),
                    accepted: collection.accepted_media_types().to_vec(),
                })?,
            )
        };
        let mut target = collection.target().map_err(ObjectError::InvalidTarget)?;
        if !self.suffix.is_empty() {
            target = target
                .append_segments(self.suffix.iter().map(String::as_str))
                .map_err(ObjectError::InvalidTarget)?;
        }
        Ok(ResolvedTarget {
            target,
            query: Vec::new(),
            content_type,
        })
    }
}

impl AdvertisedTemplate {
    fn resolve(self, client: &Client<Ready>) -> Result<ResolvedTarget, ResolveError> {
        let collection = collection(client, self.document, self.category)?;
        let variables = self
            .variables
            .into_iter()
            .map(|(name, value)| (name, Value::String(value)))
            .collect::<HashMap<_, _>>();
        let matching_template = collection
            .template_links()
            .iter()
            .filter(|link| link.relation() == self.relation)
            .find_map(|link| {
                let template = AdtUriTemplate::new(link.template());
                if !self
                    .required_variables
                    .iter()
                    .chain(self.supported_variables.iter())
                    .all(|variable| template.has_variable(variable))
                {
                    return None;
                }
                let expanded = template.expand(&variables).ok()?;
                self.required_query_parameters
                    .iter()
                    .all(|parameter| expanded.1.iter().any(|(name, _)| name == parameter))
                    .then_some((template, expanded))
            });
        let (template, expanded) = if let Some((template, expanded)) = matching_template {
            (template, Some(expanded))
        } else {
            let template = collection
                .template_links()
                .iter()
                .find(|link| link.relation() == self.relation)
                .map(|link| AdtUriTemplate::new(link.template()))
                .ok_or(ObjectError::MissingTemplate {
                    relation: self.relation,
                })?;
            (template, None)
        };

        for variable in self.required_variables {
            if !template.has_variable(variable) {
                return Err(ObjectError::InvalidTemplate {
                    template: template.as_str().to_owned(),
                    reason: format!("missing `{variable}` variable"),
                }
                .into());
            }
        }
        for variable in self.supported_variables {
            if !template.has_variable(variable) {
                return Err(ObjectError::UnsupportedTemplateParameter {
                    parameter: variable,
                }
                .into());
            }
        }

        let (target, query) = match expanded {
            Some(expanded) => expanded,
            None => template.expand(&variables)?,
        };
        for parameter in self.required_query_parameters {
            if !query.iter().any(|(name, _)| name == parameter) {
                return Err(ObjectError::InvalidTemplate {
                    template: template.as_str().to_owned(),
                    reason: format!("missing `{parameter}` query variable"),
                }
                .into());
            }
        }
        Ok(ResolvedTarget {
            target,
            query,
            content_type: None,
        })
    }
}

fn collection(
    client: &Client<Ready>,
    document: DiscoveryDocument,
    category: CategoryId,
) -> Result<&Collection, ResolveError> {
    match document {
        DiscoveryDocument::Central => client.require_collection(category),
        DiscoveryDocument::Core => client.require_core_collection(category),
    }
    .map_err(Into::into)
}
