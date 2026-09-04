use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    AdvertisedObjectReference, CategoryId, Discovery, EncodeError, EncodedOperation,
    GlobalWorkbenchType, ObjectError, ObjectRef, Operation, OperationResponse, RepositoryError,
    RequiresDiscovery, ResolveError, ResponseError, Stateless, User,
};

/// Queries a users favorite objects. Because this is stored in a table
/// (vfs_fav_objects) on the backend, favorites set inside different editors
/// can be synchronized.
///
/// Objects can be assigned to different lists within the favorites. By default,
/// a list named `$<sy-name>` is used to store the favorited objects. Custom list
/// ids can be provided in query and update operations if needed.
///
/// Backend handler: `CL_RIS_ADT_RES_VF_FAVORITES`
#[derive(Clone, Debug, Default)]
pub struct FavoriteObjectsQuery {
    /// The list ID - by default `$<sy-name>` is used
    list: Option<String>,
}

impl FavoriteObjectsQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository/virtualfolders",
        term: "objectFavorites",
    };

    pub fn new() -> Self {
        Self { list: None }
    }

    /// Constructs a query for the objects of the given username by
    /// providing a $ prefix, which is what ADT uses internally.
    pub fn username(&mut self, username: impl Into<User>) -> &mut Self {
        self.list = Some(format!("${}", username.into()));
        self
    }

    /// Constructs a query for the provided list id which may or
    /// may not exist.
    pub fn list(&mut self, list: impl Into<String>) -> &mut Self {
        self.list = Some(list.into());
        self
    }
}

impl User {
    /// Creates a query for this user's default favorites list.
    pub fn favorites(&self) -> FavoriteObjectsQuery {
        let mut query = FavoriteObjectsQuery::new();
        query.username(self);
        query
    }
}

impl Operation for FavoriteObjectsQuery {
    type Kind = Stateless;
    type Response = FavoriteObjectList;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let list = self.list.as_deref().unwrap_or("$");
        let target = resolver
            .require_collection_target(Self::CATEGORY)?
            .append_segments([list])
            .map_err(ObjectError::InvalidTarget)
            .map_err(ResolveError::from)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        request.set_accept(FavoriteObjectList::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;

        response.require_content_type(&[FavoriteObjectList::MEDIA_TYPE])?;

        serde_xml_rs::from_reader(response.body())
            .map_err(RepositoryError::InvalidResponse)
            .map_err(Into::into)
    }
}

/// Updates a users favorite object lists by either adding or removing
/// objects from the list.
///
/// The response contains the updated list as if it had been queried
/// after updating it.
///
/// Backend handler: `CL_RIS_ADT_RES_VF_FAVORITES`
pub struct FavoriteObjectsUpdate {
    /// The list ID - must be provided explicitly
    list: String,

    /// The list of objects to update or remove, marked by the operation
    objects: Vec<PendingFavoriteObject>,
}

impl FavoriteObjectsUpdate {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/repository/virtualfolders",
        term: "objectFavorites",
    };

    pub fn new(list: impl Into<String>) -> Self {
        Self {
            list: list.into(),
            objects: Vec::new(),
        }
    }

    pub fn add<T>(&mut self, object: &ObjectRef<T>) -> &mut Self {
        self.objects
            .push(PendingFavoriteObject::new(object, FavoriteOperation::Add));
        self
    }

    pub fn remove<T>(&mut self, object: &ObjectRef<T>) -> &mut Self {
        self.objects.push(PendingFavoriteObject::new(
            object,
            FavoriteOperation::Remove,
        ));
        self
    }
}

impl Operation for FavoriteObjectsUpdate {
    type Kind = Stateless;
    type Response = FavoriteObjectList;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let objects = self
            .objects
            .iter()
            .map(|object| object.resolve(resolver, &self.list))
            .collect::<Result<Vec<_>, _>>()?;
        let body = FavoriteObjectList { objects }.serialize()?;
        let target = resolver
            .require_collection_target(Self::CATEGORY)?
            .append_segments([self.list.as_str()])
            .map_err(ObjectError::InvalidTarget)
            .map_err(ResolveError::from)?;
        let mut request = EncodedOperation::new(Method::POST, target);
        request.set_accept(FavoriteObjectList::MEDIA_TYPE);
        request.set_content_type(FavoriteObjectList::UPDATE_MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;

        response.require_content_type(&[FavoriteObjectList::MEDIA_TYPE])?;

        serde_xml_rs::from_reader(response.body())
            .map_err(RepositoryError::InvalidResponse)
            .map_err(Into::into)
    }
}

#[derive(Debug)]
struct PendingFavoriteObject {
    reference: ObjectRef<()>,
    operation: FavoriteOperation,
}

impl PendingFavoriteObject {
    fn new<T>(object: &ObjectRef<T>, operation: FavoriteOperation) -> Self {
        Self {
            reference: object.erase(),
            operation,
        }
    }

    fn resolve(&self, resolver: &Discovery, list: &str) -> Result<FavoriteObject, ResolveError> {
        Ok(FavoriteObject {
            uri: resolver.resolve_object_uri(&self.reference)?.to_string(),
            object_type: self.reference.object_type().clone(),
            name: self.reference.name().to_owned(),
            list: Some(list.to_owned()),
            operation: Some(self.operation.as_str().to_owned()),
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum FavoriteOperation {
    Add,
    Remove,
}

impl FavoriteOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Add => "A",
            Self::Remove => "R",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "vf:favorites")]
pub struct FavoriteObjectList {
    #[serde(rename = "vf:favorite", default)]
    pub objects: Vec<FavoriteObject>,
}

impl FavoriteObjectList {
    pub(super) const MEDIA_TYPE: &str = "application/vnd.sap.adt.repository.favorites.v1+xml";
    pub(super) const UPDATE_MEDIA_TYPE: &str =
        "application/vnd.sap.adt.repository.favorites.modify.v1+xml";
    const NAMESPACE: &str = "http://www.sap.com/adt/ris/vf/favorites";

    fn serialize(&self) -> Result<String, RepositoryError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("vf", Self::NAMESPACE)
            .namespace("adtcore", "http://www.sap.com/adt/core")
            .to_string(self)
            .map_err(RepositoryError::InvalidRequest)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "vf:favorite")]
pub struct FavoriteObject {
    #[serde(rename = "@adtcore:uri")]
    pub uri: String,

    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    #[serde(rename = "@adtcore:name")]
    pub name: String,

    #[serde(rename = "@listId", skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,

    #[serde(rename = "@operation", skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
}

impl From<FavoriteObject> for AdvertisedObjectReference {
    fn from(value: FavoriteObject) -> Self {
        AdvertisedObjectReference {
            uri: Some(value.uri),
            object_type: Some(value.object_type),
            name: Some(value.name),
            ..Default::default()
        }
    }
}
