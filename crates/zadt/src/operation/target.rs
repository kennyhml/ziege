use http::Method;

use crate::{
    AdtRequest, CategoryId, Client, Collection, ObjectError, OperationError, Ready,
    resource::AdtUriTemplate,
};

/// A collection identified by its stable category in central discovery.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CollectionTarget(CategoryId);

impl CollectionTarget {
    pub(crate) const fn new(category: CategoryId) -> Self {
        Self(category)
    }

    pub(crate) fn collection(self, client: &Client<Ready>) -> Result<&Collection, OperationError> {
        client.require_collection(self.0).map_err(Into::into)
    }

    pub(crate) fn request(
        self,
        client: &Client<Ready>,
        method: Method,
    ) -> Result<AdtRequest, OperationError> {
        let target = self
            .collection(client)?
            .target()
            .map_err(ObjectError::InvalidTarget)?;
        Ok(AdtRequest::new(method, target))
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

    pub(crate) fn template<'a>(
        self,
        client: &'a Client<Ready>,
    ) -> Result<AdtUriTemplate<'a>, OperationError> {
        self.collection
            .collection(client)?
            .template_links()
            .iter()
            .find(|link| link.relation() == self.relation)
            .map(|link| AdtUriTemplate::new(link.template()))
            .ok_or_else(|| ObjectError::MissingTemplate {
                relation: self.relation,
            })
            .map_err(Into::into)
    }
}
