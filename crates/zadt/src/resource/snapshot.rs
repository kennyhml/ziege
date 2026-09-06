use crate::{
    AdtUri, ClassSourceProperties, compatibility::media_types_match,
    protocol::TEXT_PLAIN_MEDIA_TYPE,
};

use super::{AdtLink, AdtLinkError, AdvertisedLink, resolve_href};

const SOURCE_RELATION: &str = "http://www.sap.com/adt/relations/source";

/// A borrowed resource location and its object or component scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotResource<'a> {
    component: Option<&'a str>,
    href: &'a str,
    link: Option<&'a AdvertisedLink>,
    source: bool,
}

impl<'a> SnapshotResource<'a> {
    /// The source component owning this resource, or `None` for object-level links.
    /// Primary sources use the component name `main` for every object family.
    pub fn component(&self) -> Option<&'a str> {
        self.component
    }

    /// The authoritative resource href, borrowed from the wire properties.
    pub fn href(&self) -> &'a str {
        self.href
    }

    /// Original borrowed metadata, if a matching link was advertised.
    ///
    /// A source's authoritative href can differ from this link's href. Href-only
    /// sources have no link; use the resource accessors for normalized metadata.
    pub fn link(&self) -> Option<&'a AdvertisedLink> {
        self.link
    }

    pub fn relation(&self) -> Option<&'a str> {
        self.link
            .and_then(|link| link.relation.as_deref())
            .or(self.source.then_some(SOURCE_RELATION))
    }

    pub fn media_type(&self) -> Option<&'a str> {
        self.link
            .and_then(|link| link.media_type.as_deref())
            .or(self.source.then_some(TEXT_PLAIN_MEDIA_TYPE))
    }

    /// The advertised representation ETag, not the object's properties ETag.
    pub fn etag(&self) -> Option<&'a str> {
        self.link.and_then(|link| link.etag.as_deref())
    }

    pub fn title(&self) -> Option<&'a str> {
        self.link.and_then(|link| link.title.as_deref())
    }

    pub fn hreflang(&self) -> Option<&'a str> {
        self.link.and_then(|link| link.hreflang.as_deref())
    }

    pub fn length(&self) -> Option<&'a str> {
        self.link.and_then(|link| link.length.as_deref())
    }

    /// Resolves this resource against its snapshot's URI into an owned link.
    pub fn resolve(&self, base: &AdtUri) -> Result<AdtLink, AdtLinkError> {
        let resolved = resolve_href(base, self.href).map_err(|source| AdtLinkError {
            href: self.href.to_owned(),
            source,
        })?;
        Ok(AdtLink {
            href: self.href.to_owned(),
            target: resolved.target,
            query: resolved.query,
            fragment: resolved.fragment,
            relation: self.relation().map(str::to_owned),
            media_type: self.media_type().map(str::to_owned),
            etag: self.etag().map(str::to_owned),
            title: self.title().map(str::to_owned),
            hreflang: self.hreflang().map(str::to_owned),
            length: self.length().map(str::to_owned),
        })
    }

    fn advertised(component: Option<&'a str>, link: &'a AdvertisedLink) -> Self {
        Self {
            component,
            href: &link.href,
            link: Some(link),
            source: false,
        }
    }

    fn source(
        base: &AdtUri,
        name: &'a str,
        href: &'a str,
        links: &'a [AdvertisedLink],
    ) -> (Self, Option<usize>) {
        let location = resolve_href(base, href).ok();
        let selected = links
            .iter()
            .enumerate()
            .filter(|(_, link)| {
                link.relation.as_deref() == Some(SOURCE_RELATION)
                    && link.media_type.as_deref().is_none_or(|media_type| {
                        media_types_match(TEXT_PLAIN_MEDIA_TYPE, media_type)
                    })
                    && location.as_ref().is_some_and(|location| {
                        resolve_href(base, &link.href).is_ok_and(|candidate| {
                            candidate.target == location.target
                                && candidate.query == location.query
                                && candidate.fragment == location.fragment
                        })
                    })
            })
            .min_by_key(|(_, link)| link.media_type.is_none());
        (
            Self {
                component: Some(name),
                href,
                link: selected.map(|(_, link)| link),
                source: true,
            },
            selected.map(|(index, _)| index),
        )
    }
}

