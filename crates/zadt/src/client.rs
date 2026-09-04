use std::sync::Arc;

use crate::{
    AdtUri, BatchOperation, Capabilities, CategoryId, Collection, CompatibilityError, ObjectError,
    ResolveError, Stateless, TemplateLink, Transport, UserSession,
};

mod private {
    pub trait Sealed {}
}

/// Marker for the protocol lifecycle state of an ADT client.
///
/// ADT advertises top-level collections, URI templates, and accepted media
/// types through `/sap/bc/adt/discovery`. Operations with an advertised target
/// can only execute through a client that has completed discovery.
pub trait ClientState: private::Sealed + Clone + Send + Sync {}

/// The client has not loaded the server's central ADT capabilities.
#[derive(Clone, Debug, Default)]
pub struct Initial;

/// Central and core ADT discovery data used to resolve operation targets.
///
/// This also serves as the client state proving discovery has completed. It is
/// only a local capability snapshot and does not guarantee that authentication
/// or the underlying transport remains alive.
#[derive(Clone, Debug)]
pub struct Discovery {
    capabilities: Arc<Capabilities>,
    core_capabilities: Arc<Capabilities>,
}

impl private::Sealed for Initial {}
impl private::Sealed for Discovery {}
impl ClientState for Initial {}
impl ClientState for Discovery {}

/// A client for executing typed ADT operations.
///
/// The operations available to a client depend on the [`ClientState`] marker
/// `S`. The client owns protocol-level state such as loaded capabilities,
/// while request delivery is delegated to a [`Transport`] implementation.
///
/// A transport may send ADT requests over HTTP directly or through an adapter,
/// such as a future RFC bridge, without changing the operation API.
///
/// Because clients may be shared across different contexts, it must be possible
/// to clone it cheaply.
#[derive(Clone)]
pub struct Client<S = Initial> {
    transport: Arc<dyn Transport>,
    state: S,
}

impl Client<Initial> {
    /// Creates a client that has not loaded central ADT capabilities.
    pub fn new(transport: impl Transport + 'static) -> Self {
        Self {
            transport: Arc::new(transport),
            state: Initial,
        }
    }

    pub(crate) fn with_capabilities(
        self,
        capabilities: Capabilities,
        core_capabilities: Capabilities,
    ) -> Client<Discovery> {
        Client {
            transport: self.transport,
            state: Discovery {
                capabilities: Arc::new(capabilities),
                core_capabilities: Arc::new(core_capabilities),
            },
        }
    }
}

impl Client<Discovery> {
    /// Returns the discovery data used to resolve advertised ADT resources.
    pub fn discovery(&self) -> &Discovery {
        &self.state
    }

    /// Creates an empty stateless batch bound to this client.
    pub fn batch(&self) -> BatchOperation<'_, Stateless> {
        BatchOperation::for_client(self)
    }
}

impl Discovery {
    /// Returns the capabilities advertised by central ADT discovery.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Returns the capabilities advertised by core ADT discovery.
    pub fn core_capabilities(&self) -> &Capabilities {
        &self.core_capabilities
    }

    /// Returns a central-discovery collection by category identity.
    pub fn collection(&self, category: CategoryId) -> Option<&Collection> {
        self.capabilities()
            .collection(category.scheme, category.term)
    }

    /// Returns a central-discovery template by collection and relation.
    pub fn template(&self, category: CategoryId, relation: &str) -> Option<&TemplateLink> {
        self.capabilities()
            .template(category.scheme, category.term, relation)
    }

    pub(crate) fn require_template(
        &self,
        category: CategoryId,
        relation: &'static str,
    ) -> Result<&TemplateLink, crate::ResolveError> {
        self.require_collection(category)?;
        self.template(category, relation)
            .ok_or(crate::ObjectError::MissingTemplate { relation })
            .map_err(Into::into)
    }

    pub(crate) fn require_collection(
        &self,
        category: CategoryId,
    ) -> Result<&Collection, CompatibilityError> {
        self.collection(category)
            .ok_or(CompatibilityError::MissingCollection(category))
    }

    pub(crate) fn require_collection_target(
        &self,
        category: CategoryId,
    ) -> Result<AdtUri, ResolveError> {
        self.require_collection(category)?
            .target()
            .map_err(ObjectError::InvalidTarget)
            .map_err(Into::into)
    }

    pub(crate) fn require_core_collection(
        &self,
        category: CategoryId,
    ) -> Result<&Collection, CompatibilityError> {
        self.core_capabilities()
            .collection(category.scheme, category.term)
            .ok_or(CompatibilityError::MissingCollection(category))
    }

    pub(crate) fn require_core_collection_target(
        &self,
        category: CategoryId,
    ) -> Result<AdtUri, ResolveError> {
        self.require_core_collection(category)?
            .target()
            .map_err(ObjectError::InvalidTarget)
            .map_err(Into::into)
    }
}

impl<S: ClientState> Client<S> {
    pub(crate) fn transport(&self) -> &dyn Transport {
        self.transport.as_ref()
    }
    /// Creates an owned, long-lived SAP user session for stateful operations.
    ///
    /// The session is represented by `sap-contextid` over HTTP and can be
    /// inspected in transaction `SM04` while active.
    pub fn create_user_session(&self) -> UserSession<S> {
        UserSession::new(self.clone())
    }
}
