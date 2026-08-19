use http::Method;
use serde::Deserialize;

use super::common::ensure_ok;
use crate::{
    AdtRequest, CategoryId, Client, GlobalWorkbenchType, ObjectError, Operation, OperationError,
    OperationResponse, Ready, RepositoryError, ResponseError, Stateless,
    compatibility::media_types_match, target::CollectionTarget, vocabulary::media_type,
};

const CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/repository/virtualfolders",
    term: "objectFavorites",
};

/// Queries a users favorite objects. Because this is stored in a table
/// (vfs_fav_objects) on the backend, favorites set inside different editors
/// can be synchronized.
///
/// Objects can be assigned to different lists within the favorites. By default,
/// a list named $<sy-name> is used to store the favorited objects. Custom list
/// ids can be provided in query and update operations if needed.
///
/// Backend handler: `CL_RIS_ADT_RES_VF_FAVORITES`
pub struct FavoriteObjectsQuery {
    /// The list ID - by default $<sy-name> is used
    list: Option<String>,
}

impl FavoriteObjectsQuery {
    const TARGET: CollectionTarget = CollectionTarget::new(CATEGORY);

    pub fn new() -> Self {
        Self { list: None }
    }

    pub fn username(&mut self, username: impl Into<String>) -> &mut Self {
        self.list = Some(format!("${}", username.into()));
        self
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
        request.set_accept(media_type::REPOSITORY_FAVORITES_COMPLETE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        ensure_ok(response.response())?;

        let content_type = response.content_type(CATEGORY)?;
        if !media_types_match(media_type::REPOSITORY_FAVORITES_COMPLETE, content_type) {
            return Err(ResponseError::UnsupportedContentType {
                category: CATEGORY,
                content_type: content_type.to_owned(),
                supported: vec![media_type::REPOSITORY_FAVORITES_COMPLETE.to_owned()],
            });
        }

        serde_xml_rs::from_reader(response.body())
            .map_err(RepositoryError::InvalidResponse)
            .map_err(Into::into)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename = "vf:favorites")]
pub struct FavoriteObjectList {
    #[serde(rename = "vf:favorite", default)]
    pub objects: Vec<FavoriteObject>,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "vf:favorite")]
pub struct FavoriteObject {
    #[serde(rename = "@adtcore:uri")]
    pub uri: String,

    #[serde(rename = "@adtcore:type")]
    pub object_type: GlobalWorkbenchType,

    #[serde(rename = "@adtcore:name")]
    pub name: String,

    #[serde(rename = "@listId")]
    pub list: String,
}
