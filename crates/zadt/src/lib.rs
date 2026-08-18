#![doc = include_str!("../README.md")]

mod api;
mod client;
mod compatibility;
mod error;
mod objects;
mod operation;
mod protocol;
mod resource;
mod target;
mod uri;
mod vocabulary;

pub mod transport;

pub use api::activation::{
    ActivationRun, ActivationRunMessage, ActivationRunMessageText, ActivationRunMessages,
    ActivationRunMode, ActivationRunProperties, InactiveCtsObject, InactiveCtsObjectEntry,
    InactiveCtsObjectTransport, InactiveCtsObjects, InactiveCtsObjectsQuery, InactiveObjectsQuery,
};
pub use api::classes::{ClassRun, ClassRunResult};
pub use api::creation::CreateObjectRequest;
pub use api::discovery::{
    Capabilities, Category, Collection, CoreDiscoveryQuery, DiscoveryQuery, TemplateLink, Workspace,
};
pub use api::locking::{AccessMode, LockRequest, ObjectLock, UnlockRequest};
pub use api::packages::{
    PackageInterfaceReference, PackageReference, PackageSettings, PackageSettingsQuery,
    PackageTree, PackageTreeKind, PackageTreeNode, PackageTreeQuery,
};
pub use api::programs::{ProgramRun, ProgramRunResult};
pub use api::properties::{ObjectPropertiesQuery, ObjectPropertiesUpdate};
pub use api::repository::{
    RepositoryContent, RepositoryContentOperation, RepositoryContentQuery,
    RepositoryContentQueryBuilder, RepositoryContentQueryBuilderError, RepositoryFacet,
    RepositoryFacetDefinition, RepositoryFacetValuesLink, RepositoryFacets, RepositoryFacetsQuery,
    RepositoryObjectEntry, RepositoryObjectProperties, RepositoryObjectPropertiesQuery,
    RepositoryObjectPropertiesQueryBuilder, RepositoryObjectPropertiesQueryBuilderError,
    RepositoryObjectSummary, RepositoryPreselection, RepositoryPreselectionInfo,
    RepositoryProperty, RepositoryVirtualFolder,
};
pub use api::run::{ObjectRun, ObjectRunResult};
pub use api::session::{Logon, SessionInformation, SessionUri, SystemInformationLink};
pub use api::source::{ObjectSourceQuery, ObjectSourceUpdate, SourceCode, SourceUpdateResult};
pub use api::transports::{
    QueryTransportKind, TransportCheck, TransportCheckBuilder, TransportCheckBuilderError,
    TransportCheckLinkUpMode, TransportCheckMessage, TransportCheckOperation, TransportCheckResult,
    TransportCreate, TransportCreateBuilder, TransportCreateBuilderError, TransportCreation,
    TransportCreationMessage, TransportKind, TransportNumber, TransportObjectKey,
    TransportObjectLock, TransportProject, TransportPropertiesQuery, TransportRequest,
    TransportRequests, TransportStatus, TransportTask, TransportsQuery, TransportsQueryBuilder,
    TransportsQueryBuilderError,
};
pub use client::{Client, ClientState, Initial, Ready};
pub use compatibility::CompatibilityError;
#[cfg(feature = "reqwest")]
pub use error::ReqwestTransportBuildError;
pub use error::{
    AdtException, AdtExceptionProperty, CtsError, DiscoveryError, LogonError, ObjectError,
    OperationError, RepositoryError, ResponseError, TransportError,
};
pub use objects::{
    AbapLanguageVersion, AdvertisedObjectReference, Class, ClassCategory, ClassCreateProperties,
    ClassCreatePropertiesBuilder, ClassCreatePropertiesBuilderError, ClassProperties,
    ClassPropertiesVersion, ClassSourceComponent, ClassSourceProperties, ClassTemplate,
    ClassTemplateProperty, Create, CreationPropertyModel, DataElement, DataElementDefinition,
    DataElementProperties, DataElementPropertiesVersion, GlobalWorkbenchType, Include,
    IncludeProperties, IncludePropertyVersion, InvalidWorkbenchType, Object, ObjectRef,
    ObjectReferences, ObjectState, ObjectType, ObjectVersion, Package, PackageAssignment,
    PackageAttributes, PackageProperties, PackagePropertiesVersion, PackageTransport,
    PackageUseAccess, Program, ProgramProperties, ProgramPropertiesVersion, PropertyModel, Source,
    SourceComponents, SyntaxConfiguration, SyntaxLanguage, UpdateProperties,
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
