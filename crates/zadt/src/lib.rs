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
pub use api::properties::{ObjectPropertiesQuery, ObjectPropertiesUpdate, RawObjectProperties};
pub use api::repository::{
    RepositoryContentOperation, RepositoryContentQuery, RepositoryContentQueryBuilder,
    RepositoryContentQueryBuilderError, RepositoryFacetsQuery, RepositoryObjectPropertiesQuery,
    RepositoryObjectPropertiesQueryBuilder, RepositoryObjectPropertiesQueryBuilderError,
};
pub use api::session::{Logon, SessionMediaVersion};
pub use api::transports::{
    QueryTransportKind, TransportCheck, TransportCheckBuilder, TransportCheckBuilderError,
    TransportCheckLinkUpMode, TransportCheckOperation, TransportCreate, TransportCreateBuilder,
    TransportCreateBuilderError, TransportPropertiesQuery, TransportsQuery, TransportsQueryBuilder,
    TransportsQueryBuilderError,
};
pub use client::{Client, ClientState, Initial, Ready};
pub use compatibility::{CompatibilityError, MediaVersionNegotiation, negotiate};
#[cfg(feature = "reqwest")]
pub use error::ReqwestTransportBuildError;
pub use error::{
    CtsError, DiscoveryError, LogonError, ObjectError, OperationError, RepositoryError,
    ResponseError, TransportError,
};
pub use models::{
    AccessMode, AdtException, AdtExceptionProperty, Capabilities, Category, ClassObjectReference,
    ClassProperties, ClassPropertiesV2, ClassPropertiesV3, ClassPropertiesV4,
    ClassPropertiesVersion, ClassRunResult, ClassSourceProperties, Collection,
    DataElementDefinition, DataElementDocumentationStatus, DataElementFieldLabel,
    DataElementProperties, DataElementPropertiesV2, DataElementPropertiesVersion,
    DataElementTypeKind, IncludeProperties, IncludePropertiesV2, IncludePropertyVersion,
    ObjectLock, ObjectPropertiesUpdateResult, ObjectRunResult, PackageAssignment,
    PackageAttributes, PackageInterfaceReference, PackageProperties, PackagePropertiesV1,
    PackagePropertiesV2, PackagePropertiesVersion, PackageReference, PackageSettings,
    PackageTransport, PackageTree, PackageTreeKind, PackageTreeNode, PackageUseAccess,
    ProgramProperties, ProgramPropertiesV2, ProgramPropertiesV3, ProgramPropertiesVersion,
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
    Class, ClassSourceComponent, DataElement, GlobalWorkbenchType, HasSource, Include,
    InvalidWorkbenchType, ObjectRef, ObjectType, ObjectVersion, Package, Program, ReadProperties,
    RepositoryObject, UpdateProperties, WritableProperties,
};
pub use operation::{
    BatchError, BatchKey, BatchOperation, BatchResponses, Batched, Execute, IfNoneMatch, Operation,
    OperationContext, OperationKind, OperationResponse, Revalidation, Stateful, Stateless,
    UserSession,
};
pub use protocol::{AdtRequest, AdtResponse, EntityTag};
pub use resource::{
    AdtLink, AdtLinkError, EnhancementImplementationsRef, HtmlSourceRef,
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
