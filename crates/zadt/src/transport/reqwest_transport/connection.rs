use std::sync::{Arc, RwLock};

use http::{HeaderMap, HeaderValue, header};
use reqwest::cookie::{CookieStore, Jar};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{AdtRequest, AdtResponse, TransportError, User};

pub(super) struct HttpConnection {
    client: reqwest::Client,
    cookies: Arc<SapCookieStore>,
    destination: Url,
    username: User,
    password: SecretString,
}

impl HttpConnection {
    pub(super) fn new(
        destination: Url,
        user_context: &str,
        username: User,
        password: SecretString,
    ) -> Result<Self, reqwest::Error> {
        let cookies = Arc::new(SapCookieStore::new(&destination, user_context));
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_provider(Arc::clone(&cookies))
            .build()?;
        Ok(Self {
            client,
            cookies,
            destination,
            username,
            password,
        })
    }

    pub(super) async fn send(
        &self,
        request: &AdtRequest,
        header_overrides: &HeaderMap,
    ) -> Result<AdtResponse, TransportError> {
        let mut headers = request.headers().clone();
        headers.extend(header_overrides.clone());
        let url = request_url(
            &self.destination,
            request.target().as_str(),
            request.query(),
        )
        .map_err(TransportError::new)?;
        merge_cookie_headers(&mut headers, self.cookies.cookies(&url))
            .map_err(TransportError::new)?;
        let response = self
            .client
            .request(request.method().clone(), url)
            .headers(headers)
            .basic_auth(self.username.as_str(), Some(self.password.expose_secret()))
            .body(request.body().to_vec())
            .send()
            .await
            .map_err(TransportError::new)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .bytes()
            .await
            .map_err(TransportError::new)?
            .to_vec();
        Ok(AdtResponse::new(status, headers, body))
    }

    pub(super) fn reset_cookies(&self) {
        self.cookies.reset();
    }
}

#[derive(Debug)]
pub(super) struct SapCookieStore {
    jar: RwLock<Jar>,
    destination: Url,
    user_context: String,
}

impl SapCookieStore {
    pub(super) fn new(destination: &Url, user_context: &str) -> Self {
        Self {
            jar: RwLock::new(Self::seeded_jar(destination, user_context)),
            destination: destination.clone(),
            user_context: user_context.to_owned(),
        }
    }

    fn seeded_jar(destination: &Url, user_context: &str) -> Jar {
        let jar = Jar::default();
        jar.add_cookie_str(
            &format!("sap-usercontext={user_context}; Path=/"),
            destination,
        );
        jar
    }

    pub(super) fn reset(&self) {
        *self.jar.write().unwrap_or_else(|error| error.into_inner()) =
            Self::seeded_jar(&self.destination, &self.user_context);
    }
}

impl CookieStore for SapCookieStore {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let cookies = cookie_headers
            .filter(|header| !is_adt_context_cookie(header))
            .collect::<Vec<_>>();
        self.jar
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .set_cookies(&mut cookies.into_iter(), url);
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        self.jar
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .cookies(url)
    }
}

fn is_adt_context_cookie(header: &HeaderValue) -> bool {
    header
        .to_str()
        .ok()
        .and_then(|value| value.split_once('='))
        .is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case("sap-contextid"))
}

pub(super) fn merge_cookie_headers(
    headers: &mut HeaderMap,
    session_cookies: Option<HeaderValue>,
) -> Result<(), http::header::InvalidHeaderValue> {
    let mut cookies = Vec::new();
    if let Some(session_cookies) = session_cookies {
        cookies.extend_from_slice(session_cookies.as_bytes());
    }
    for request_cookies in headers.get_all(header::COOKIE) {
        if !cookies.is_empty() {
            cookies.extend_from_slice(b"; ");
        }
        cookies.extend_from_slice(request_cookies.as_bytes());
    }
    if !cookies.is_empty() {
        headers.insert(header::COOKIE, HeaderValue::from_bytes(&cookies)?);
    }
    Ok(())
}

pub(super) fn request_url(
    destination: &Url,
    target: &str,
    query_parameters: &[(String, String)],
) -> Result<Url, url::ParseError> {
    let mut url = destination.join(target)?;
    if !query_parameters.is_empty() {
        let mut query = url.query_pairs_mut();
        query.extend_pairs(
            query_parameters
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        );
    }
    Ok(url)
}
