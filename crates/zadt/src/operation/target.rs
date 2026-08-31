use std::collections::HashMap;

use http::Method;
use stduritemplate::Value;

use super::{Advertised, EncodedOperation};
use crate::{
    AdtUri, CategoryId, Client, CompatibilityError, ObjectError, Ready, ResolveError,
    compatibility, resource::AdtUriTemplate,
};

/// An advertised collection or URI-template target.
#[derive(Debug)]
pub enum AdvertisedTarget {
    Collection(AdvertisedCollection),
    Template(AdvertisedTemplate),
}

impl AdvertisedTarget {
    /// Dispatches the resolve call to its internal target.
    pub(crate) fn resolve(self, client: &Client<Ready>) -> Result<ResolvedTarget, ResolveError> {
        match self {
            AdvertisedTarget::Collection(target) => target.resolve(client),
            AdvertisedTarget::Template(target) => target.resolve(client),
        }
    }
}

impl From<AdvertisedCollection> for AdvertisedTarget {
    fn from(target: AdvertisedCollection) -> Self {
        Self::Collection(target)
    }
}

impl From<AdvertisedTemplate> for AdvertisedTarget {
    fn from(target: AdvertisedTemplate) -> Self {
        Self::Template(target)
    }
}

/// Selects the discovery document containing an advertised resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryDocument {
    /// Main discovery at `/sap/bc/adt/discovery`
    Central,
    /// Core discovery at `/sap/bc/adt/core/discovery`
    Core,
}

/// A collection locator resolved from an ADT discovery document during execution.
#[derive(Debug)]
pub struct AdvertisedCollection {
    /// Which discovery document is this collection advertised in
    document: DiscoveryDocument,
    /// The collection category identifier
    category: CategoryId,
    /// Path segments to append to the discovered url
    path_segments: Vec<String>,
    /// Media types to check against the accepted types, usually create requests
    accepted_media_types: &'static [&'static str],
}

impl AdvertisedCollection {
    /// Selects a collection from central ADT discovery.
    pub fn new(category: CategoryId) -> Self {
        Self::in_document(DiscoveryDocument::Central, category)
    }

    /// Selects a collection from core ADT discovery.
    pub fn core(category: CategoryId) -> Self {
        Self::in_document(DiscoveryDocument::Core, category)
    }

    fn in_document(document: DiscoveryDocument, category: CategoryId) -> Self {
        Self {
            document,
            category,
            path_segments: Vec::new(),
            accepted_media_types: &[],
        }
    }

    /// Appends one safely encoded segment to a collection target.
    pub fn push_segment(&mut self, segment: impl Into<String>) {
        self.path_segments.push(segment.into());
    }