/// A base-independent description of resources borrowed from wire properties.
///
/// Returned by [`crate::Resources`]. It contains no object URI or owned metadata;
/// snapshots supply their own URI when exposing navigation through this view.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceView<'a> {
    links: &'a [AdvertisedLink],
    syntax_links: &'a [AdvertisedLink],
    main: Option<&'a str>,
    components: &'a [ClassSourceProperties],
}

impl<'a> ResourceView<'a> {
    /// Describes the links advertised at the object's root.
    pub fn new(links: &'a [AdvertisedLink]) -> Self {
        Self {
            links,
            ..Self::default()
        }
    }

    /// Adds links advertised by the object's syntax language.
    #[must_use]
    pub fn with_syntax_links(mut self, links: &'a [AdvertisedLink]) -> Self {
        self.syntax_links = links;
        self
    }

    /// Identifies a primary source whose metadata is in the root links.
    #[must_use]
    pub fn with_main(mut self, href: &'a str) -> Self {
        self.main = Some(href);
        self
    }

    /// Describes source components with their individually advertised metadata.
    #[must_use]
    pub fn with_components(mut self, components: &'a [ClassSourceProperties]) -> Self {
        self.components = components;
        self
    }

    pub fn links(&self) -> &'a [AdvertisedLink] {
        self.links
    }

    pub fn syntax_links(&self) -> &'a [AdvertisedLink] {
        self.syntax_links
    }

    /// The primary source href for flat properties; classes use their main component.
    pub fn main_source_uri(&self) -> Option<&'a str> {
        self.main
    }

    pub fn components(&self) -> &'a [ClassSourceProperties] {
        self.components
    }
}

/// An on-demand navigation view bound to its snapshot's object URI.
///
/// Creating or copying the view does not clone metadata or allocate a collection.
/// Iteration and source selection borrow entries lazily; matching equivalent hrefs
/// may allocate temporary URI data. Only resolution or handle creation owns metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotResources<'a> {
    base: &'a AdtUri,
    view: ResourceView<'a>,
}

impl<'a> SnapshotResources<'a> {
    /// Iterates without collecting entries or borrowing the temporary view itself.
    pub fn iter(&self) -> impl Iterator<Item = SnapshotResource<'a>> + 'a {
        let Self { base, view } = *self;
        let ResourceView {
            links,
            syntax_links,
            main,
            components,
        } = view;
        links
            .iter()
            .chain(syntax_links)
            .map(|link| SnapshotResource::advertised(None, link))
            .chain(
                main.into_iter()
                    .map(move |href| SnapshotResource::source(base, "main", href, links).0),
            )
            .chain(components.iter().flat_map(move |component| {
                let name = component.include_type.as_str();
                let (source, selected) =
                    SnapshotResource::source(base, name, &component.source_uri, &component.links);
                std::iter::once(source).chain(
                    component
                        .links
                        .iter()
                        .enumerate()
                        .filter(move |(index, _)| selected != Some(*index))
                        .map(move |(_, link)| SnapshotResource::advertised(Some(name), link)),
                )
            }))
    }

    /// Counts the projected entries without retaining them.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.view.links.is_empty()
            && self.view.syntax_links.is_empty()
            && self.view.main.is_none()
            && self.view.components.is_empty()
    }

    pub(crate) fn new(base: &'a AdtUri, view: ResourceView<'a>) -> Self {
        Self { base, view }
    }

    pub(crate) fn source(&self, name: &str) -> Option<SnapshotResource<'a>> {
        if let Some(href) = self.view.main.filter(|_| name == "main") {
            return Some(SnapshotResource::source(self.base, "main", href, self.view.links).0);
        }
        self.view
            .components
            .iter()
            .find(|component| component.include_type == name)
            .map(|component| {
                SnapshotResource::source(
                    self.base,
                    &component.include_type,
                    &component.source_uri,
                    &component.links,
                )
                .0
            })
    }

    pub(crate) fn object_link(&self, relation: &str) -> Option<&'a AdvertisedLink> {
        self.view
            .links
            .iter()
            .chain(self.view.syntax_links)
            .find(|link| link.relation.as_deref() == Some(relation))
    }
}
