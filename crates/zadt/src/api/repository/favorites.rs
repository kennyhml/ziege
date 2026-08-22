use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    AdtRequest, AdvertisedObjectReference, CategoryId, Client, GlobalWorkbenchType, ObjectError,
    ObjectRef, Operation, OperationError, OperationResponse, Ready, RepositoryError, ResponseError,
    Stateless, User, operation::CollectionTarget,
};

const CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/repository/virtualfolders",
    term: "objectFavorites",
};
const FAVORITES_NAMESPACE: &str = "http://www.sap.com/adt/ris/vf/favorites";
pub(super) const FAVORITES_MEDIA_TYPE: &str = "application/vnd.sap.adt.repository.favorites.v1+xml";
pub(super) const FAVORITES_UPDATE_MEDIA_TYPE: &str =
    "application/vnd.sap.adt.repository.favorites.modify.v1+xml";

/// Queries a users favorite objects. Because this is stored in a table
/// (vfs_fav_objects) on the backend, favorites set inside different editors
/// can be synchronized.
///
/// Objects can be assigned to different lists within the favorites. By default,
/// a list named $<sy-name> is used to store the favorited objects. Custom list
/// ids can be provided in query and update operations if needed.
///
/// Backend handler: `CL_RIS_ADT_RES_VF_FAVORITES`
#[derive(Clone, Debug, Default)]
pub struct FavoriteObjectsQuery {
    /// The list ID - by default $<sy-name> is used
    list: Option<String>,
}

impl FavoriteObjectsQuery {
    const TARGET: CollectionTarget = CollectionTarget::new(CATEGORY);

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

impl Operation<Ready> for FavoriteObjectsQuery {
    type Kind = Stateless;
    type Response = FavoriteObjectList;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let list = self.list.as_deref().unwrap_or("$");
        let target = Self::TARGET
            .collection(client)?
            .target()
            .and_then(|uri| uri.append_segments([list]))
            .map_err(ObjectError::InvalidTarget)?;

        let mut request = AdtRequest::new(Method::GET, target);
        request.set_accept(FAVORITES_MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;

        response.require_content_type(&[FAVORITES_MEDIA_TYPE])?;

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
    objects: FavoriteObjectList,
}

impl FavoriteObjectsUpdate {
    const TARGET: CollectionTarget = CollectionTarget::new(CATEGORY);

    pub fn new(list: impl Into<String>) -> Self {
        Self {
            list: list.into(),
            objects: FavoriteObjectList { objects: vec![] },
        }
    }

    pub fn add<T>(&mut self, object: &ObjectRef<T>) -> &mut Self {
        let mut entry: FavoriteObject = object.into();
        entry.operation = Some("A".into());
        entry.list = Some(self.list.clone());
        self.objects.objects.push(entry);
        self
    }

    pub fn remove<T>(&mut self, object: &ObjectRef<T>) -> &mut Self {
        let mut entry: FavoriteObject = object.into();
        entry.operation = Some("R".into());
        entry.list = Some(self.list.clone());
        self.objects.objects.push(entry);
        self
    }
}

impl Operation<Ready> for FavoriteObjectsUpdate {
    type Kind = Stateless;
    type Response = FavoriteObjectList;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let body = self.objects.serialize()?;
        let target = Self::TARGET
            .collection(client)?
            .target()
            .and_then(|uri| uri.append_segments([&self.list]))
            .map_err(ObjectError::InvalidTarget)?;

        let mut request = AdtRequest::new(Method::POST, target);
        request.set_accept(FAVORITES_MEDIA_TYPE);
        request.set_content_type(FAVORITES_UPDATE_MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;

        response.require_content_type(&[FAVORITES_MEDIA_TYPE])?;

        serde_xml_rs::from_reader(response.body())
            .map_err(RepositoryError::InvalidResponse)
            .map_err(Into::into)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "vf:favorites")]
pub struct FavoriteObjectList {
    #[serde(rename = "vf:favorite", default)]
    pub objects: Vec<FavoriteObject>,
}

impl FavoriteObjectList {
    fn serialize(&self) -> Result<String, RepositoryError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("vf", FAVORITES_NAMESPACE)
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

impl<T> From<&ObjectRef<T>> for FavoriteObject {
    fn from(value: &ObjectRef<T>) -> Self {
        Self {
            uri: value.uri().as_str().to_owned(),
            object_type: value.object_type().clone(),
            name: value.name().to_owned(),
            list: None,
            operation: None,
        }
    }
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