    /// Requires that the collection accepts any of the given media types.
    pub(crate) fn require_accepted_media_types(&mut self, media_types: &'static [&'static str]) {
        self.accepted_media_types = media_types;
    }

    /// Internal helper to resolve the collection against a client that holds
    /// discovery data and check whether the required media type is accepted.
    fn resolve(self, client: &Client<Ready>) -> Result<ResolvedTarget, ResolveError> {
        let collection = match self.document {
            DiscoveryDocument::Central => client.require_collection(self.category),
            DiscoveryDocument::Core => client.require_core_collection(self.category),
        }?;

        let content_type = if self.accepted_media_types.is_empty() {
            None
        } else {
            Some(
                compatibility::select_accepted_media_type(
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

        // The uri target only identifies the base path of the resource. If we want
        // to query a certain resource, the path segments need to be appended.
        let mut target = collection.target().map_err(ObjectError::InvalidTarget)?;
        if !self.path_segments.is_empty() {
            target = target
                .append_segments(self.path_segments.iter().map(String::as_str))
                .map_err(ObjectError::InvalidTarget)?;
        }

        Ok(ResolvedTarget {
            target,
            query: Vec::new(),
            content_type,
        })
    }
}

/// A URI-template locator resolved from an ADT discovery document during execution.
///
/// Example advertisement:
/// ```xml
/// <adtcomp:templateLink
///     rel="http://www.sap.com/adt/relations/informationsystem/textsearch"
///     template="/sap/bc/adt/repository/informationsystem/textsearch{?searchString,
///               searchFromIndex,searchToIndex,getAllResults}{&amp;packageName*}
///               {&amp;userName*}{&amp;objectName*}{&amp;objectType*}"/>
/// ```
/// The `&param*` denotes that `param` may occur more than once.
#[derive(Debug)]
pub struct AdvertisedTemplate {
    /// Which discovery document is this collection advertised in
    document: DiscoveryDocument,
    /// The category that identifies the collection
    category: CategoryId,
    /// The relation that identifies the template
    relation: &'static str,
    /// The values to substitute.
    variables: Vec<TemplateVariable>,
}

impl AdvertisedTemplate {
    /// Selects a URI template from a central-discovery collection.
    pub fn new(category: CategoryId, relation: &'static str) -> Self {
        Self {
            document: DiscoveryDocument::Central,
            category,
            relation,
            variables: Vec::new(),
        }
    }

    /// Supplies a variable for URI-template expansion.
    pub fn push_variable(&mut self, name: &'static str, value: impl Into<String>) {
        self.variables.push(TemplateVariable {
            name,
            value: Value::String(value.into()),
        });
    }

    /// Supplies a list variable for exploded URI-template expansion.
    pub fn push_list_variable<I, V>(&mut self, name: &'static str, values: I)
    where
        I: IntoIterator<Item = V>,
        V: Into<String>,
    {
        self.variables.push(TemplateVariable {
            name,
            value: Value::List(
                values
                    .into_iter()
                    .map(|value| Value::String(value.into()))
                    .collect(),
            ),
        });
    }

    /// Resolves the template by fetching its definition from the discovery, then
    /// substituting the passed variables and query parameters into it.
    fn resolve(self, client: &Client<Ready>) -> Result<ResolvedTarget, ResolveError> {
        let collection = match self.document {
            DiscoveryDocument::Central => client.require_collection(self.category),
            DiscoveryDocument::Core => client.require_core_collection(self.category),
        }?;

        let template = collection
            .template_links()
            .iter()
            .find(|link| link.relation() == self.relation)
            .map(|link| AdtUriTemplate::new(link.template()))
            .ok_or(ObjectError::MissingTemplate {
                relation: self.relation,
            })?;

        // At this point we have a loaded representation of the targetted template, now
        // we just need to see if our way of calling it fits the definition.
        let variables = self
            .variables
            .into_iter()
            .map(|variable| {
                template
                    .has_variable(variable.name)
                    .then_some((variable.name.to_owned(), variable.value))
                    .ok_or(ObjectError::UnsupportedTemplateParameter {
                        parameter: variable.name,
                    })
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        let (target, query) = template.expand(&variables)?;
        Ok(ResolvedTarget {
            target,
            query,
            content_type: None,
        })
    }
}

/// One supplied URI-template variable.
///
/// Uses `stduritemplate::Value` internally to represent different
/// value types, such as lists.
#[derive(Debug)]
struct TemplateVariable {
    name: &'static str,
    value: Value,
}

/// A collection identified by its stable category in central discovery.
///
/// Operations can use this to declare their dependency on some collection
/// during operation creation. The locator later transitions into an
/// [`AdvertisedCollection`] with an optional set of path segments.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CollectionLocator {
    document: DiscoveryDocument,
    category: CategoryId,
}

impl CollectionLocator {
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
///
/// Operations can use this to declare their dependency on some template
/// during operation creation. The locator later transitions into an
/// [`AdvertisedTemplate`] together with all the variables to expand.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TemplateLocator {
    collection: CollectionLocator,
    relation: &'static str,
}

impl TemplateLocator {
    pub(crate) const fn new(category: CategoryId, relation: &'static str) -> Self {
        Self {
            collection: CollectionLocator::new(category),
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
