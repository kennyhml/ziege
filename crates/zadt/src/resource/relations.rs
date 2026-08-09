use crate::{AdtUri, ObjectRef};
use serde::Serialize;

use super::{AdtLink, AdtLinkError, AdvertisedLink, OwnedResourceRef, refs::FromAdtLink};

/// Maps relations of an object reference and enables lazy evaluation
/// of possible references. While the underlying [`ObjectRef`] could
/// be a statically known [`ObjectRef<T>`], as the relations can only
/// be found by having a reference to such, it should always be used
/// as an implementation detail, only to be called on directly in
/// exceptional cases, such as when access to a relation is not yet
/// supported statically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relations {
    owner: ObjectRef,
    links: Box<[AdvertisedLink]>,
}

impl Serialize for Relations {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.links
            .iter()
            .map(|link| SerializedRelation::new(link, self.owner.uri()))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedRelation {
    href: String,
    target: Option<AdtUri>,
    query: Vec<(String, String)>,
    fragment: Option<String>,
    relation: Option<String>,
    media_type: Option<String>,
    hreflang: Option<String>,
    title: Option<String>,
    length: Option<String>,
    etag: Option<String>,
    resolved: bool,
    resolution_error: Option<String>,
}

impl SerializedRelation {
    fn new(link: &AdvertisedLink, base: &AdtUri) -> Self {
        let (target, query, fragment, resolution_error) = match link.resolve(base) {
            Ok(link) => (Some(link.target), link.query, link.fragment, None),
            Err(error) => (None, Vec::new(), None, Some(error.to_string())),
        };
        Self {
            href: link.href.clone(),
            target,
            query,
            fragment,
            relation: link.relation.clone(),
            media_type: link.media_type.clone(),
            hreflang: link.hreflang.clone(),
            title: link.title.clone(),
            length: link.length.clone(),
            etag: link.etag.clone(),
            resolved: resolution_error.is_none(),
            resolution_error,
        }
    }
}

impl Relations {
    pub(crate) fn new(owner: ObjectRef, links: Vec<AdvertisedLink>) -> Self {
        Self {
            owner,
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
        self.links.iter().map(|link| link.resolve(self.owner.uri()))
    }

    /// Resolves and converts the relation associated to `R`.
    ///
    /// The link is resolved against the owner of the relation as the base,
    /// enforcing a valid resource reference.
    pub(crate) fn get<R>(&self) -> Result<Option<OwnedResourceRef<R>>, AdtLinkError>
    where
        R: FromAdtLink,
    {
        self.links
            .iter()
            .find(|link| link.matches(R::RELATION, R::MEDIA_TYPE))
            .map(|link| {
                link.resolve(self.owner.uri())
                    .map(|link| R::from_adt_link(&self.owner, &link))
            })
            .transpose()
    }

    pub(crate) fn find(&self, relation: &str) -> Result<Option<AdtLink>, AdtLinkError> {
        self.links
            .iter()
            .find(|link| link.relation.as_deref() == Some(relation))
            .map(|link| link.resolve(self.owner.uri()))
            .transpose()
    }
}
