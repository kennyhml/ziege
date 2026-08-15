use http::{Method, StatusCode};
use serde::Deserialize;
use url::Url;

use crate::{
    AdtRequest, AdtUri, Client, ClientState, DiscoveryError, Initial, Operation, OperationError,
    OperationResponse, Ready, ResponseError, Stateless,
    target::{CENTRAL_DISCOVERY, CORE_DISCOVERY},
    vocabulary::media_type,
};

/// Fetches the small, fixed ADT bootstrap capability document.
///
/// `GET /sap/bc/adt/core/discovery` returns an AtomPub service document with
/// infrastructure collections such as compatibility and batch resources. It
/// is distinct from [`DiscoveryQuery`], which advertises the domain workspaces
/// and collections used by most ADT operations. [`Client::discover`] stores
/// both documents in the resulting [`Ready`] client.
///
/// The endpoint is known in advance, so this operation can execute with any
/// [`ClientState`]. Executing it returns [`Capabilities`] but does not perform
/// the [`Client::discover`] typestate transition.
///
/// # Observed server handlers
///
/// SAP's ADT development diagnostics map this endpoint to:
///
/// - `CL_ADT_DISCOVERY_BASE_RES_APP->REGISTER_RESOURCES` for registration;
/// - `CL_ADT_RES_DISCOVERY_BASE->GET` for the `GET` implementation.
#[derive(Debug, Default)]
pub struct CoreDiscoveryQuery;

impl<S: ClientState> Operation<S> for CoreDiscoveryQuery {
    type Response = Capabilities;
    type Kind = Stateless;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = CORE_DISCOVERY.request(Method::GET);
        request.set_accept(media_type::DISCOVERY);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_discovery_response(response)
    }
}

/// Fetches the central ADT capability document.
///
/// `GET /sap/bc/adt/discovery` returns an AtomPub service document containing
/// the server's domain workspaces and collections. Each collection can
/// advertise its resource URI, stable category identities, accepted media
/// types, and URI template links.
///
/// The endpoint is known in advance, so this operation can execute with any
/// [`ClientState`]. [`Client::discover`] executes it and stores the resulting
/// [`Capabilities`] while transitioning to [`Ready`].
///
/// # Observed server handlers
///
/// SAP's ADT development diagnostics map this endpoint to:
///
/// - `CL_ADT_DISCOVERY_RES_APP->REGISTER_RESOURCES` for registration;
/// - `CL_ADT_RES_DISCOVERY->GET` for the `GET` implementation.
#[derive(Debug, Default)]
pub struct DiscoveryQuery;

impl<S: ClientState> Operation<S> for DiscoveryQuery {
    type Response = Capabilities;
    type Kind = Stateless;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = CENTRAL_DISCOVERY.request(Method::GET);
        request.set_accept(media_type::DISCOVERY);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        decode_discovery_response(response)
    }
}

impl Client<Initial> {
    /// Fetches central and core ADT discovery and returns a ready client.
    pub async fn discover(self) -> Result<Client<Ready>, OperationError> {
        let capabilities = DiscoveryQuery.execute(&self).await?;
        let core_capabilities = CoreDiscoveryQuery.execute(&self).await?;
        Ok(self.with_capabilities(capabilities, core_capabilities))
    }
}

// Both dicovery endpoints use the same format
fn decode_discovery_response(response: OperationResponse) -> Result<Capabilities, ResponseError> {
    if response.status() != StatusCode::OK {
        return Err(ResponseError::unexpected_status(response.response()));
    }

    parse_capabilities(response.body()).map_err(ResponseError::from)
}

pub(crate) fn parse_capabilities(body: &[u8]) -> Result<Capabilities, DiscoveryError> {
    serde_xml_rs::from_reader(body).map_err(Into::into)
}

