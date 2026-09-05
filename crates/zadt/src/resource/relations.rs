use crate::AdtUri;

use super::{AdtLink, AdtLinkError, AdvertisedLink};

/// Maps relations of an object reference and enables lazy evaluation
/// of possible references. While the underlying [`crate::ObjectKey`] could
/// be a statically known [`crate::ObjectKey<T>`], as the relations can only
/// be found by having a reference to such, it should always be used
/// as an implementation detail, only to be called on directly in
/// exceptional cases, such as when access to a relation is not yet
/// supported statically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relations {
    base: AdtUri,
    links: Box<[AdvertisedLink]>,
}

impl Relations {
    pub(crate) fn for_base(base: AdtUri, links: Vec<AdvertisedLink>) -> Self {
        Self {
            base,
            links: links.into_boxed_slice(),
        }
    }

    /// Returns the number of advertised links without resolving them.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Returns whether no links were advertised.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Resolves advertised links in document order.
    pub fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = Result<AdtLink, AdtLinkError>> + 'a {
        self.links.iter().map(|link| link.resolve(&self.base))
    }
}
