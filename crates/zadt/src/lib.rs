#![doc = include_str!("../README.md")]

mod api;
mod client;
mod compatibility;
mod error;
mod objects;
mod operation;
mod protocol;
mod resource;
mod uri;
mod user;
mod user_session;

pub mod transport;

pub use api::activation::{
    ActivationRun, ActivationRunMessage, ActivationRunMessageText, ActivationRunMessages,
    ActivationRunMode, ActivationRunProperties, CheckRunArtifact, CheckRunMessage,
    CheckRunMessageList, CheckRunObject, CheckRunReport, CheckRunReporter, CheckRunReportersQuery,
    CheckRunReports, CheckRunT100Key, InactiveCtsObject, InactiveCtsObjectEntry,
    InactiveCtsObjectTransport, InactiveCtsObjects, InactiveCtsObjectsQuery, InactiveObjectsQuery,
    ObjectCheckRun, SupportedCheckReporter, SupportedCheckReporters,
};
pub use api::classes::{ClassRun, ClassRunResult};
pub use api::creation::CreateObjectRequest;
pub use api::discovery::{
    Capabilities, Category, CategoryId, Collection, CoreDiscoveryQuery, DiscoveryQuery,
    TemplateLink, Workspace,
};
pub use api::locking::{AccessMode, LockRequest, ObjectLock, UnlockRequest};
pub use api::objectstructure::{ObjectStructure, ObjectStructureElement, ObjectStructureQuery};
pub use api::packages::{
    PackageInterfaceReference, PackageReference, PackageSettings, PackageSettingsQuery,
    PackageTree, PackageTreeKind, PackageTreeNode, PackageTreeQuery,
};
pub use api::programs::{ProgramRun, ProgramRunResult};
pub use api::properties::{ObjectPropertiesQuery, ObjectPropertiesUpdate};
pub use api::repository::{
    AssignedTransport, AssignedTransportRequests, AssignedTransportsQuery, FavoriteObject,
    FavoriteObjectList, FavoriteObjectsQuery, FavoriteObjectsUpdate, RepositoryContent,
    RepositoryContentOperation, RepositoryContentQuery, RepositoryContentQueryBuilder,
    RepositoryContentQueryBuilderError, RepositoryFacet, RepositoryFacetDefinition,
    RepositoryFacetValuesLink, RepositoryFacets, RepositoryFacetsQuery, RepositoryObjectEntry,
    RepositoryObjectProperties, RepositoryObjectPropertiesQuery, RepositoryObjectSummary,
    RepositoryPreselection, RepositoryPreselectionInfo, RepositoryProperty,
    RepositoryVirtualFolder,
};
pub use api::run::{ObjectRun, ObjectRunResult};
pub use api::session::{Logon, SessionInformation, SessionUri, SystemInformationLink};
pub use api::source::{ObjectSourceQuery, ObjectSourceUpdate, SourceCode, SourceUpdateResult};
pub use api::transports::{
    QueryTransportKind, TransportCheck, TransportCheckBuilder, TransportCheckBuilderError,
    TransportCheckLinkUpMode, TransportCheckMessage, TransportCheckOperation, TransportCheckResult,
    TransportCreate, TransportCreateBuilder, TransportCreateBuilderError, TransportCreateVersion,
    TransportCreation, TransportCreationMessage, TransportKind, TransportNumber,
    TransportObjectKey, TransportObjectLock, TransportProject, TransportPropertiesQuery,
    TransportRequest, TransportRequests, TransportStatus, TransportTask, TransportsQuery,
    TransportsQueryBuilder, TransportsQueryBuilderError,
};
pub use api::users::{UserDetailsQuery, Users, UsersQuery};
pub use client::{Client, ClientState, Initial, Ready};
pub use compatibility::CompatibilityError;
#[cfg(feature = "reqwest")]
pub use error::ReqwestTransportBuildError;
pub use error::{
    AdtException, AdtExceptionProperty, CtsError, DiscoveryError, EncodeError, LogonError,
    ObjectError, OperationError, RepositoryError, ResolveError, ResponseError, TransportError,
    UserError,
};
pub use objects::{
    AbapLanguageVersion, AccessControl, AccessControlCreateProperties,
    AccessControlCreatePropertiesBuilder, AccessControlCreatePropertiesBuilderError,
    AccessControlProperties, AccessControlPropertiesVersion, AdvertisedObjectReference,
    AnnotationDefinition, AnnotationDefinitionCreateProperties,
    AnnotationDefinitionCreatePropertiesBuilder, AnnotationDefinitionCreatePropertiesBuilderError,
    AnnotationDefinitionProperties, AnnotationDefinitionPropertiesVersion, AnyObject, Class,
    ClassCategory, ClassCreateProperties, ClassCreatePropertiesBuilder,
    ClassCreatePropertiesBuilderError, ClassProperties, ClassPropertiesVersion,
    ClassSourceComponent, ClassSourceProperties, ClassTemplate, ClassTemplateProperty, Create,
    CreationPropertyModel, DataDefinition, DataDefinitionCreateProperties,
    DataDefinitionCreatePropertiesBuilder, DataDefinitionCreatePropertiesBuilderError,
    DataDefinitionProperties, DataDefinitionPropertiesVersion, DataElement, DataElementDefinition,
    DataElementProperties, DataElementPropertiesVersion, Domain, DomainContent,
    DomainCreateProperties, DomainCreatePropertiesBuilder, DomainCreatePropertiesBuilderError,
    DomainFixedValue, DomainFixedValues, DomainOutputInformation, DomainProperties,
    DomainPropertiesVersion, DomainTypeInformation, DomainValueInformation, FunctionGroup,
    FunctionGroupInclude, FunctionGroupIncludeProperties, FunctionGroupIncludePropertiesVersion,
    FunctionGroupProperties, FunctionGroupPropertiesVersion, FunctionModule,
    FunctionModuleProperties, FunctionModulePropertiesVersion, GlobalWorkbenchType, Include,
    IncludeProperties, IncludePropertyVersion, Interface, InterfaceCreateProperties,
    InterfaceCreatePropertiesBuilder, InterfaceCreatePropertiesBuilderError, InterfaceProperties,
    InterfacePropertiesVersion, InvalidWorkbenchType, MetadataExtension,
    MetadataExtensionCreateProperties, MetadataExtensionCreatePropertiesBuilder,
    MetadataExtensionCreatePropertiesBuilderError, MetadataExtensionProperties,
    MetadataExtensionPropertiesVersion, Object, ObjectRef, ObjectReferences, ObjectType,
    ObjectVersion, Package, PackageAssignment, PackageAttributes, PackageProperties,
    PackagePropertiesVersion, PackageTransport, PackageUseAccess, PrimaryObjectType, Program,
    ProgramProperties, ProgramPropertiesVersion, PropertyModel, ServiceDefinition,
    ServiceDefinitionCreateProperties, ServiceDefinitionCreatePropertiesBuilder,
    ServiceDefinitionCreatePropertiesBuilderError, ServiceDefinitionProperties,
    ServiceDefinitionPropertiesVersion, Source, SourceComponents, Structure, SubObjects,
    SyntaxConfiguration, SyntaxLanguage,
};
pub use operation::{
    Advertised, AdvertisedCollection, AdvertisedTarget, AdvertisedTemplate, BatchError, BatchKey,
    BatchOperation, BatchResponses, Batched, DiscoveryDocument, EncodedOperation, Execute,
    IfNoneMatch, Operation, OperationContext, OperationKind, OperationResponse, OperationTarget,
    Owned, Resolve, ResolvedOperation, Revalidation, Stateful, Stateless,
};
pub use protocol::{AdtRequest, AdtResponse, EntityTag, PostAction};
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
pub use user::User;
pub use user_session::{UserSession, UserSessionId};