/// Capabilities advertised by an ADT discovery document.
///
/// A capability document consists of one or more [`Workspace`] values, each
/// containing related [`Collection`] values. Use [`Capabilities::collection`]
/// with a category scheme and term when selecting a protocol capability.
#[derive(Debug, Deserialize)]
#[serde(rename = "app:service")]
pub struct Capabilities {
    #[serde(rename = "app:workspace", default)]
    workspaces: Vec<Workspace>,
}

impl Capabilities {
    /// Returns all workspaces in document order.
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// Finds the first collection advertising the category `scheme` and `term`.
    ///
    /// Category pairs are preferable to workspace or collection titles for
    /// capability lookup because titles are display text and can be localized.
    pub fn collection(&self, scheme: &str, term: &str) -> Option<&Collection> {
        self.workspaces
            .iter()
            .flat_map(Workspace::collections)
            .find(|collection| {
                collection
                    .categories
                    .iter()
                    .any(|category| category.scheme == scheme && category.term == term)
            })
    }
}

/// A named group of related ADT collections.
#[derive(Debug, Deserialize)]
pub struct Workspace {
    #[serde(rename = "atom:title")]
    title: String,
    #[serde(rename = "app:collection", default)]
    collections: Vec<Collection>,
}

impl Workspace {
    /// Returns the workspace's display title.
    ///
    /// The server can localize this value; do not use it as a protocol key.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the collections advertised in this workspace.
    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }
}

/// An ADT resource collection advertised by a discovery document.
#[derive(Debug, Deserialize)]
pub struct Collection {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "atom:title")]
    title: Option<String>,
    #[serde(rename = "app:accept", default)]
    accepted_media_types: Vec<String>,
    #[serde(rename = "atom:category", default)]
    categories: Vec<Category>,
    #[serde(rename = "adtcomp:templateLinks", default)]
    template_links: TemplateLinks,
}

impl Collection {
    /// Returns the collection `href` exactly as advertised by the server.
    ///
    /// Most collections use a root-relative URI, but AtomPub also permits an
    /// absolute URL. Use [`Collection::target`] when constructing an operation
    /// for the configured transport destination.
    pub fn href(&self) -> &str {
        &self.href
    }

    /// Resolves the advertised href to a destination-relative request target.
    ///
    /// For an absolute advertised `href`, this preserves the path while
    /// discarding its authority. Requests therefore remain bound to the
    /// transport's configured SAP destination.
    pub fn target(&self) -> Result<AdtUri, crate::AdtUriError> {
        collection_target(&self.href)
    }

    /// Returns the collection's optional display title.
    ///
    /// SAP can omit or localize this value; use [`Collection::categories`] to
    /// identify a capability programmatically.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Returns the representation media types accepted by the collection.
    pub fn accepted_media_types(&self) -> &[String] {
        &self.accepted_media_types
    }

    /// Returns the protocol categories advertised by the collection.
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    /// Returns the collections parameterized links to related resources.
    pub fn template_links(&self) -> &[TemplateLink] {
        &self.template_links.links
    }
}

/// A stable category identity advertised for a collection.
///
/// ADT identifies a category by the combination of its [`scheme`](Self::scheme)
/// and [`term`](Self::term).
#[derive(Debug, Deserialize)]
pub struct Category {
    #[serde(rename = "@term")]
    term: String,
    #[serde(rename = "@scheme")]
    scheme: String,
}

impl Category {
    /// Returns the category term within its scheme.
    pub fn term(&self) -> &str {
        &self.term
    }

    /// Returns the category scheme URI.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
}

/// A parameterized link from a collection to a related ADT resource.
#[derive(Debug, Deserialize)]
pub struct TemplateLink {
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@rel")]
    relation: String,
    #[serde(rename = "@template")]
    template: String,
    #[serde(rename = "@type")]
    media_type: Option<String>,
}

impl TemplateLink {
    /// Returns the optional display title of the link.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Returns the relation URI describing the link's semantics.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Returns the URI template supplied by the server.
    pub fn template(&self) -> &str {
        &self.template
    }

    /// Returns the optional media type associated with the target resource.
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }
}

