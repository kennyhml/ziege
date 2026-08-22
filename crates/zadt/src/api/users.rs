use std::collections::HashMap;

use http::{Method, StatusCode};
use serde::Deserialize;
use stduritemplate::Value;

use crate::{
    AdtRequest, CategoryId, Client, Operation, OperationError, OperationResponse, Ready,
    ResponseError, Stateless, User, UserError,
    operation::{CollectionTarget, TemplateTarget},
};

const USERS_CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/system/users",
    term: "users",
};
const USERS_MEDIA_TYPE: &str = "application/atom+xml;type=feed";
const USER_RELATION: &str = "self";

/// Queries users visible in the SAP system user directory.
///
/// Search patterns use backend wildcard syntax, for example `*DEV*`. If no
/// pattern is supplied, the backend returns its default user selection.
#[derive(Clone, Debug, Default)]
pub struct UsersQuery {
    query_string: Option<String>,
    max_count: Option<usize>,
}

impl UsersQuery {
    const TARGET: CollectionTarget = CollectionTarget::new(USERS_CATEGORY);

    /// Creates a query using the backend's default selection and result limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the backend search pattern.
    pub fn query(&mut self, query: impl Into<String>) -> &mut Self {
        self.query_string = Some(query.into());
        self
    }

    /// Limits the number of returned users.
    pub fn max_count(&mut self, max_count: usize) -> &mut Self {
        self.max_count = Some(max_count);
        self
    }
}

impl Operation<Ready> for UsersQuery {
    type Response = Users;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let mut request = Self::TARGET.request(client, Method::GET)?;
        if let Some(query_string) = &self.query_string {
            request.push_query("querystring", query_string);
        }
        if let Some(max_count) = self.max_count {
            request.push_query("maxcount", max_count.to_string());
        }
        request.set_accept(USERS_MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[USERS_MEDIA_TYPE])?;
        Users::parse(response.body()).map_err(Into::into)
    }
}

/// Loads one user from the SAP system user directory.
#[derive(Clone, Debug)]
pub struct UserDetailsQuery {
    user: User,
}

impl UserDetailsQuery {
    const TARGET: TemplateTarget = TemplateTarget::new(USERS_CATEGORY, USER_RELATION);

    fn new(user: User) -> Self {
        Self { user }
    }
}

impl Operation<Ready> for UserDetailsQuery {
    type Response = Option<User>;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let variables = HashMap::from([(
            "username".to_owned(),
            Value::String(self.user.as_str().to_owned()),
        )]);
        let (target, query) = Self::TARGET.template(client)?.expand(&variables)?;
        let mut request = AdtRequest::new(Method::GET, target);
        for (name, value) in query {
            request.push_query(name, value);
        }
        request.set_accept(USERS_MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[USERS_MEDIA_TYPE])?;
        let mut users = Users::parse(response.body())?.users;
        if users.len() > 1 {
            return Err(UserError::MultipleUsers {
                user: self.user.as_str().to_owned(),
            }
            .into());
        }
        let Some(user) = users.pop() else {
            return Ok(None);
        };
        if user != self.user {
            return Err(UserError::UnexpectedUser {
                expected: self.user.as_str().to_owned(),
                actual: user.as_str().to_owned(),
            }
            .into());
        }
        Ok(Some(user))
    }
}

impl User {
    /// Creates a query that loads this user's directory details.
    pub fn details(&self) -> UserDetailsQuery {
        UserDetailsQuery::new(self.clone())
    }
}

impl Client<Ready> {
    /// Creates a query for users visible in this SAP system.
    pub fn users(&self) -> UsersQuery {
        UsersQuery::new()
    }
}

/// Users returned by the SAP system user directory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Users {
    pub users: Vec<User>,
}

impl Users {
    fn parse(body: &[u8]) -> Result<Self, UserError> {
        let feed: RawUsersFeed = serde_xml_rs::from_reader(body)?;
        Ok(Self {
            users: feed
                .entries
                .into_iter()
                .map(|entry| User::with_display_name(entry.name, entry.display_name))
                .collect(),
        })
    }

    pub fn len(&self) -> usize {
        self.users.len()
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }
}

