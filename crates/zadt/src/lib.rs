#![doc = include_str!("../README.md")]

mod api;
mod client;
mod compatibility;
mod error;
mod models;
mod objects;
mod operation;
mod protocol;
mod resource;
mod target;
mod uri;
mod vocabulary;

pub mod transport;

pub use api::classes::ClassRun;
pub use api::discovery::{CoreDiscoveryQuery, DiscoveryQuery};
pub use api::object::{
    LockRequest, ObjectRun, ObjectSourceQuery, ObjectSourceUpdate, UnlockRequest,
};
pub use api::packages::{PackageSettingsQuery, PackageTreeQuery};
pub use api::programs::ProgramRun;
pub use api::properties::{
    JsonObjectProperties, JsonObjectPropertiesQuery, JsonObjectPropertiesUpdate, ObjectProperties,
    ObjectPropertiesQuery, ObjectPropertiesUpdate,
};
pub use api::repository::{
    RepositoryContentOperation, RepositoryContentQuery, RepositoryContentQueryBuilder,
    RepositoryContentQueryBuilderError, RepositoryFacetsQuery, RepositoryObjectPropertiesQuery,
    RepositoryObjectPropertiesQueryBuilder, RepositoryObjectPropertiesQueryBuilderError,
};
pub use api::session::Logon;
pub use api::transports::{
    QueryTransportKind, TransportCheck, TransportCheckBuilder, TransportCheckBuilderError,
    TransportCheckLinkUpMode, TransportCheckOperation, TransportCreate, TransportCreateBuilder,
    TransportCreateBuilderError, TransportPropertiesQuery, TransportsQuery, TransportsQueryBuilder,
    TransportsQueryBuilderError,
};
pub use client::{Client, ClientState, Initial, Ready};
pub use compatibility::CompatibilityError;
#[cfg(feature = "reqwest")]
pub use error::ReqwestTransportBuildError;
pub use error::{
    CtsError, DiscoveryError, LogonError, ObjectError, OperationError, RepositoryError,
    ResponseError, TransportError,
};
pub use models::{
    AccessMode, AdtException, AdtExceptionProperty, AdvertisedObjectReference, Capabilities,
    Category, ClassProperties, ClassPropertiesVersion, ClassRunResult, ClassSourceProperties,
    Collection, DataElementDefinition, DataElementProperties, DataElementPropertiesVersion,
    IncludeProperties, IncludePropertyVersion, ObjectLock, ObjectRunResult, PackageAssignment,
    PackageAttributes, PackageInterfaceReference, PackageProperties, PackagePropertiesVersion,
    PackageReference, PackageSettings, PackageTransport, PackageTree, PackageTreeKind,
    PackageTreeNode, PackageUseAccess, ProgramProperties, ProgramPropertiesVersion,
    ProgramRunResult, RepositoryContent, RepositoryFacet, RepositoryFacetDefinition,
    RepositoryFacetValuesLink, RepositoryFacets, RepositoryObjectEntry, RepositoryObjectProperties,
    RepositoryObjectSummary, RepositoryPreselection, RepositoryPreselectionInfo,
    RepositoryProperty, RepositoryVirtualFolder, SessionInformation, SessionUri, SourceCode,
    SourceUpdateResult, SyntaxConfiguration, SyntaxLanguage, SystemInformationLink, TemplateLink,
    TransportCheckMessage, TransportCheckResult, TransportCreation, TransportCreationMessage,
    TransportKind, TransportNumber, TransportObjectKey, TransportObjectLock, TransportProject,
    TransportRequest, TransportRequests, TransportStatus, TransportTask, Workspace,
};
pub use objects::{
    Class, ClassSourceComponent, DataElement, Erased, GlobalWorkbenchType, HasSource, Include,
    InvalidWorkbenchType, ObjectRef, ObjectType, ObjectVersion, Package, Program, PropertyModel,
    ReadProperties, UpdateProperties,
};
pub use operation::{
    BatchError, BatchKey, BatchOperation, BatchResponses, Batched, Execute, IfNoneMatch, Operation,
    OperationContext, OperationKind, OperationResponse, Revalidation, Stateful, Stateless,
    UserSession,
};
pub use protocol::{AdtRequest, AdtResponse, EntityTag};
pub use resource::{
    AdtLink, AdtLinkError, AdvertisedLink, EnhancementImplementationsRef, HtmlSourceRef,
    ObjectEnhancementOptionsRef, ObjectStateRef, ObjectStructureRef, OwnedResourceRef, ParserRef,
    Relations, SourceEnhancementOptionsRef, SourceRef, SourceVersionsRef, TextElementsRef,
};
pub use transport::Transport;
#[cfg(feature = "reqwest")]
pub use transport::{ReqwestTransport, ReqwestTransportBuilder};
#[cfg(feature = "logging")]
pub use transport::{Traced, TransportExt};
pub use uri::{ADT_RESOURCE_ROOT, ADT_ROOT, AdtUri, AdtUriError};
pub use vocabulary::{CategoryId, PostAction};