#[derive(Debug, Default, Deserialize)]
struct TemplateLinks {
    #[serde(rename = "adtcomp:templateLink", default)]
    links: Vec<TemplateLink>,
}

fn collection_target(href: &str) -> Result<AdtUri, crate::AdtUriError> {
    match AdtUri::parse(href) {
        Ok(target) => Ok(target),
        Err(crate::AdtUriError::Absolute) if !href.starts_with("//") => {
            let url = Url::parse(href)?;
            if !matches!(url.scheme(), "http" | "https")
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(crate::AdtUriError::Absolute);
            }
            if url.query().is_some() || url.fragment().is_some() {
                return Err(crate::AdtUriError::QueryOrFragment);
            }
            AdtUri::parse(url.path())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectType, Program};

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");
    const INVALID_DISCOVERY_XML: &[u8] =
        include_bytes!("../../tests/fixtures/invalid-discovery.xml");

    #[test]
    fn parses_discovery_capabilities() {
        let capabilities = parse_capabilities(DISCOVERY_XML).unwrap();
        let collection = capabilities
            .collection(Program::CATEGORY.scheme, Program::CATEGORY.term)
            .unwrap();

        assert_eq!(capabilities.workspaces()[0].title(), "Programme");
        assert_eq!(collection.href(), "/sap/bc/adt/programs/programs");
        assert_eq!(collection.target().unwrap().as_str(), collection.href());
        assert_eq!(
            collection.accepted_media_types(),
            [
                "application/vnd.sap.adt.programs.programs.v2+xml",
                "application/vnd.sap.adt.programs.programs.v3+xml",
            ]
        );
        assert_eq!(
            collection.template_links()[0].relation(),
            "http://www.sap.com/adt/categories/programs/valuehelp/application"
        );

        let run_collection = capabilities
            .collection("http://www.sap.com/adt/categories/programs", "programrun")
            .unwrap();
        let run_template = &run_collection.template_links()[0];
        assert_eq!(
            run_template.relation(),
            "http://www.sap.com/adt/relations/programs/programrun"
        );
        assert_eq!(
            run_template.template(),
            "/sap/bc/adt/programs/programrun/{programname}{?profilerId}"
        );
        assert_eq!(run_template.media_type(), Some("text/plain"));
    }

    #[test]
    fn defers_collection_target_validation() {
        let capabilities = parse_capabilities(INVALID_DISCOVERY_XML).unwrap();
        let collection = capabilities
            .collection(Program::CATEGORY.scheme, Program::CATEGORY.term)
            .unwrap();

        assert!(collection.target().is_err());
    }

    #[test]
    fn rejects_malformed_discovery_xml() {
        let error = parse_capabilities(b"<app:service>").unwrap_err();

        assert!(matches!(error, DiscoveryError::Xml(_)));
    }

    #[test]
    fn accepts_a_collection_without_a_title() {
        let capabilities = parse_capabilities(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom">
                    <app:workspace>
                        <atom:title>Services</atom:title>
                        <app:collection href="/sap/bc/adt/services" />
                    </app:workspace>
                </app:service>"#,
        )
        .unwrap();

        assert_eq!(capabilities.workspaces()[0].collections()[0].title(), None);
    }

    #[test]
    fn retains_an_absolute_href_with_a_destination_relative_target() {
        let capabilities = parse_capabilities(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom">
                    <app:workspace>
                        <atom:title>Transport</atom:title>
                        <app:collection href="https://sap.example.test:44300/sap/bc/adt">
                            <atom:title>ADT endpoint</atom:title>
                        </app:collection>
                    </app:workspace>
                </app:service>"#,
        )
        .unwrap();
        let collection = &capabilities.workspaces()[0].collections()[0];

        assert_eq!(
            collection.href(),
            "https://sap.example.test:44300/sap/bc/adt"
        );
        assert_eq!(collection.target().unwrap().as_str(), "/sap/bc/adt");
    }
}