#[derive(Deserialize)]
#[serde(rename = "atom:feed")]
struct RawUsersFeed {
    #[serde(rename = "atom:entry", default)]
    entries: Vec<RawUserEntry>,
}

#[derive(Deserialize)]
struct RawUserEntry {
    #[serde(rename = "atom:id")]
    name: String,

    #[serde(rename = "atom:title", default)]
    display_name: String,
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use http::{HeaderMap, header};

    use super::*;
    use crate::{AdtResponse, AdtUri, Transport, TransportError};

    const DISCOVERY_XML: &[u8] = br#"
        <app:service xmlns:app="http://www.w3.org/2007/app"
                xmlns:atom="http://www.w3.org/2005/Atom">
            <app:workspace>
                <atom:title>System</atom:title>
                <app:collection href="/sap/bc/adt/system/users">
                    <atom:category term="users"
                        scheme="http://www.sap.com/adt/categories/system/users" />
                    <adtcomp:templateLinks xmlns:adtcomp="http://www.sap.com/adt/compatibility">
                        <adtcomp:templateLink rel="self"
                            template="/sap/bc/adt/system/users/{username}" />
                    </adtcomp:templateLinks>
                </app:collection>
            </app:workspace>
        </app:service>
    "#;
    const USERS_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
        <atom:feed xmlns:atom="http://www.w3.org/2005/Atom">
            <atom:title>Users</atom:title>
            <atom:entry>
                <atom:id>DEVELOPER</atom:id>
                <atom:title>John Doe</atom:title>
            </atom:entry>
            <atom:entry>
                <atom:id>DDIC</atom:id>
                <atom:title>DDIC</atom:title>
            </atom:entry>
        </atom:feed>
    "#;

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn ready_client() -> Client<Ready> {
        let capabilities = crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap();
        Client::new(UnusedTransport).with_capabilities(
            capabilities,
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
        )
    }

    #[test]
    fn client_user_query_uses_the_advertised_collection_and_filters() {
        let client = ready_client();
        let mut query = client.users();
        query.query("*DEV*").max_count(2);

        let request = query.request(&client).unwrap();

        assert_eq!(request.target().as_str(), "/sap/bc/adt/system/users");
        assert_eq!(
            request.query(),
            [
                ("querystring".to_owned(), "*DEV*".to_owned()),
                ("maxcount".to_owned(), "2".to_owned()),
            ]
        );
        assert_eq!(
            request.headers().get(header::ACCEPT).unwrap(),
            USERS_MEDIA_TYPE
        );
    }

    #[test]
    fn user_feed_decodes_identity_and_display_name() {
        let query = UsersQuery::new();
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, USERS_MEDIA_TYPE.parse().unwrap());
        let response = AdtResponse::new(StatusCode::OK, headers, USERS_XML.to_vec());
        let users = query
            .decode(OperationResponse::new(
                response,
                AdtUri::parse("/sap/bc/adt/system/users").unwrap(),
            ))
            .unwrap();

        assert_eq!(users.len(), 2);
        assert_eq!(users.users[0].as_str(), "DEVELOPER");
        assert_eq!(users.users[0].display_name(), Some("John Doe"));
        assert_eq!(users.users[1].as_str(), "DDIC");
        assert_eq!(users.users[1].display_name(), Some("DDIC"));
    }

    #[test]
    fn user_details_uses_the_advertised_self_template_and_decodes_one_user() {
        let client = ready_client();
        let query = User::new("DEVELOPER").details();
        let request = query.request(&client).unwrap();
        assert_eq!(
            request.target().as_str(),
            "/sap/bc/adt/system/users/DEVELOPER"
        );

        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, USERS_MEDIA_TYPE.parse().unwrap());
        let response = AdtResponse::new(
            StatusCode::OK,
            headers,
            br#"<atom:feed xmlns:atom="http://www.w3.org/2005/Atom">
                <atom:entry>
                    <atom:id>DEVELOPER</atom:id>
                    <atom:title>John Doe</atom:title>
                </atom:entry>
            </atom:feed>"#
                .to_vec(),
        );
        let user = query
            .decode(OperationResponse::new(response, request.target().clone()))
            .unwrap()
            .unwrap();

        assert_eq!(user.as_str(), "DEVELOPER");
        assert_eq!(user.display_name(), Some("John Doe"));
    }
}
