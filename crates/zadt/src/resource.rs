mod link;
mod refs;
mod relations;
mod template;

pub use link::{AdtLink, AdtLinkError, AdvertisedLink};
pub use refs::{
    EnhancementImplementationsRef, HtmlSourceRef, ObjectEnhancementOptionsRef, ObjectStateRef,
    ObjectStructureRef, OwnedResourceRef, ParserRef, SourceEnhancementOptionsRef, SourceRef,
    SourceVersionsRef, TextElementsRef,
};
pub use relations::Relations;

pub(crate) use link::resolve_href;
pub(crate) use template::AdtUriTemplate;
