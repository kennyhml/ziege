use async_trait::async_trait;
use derive_builder::Builder;
use secrecy::SecretString;
use url::Url;

use crate::{AdtRequest, AdtResponse, ReqwestTransportBuildError, Transport, TransportError, User};

use self::{connection::HttpConnection, security::HttpSecuritySession};

mod connection;
mod security;

/// An ADT transport backed by `reqwest`.
///
/// Each transport owns an RFC-aware cookie store seeded with the configured
/// SAP client and language. Cookies returned by the SAP destination are
/// retained according to their domain, path, security, and expiration rules.
///
/// ADT `sap-contextid` cookies are excluded because they belong to individual
/// [`UserSession`](crate::UserSession) values rather than the transport-wide
/// security session.
///
/// Before the first mutating request, the transport fetches a CSRF token from
/// core discovery and reuses it for subsequent requests in the same security
/// session. A definitive `403` CSRF rejection refreshes the token and replays
/// the request once. A `401` resets the HTTP security session, performs one
/// preflight logon, and replays the rejected request once.
///
/// Transport failures and server errors are never replayed because SAP may
/// already have applied a mutating request when those failures are observed.
pub struct ReqwestTransport {
    connection: HttpConnection,
    security: HttpSecuritySession,
}

impl ReqwestTransport {
    pub fn builder() -> ReqwestTransportBuilder {
        ReqwestTransportBuilder::default()
    }
}

#[doc(hidden)]
#[derive(Builder)]
#[builder(
    name = "ReqwestTransportBuilder",
    pattern = "owned",
    setter(into),
    build_fn(private, name = "build_config", error = "ReqwestTransportBuildError")
)]
pub struct ReqwestTransportConfig {
    destination: String,
    sap_client: String,
    language: String,

    #[builder(setter(custom))]
    username: User,

    #[builder(setter(custom))]
    password: SecretString,
}

impl ReqwestTransportBuilder {
    pub fn basic_auth(mut self, username: impl Into<User>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(SecretString::from(password.into()));
        self
    }

    pub fn build(self) -> Result<ReqwestTransport, ReqwestTransportBuildError> {
        let config = self.build_config()?;
        let mut destination = Url::parse(&config.destination)?;
        if !matches!(destination.scheme(), "http" | "https") {
            return Err(ReqwestTransportBuildError::UnsupportedScheme);
        }
        if !destination.username().is_empty()
            || destination.password().is_some()
            || destination.query().is_some()
            || destination.fragment().is_some()
        {
            return Err(ReqwestTransportBuildError::InvalidDestinationComponents);
        }
        destination.set_path("/");

        let user_context = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("sap-client", &config.sap_client)
            .append_pair("sap-language", &config.language)
            .finish();
        Ok(ReqwestTransport {
            connection: HttpConnection::new(
                destination,
                &user_context,
                config.username,
                config.password,
            )?,
            security: HttpSecuritySession::default(),
        })
    }
}

#[async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, request: AdtRequest) -> Result<AdtResponse, TransportError> {
        let mut retried_csrf = false;
        let mut retried_auth = false;

        loop {
            // Sometimes the security component can infer that action must be taken
            // prior to sending the request, for instance when a server defined timeout
            // has been reached since the last request, or when a CSRF token is needed
            let prepared = self.security.prepare(&self.connection, &request).await?;

            let res = self.connection.send(&request, prepared.headers()).await?;
            self.security.observe(&prepared, &res).await;

            // Handling of security failures we could not anticipate. No I/O takes
            // place here, they are just marked for retry on the next prepare.
            if !retried_auth && self.security.invalidate_unauthorized(&prepared, &res).await {
                retried_auth = true;
                continue;
            }
            if !retried_csrf && self.security.invalidate_csrf(&prepared, &res).await {
                retried_csrf = true;
                continue;
            }

            return Ok(res);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use super::{
        connection::{SapCookieStore, merge_cookie_headers, request_url},
        security::{CSRF_FETCH, CSRF_TOKEN_HEADER},
    };
    use crate::{
        AdtUri,
        api::session::{
            HTTP_SESSIONS_PATH, PREFLIGHT_LOGON_PURPOSE, SECURITY_SESSION_HEADER,
            SESSION_MEDIA_TYPE,
        },
        protocol::CORE_DISCOVERY_PATH,
    };
    use http::{HeaderMap, HeaderValue, Method, StatusCode, header};
    use httpmock::prelude::*;
    use reqwest::cookie::CookieStore;

    const SESSION_XML: &str = include_str!("../../tests/fixtures/http-session-v3.xml");

    fn test_transport(server: &MockServer) -> ReqwestTransport {
        ReqwestTransport::builder()
            .destination(server.base_url())
            .sap_client("001")
            .language("EN")
            .basic_auth("USER", "PASSWORD")
            .build()
            .unwrap()
    }

    #[test]
    fn builder_reports_missing_required_fields() {
        let Err(error) = ReqwestTransport::builder().build() else {
            panic!("incomplete transport builder succeeded");
        };

        assert!(matches!(
            error,
            ReqwestTransportBuildError::MissingField("destination")
        ));
    }

    #[test]
    fn request_without_query_has_no_empty_query_delimiter() {
        let destination = Url::parse("https://sap.example.test/").unwrap();
        let request = AdtRequest::new(
            Method::GET,
            AdtUri::parse("/sap/bc/adt/core/discovery").unwrap(),
        );

        let url = request_url(&destination, request.target().as_str(), request.query()).unwrap();

        assert_eq!(url.query(), None);
        assert_eq!(
            url.as_str(),
            "https://sap.example.test/sap/bc/adt/core/discovery"
        );
    }

    #[test]
    fn cookie_store_keeps_security_session_but_excludes_adt_context() {
        let destination = Url::parse("https://sap.example.test/").unwrap();
        let store = SapCookieStore::new(&destination, "sap-client=001&sap-language=EN");
        let session = HeaderValue::from_static("SAP_SESSIONID_A4H_001=session; Path=/");
        let context = HeaderValue::from_static("sap-contextid=context; Path=/sap/bc/adt");

        store.set_cookies(&mut [&session, &context].into_iter(), &destination);

        let cookies = store.cookies(&destination).unwrap();
        let cookies = cookies.to_str().unwrap();
        assert!(cookies.contains("sap-usercontext=sap-client=001&sap-language=EN"));
        assert!(cookies.contains("SAP_SESSIONID_A4H_001=session"));
        assert!(!cookies.contains("sap-contextid"));
    }

    #[test]
    fn cookie_store_reset_drops_security_session_but_keeps_user_context() {
        let destination = Url::parse("https://sap.example.test/").unwrap();
        let store = SapCookieStore::new(&destination, "sap-client=001&sap-language=EN");
        let session = HeaderValue::from_static("SAP_SESSIONID_A4H_001=stale; Path=/");
        store.set_cookies(&mut [&session].into_iter(), &destination);

        store.reset();

        let cookies = store.cookies(&destination).unwrap();
        let cookies = cookies.to_str().unwrap();
        assert_eq!(cookies, "sap-usercontext=sap-client=001&sap-language=EN");
    }

    #[test]
    fn merges_session_and_request_specific_cookies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("sap-contextid=context"),
        );

        merge_cookie_headers(
            &mut headers,
            Some(HeaderValue::from_static(
                "sap-usercontext=sap-client=001; SAP_SESSIONID_A4H_001=session",
            )),
        )
        .unwrap();

        assert_eq!(
            headers.get(header::COOKIE).unwrap(),
            "sap-usercontext=sap-client=001; SAP_SESSIONID_A4H_001=session; sap-contextid=context"
        );
    }

    #[tokio::test]
    async fn refreshes_a_rejected_csrf_token_once() {
        let server = MockServer::start_async().await;
        let initial_fetch = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(CORE_DISCOVERY_PATH)
                    .header(CSRF_TOKEN_HEADER, CSRF_FETCH)
                    .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
                then.status(200).header(CSRF_TOKEN_HEADER, "CSRF-OLD");
            })
            .await;
        let rejected = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/sap/bc/adt/test/write")
                    .header(CSRF_TOKEN_HEADER, "CSRF-OLD");
                then.status(403)
                    .header(CSRF_TOKEN_HEADER, "Required")
                    .header("set-cookie", "csrf-stage=refresh; Path=/");
            })
            .await;
        let refreshed_fetch = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(CORE_DISCOVERY_PATH)
                    .header(CSRF_TOKEN_HEADER, CSRF_FETCH)
                    .cookie("csrf-stage", "refresh");
                then.status(200).header(CSRF_TOKEN_HEADER, "CSRF-NEW");
            })
            .await;
        let accepted = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/sap/bc/adt/test/write")
                    .header(CSRF_TOKEN_HEADER, "CSRF-NEW");
                then.status(204);
            })
            .await;
        let transport = test_transport(&server);
        let mut request = AdtRequest::new(
            Method::POST,
            AdtUri::parse("/sap/bc/adt/test/write").unwrap(),
        );
        request.set_body(b"payload".to_vec());

        let response = transport.send(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        initial_fetch.assert_hits_async(1).await;
        rejected.assert_hits_async(1).await;
        refreshed_fetch.assert_hits_async(1).await;
        accepted.assert_hits_async(1).await;
    }

    #[tokio::test]
    async fn transport_owned_csrf_overrides_a_request_header() {
        let server = MockServer::start_async().await;
        let fetch = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(CORE_DISCOVERY_PATH)
                    .header(CSRF_TOKEN_HEADER, CSRF_FETCH);
                then.status(200).header(CSRF_TOKEN_HEADER, "CSRF-MANAGED");
            })
            .await;
        let write = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/sap/bc/adt/test/write")
                    .header(CSRF_TOKEN_HEADER, "CSRF-MANAGED");
                then.status(204);
            })
            .await;
        let transport = test_transport(&server);
        let mut request = AdtRequest::new(
            Method::POST,
            AdtUri::parse("/sap/bc/adt/test/write").unwrap(),
        );
        request.headers_mut().insert(
            CSRF_TOKEN_HEADER,
            HeaderValue::from_static("CALLER-SUPPLIED"),
        );

        let response = transport.send(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        fetch.assert_hits_async(1).await;
        write.assert_hits_async(1).await;
    }

    #[tokio::test]
    async fn security_session_request_is_not_reconnected_recursively() {
        let server = MockServer::start_async().await;
        let logon = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(HTTP_SESSIONS_PATH)
                    .header("x-sap-security-session", "create");
                then.status(401);
            })
            .await;
        let transport = test_transport(&server);
        let mut request = AdtRequest::new(Method::GET, AdtUri::parse(HTTP_SESSIONS_PATH).unwrap());
        request
            .headers_mut()
            .insert(SECURITY_SESSION_HEADER, HeaderValue::from_static("create"));

        let response = transport.send(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        logon.assert_hits_async(1).await;
    }

    #[tokio::test]
    async fn reconnects_and_retries_once_after_unauthorized() {
        let server = MockServer::start_async().await;
        let unauthorized = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/sap/bc/adt/test/read")
                    .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
                then.status(401)
                    .header("set-cookie", "SAP_SESSIONID_A4H_001=stale; Path=/");
            })
            .await;
        let relogon = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(HTTP_SESSIONS_PATH)
                    .header("sap-adt-purpose", PREFLIGHT_LOGON_PURPOSE)
                    .header("x-sap-security-session", "create")
                    .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
                then.status(200)
                    .header("content-type", SESSION_MEDIA_TYPE)
                    .header("set-cookie", "SAP_SESSIONID_A4H_001=fresh; Path=/")
                    .body(SESSION_XML);
            })
            .await;
        let accepted = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/sap/bc/adt/test/read")
                    .cookie("SAP_SESSIONID_A4H_001", "fresh");
                then.status(200).body("ok");
            })
            .await;
        let transport = test_transport(&server);
        let request = AdtRequest::new(Method::GET, AdtUri::parse("/sap/bc/adt/test/read").unwrap());

        let response = transport.send(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), b"ok");
        unauthorized.assert_hits_async(1).await;
        relogon.assert_hits_async(1).await;
        accepted.assert_hits_async(1).await;
    }

    #[tokio::test]
    async fn does_not_reconnect_recursively() {
        let server = MockServer::start_async().await;
        let initial = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/sap/bc/adt/test/read")
                    .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
                then.status(401);
            })
            .await;
        let relogon = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(HTTP_SESSIONS_PATH)
                    .header("sap-adt-purpose", PREFLIGHT_LOGON_PURPOSE);
                then.status(200)
                    .header("content-type", SESSION_MEDIA_TYPE)
                    .header("set-cookie", "SAP_SESSIONID_A4H_001=fresh; Path=/")
                    .body(SESSION_XML);
            })
            .await;
        let retried = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/sap/bc/adt/test/read")
                    .cookie("SAP_SESSIONID_A4H_001", "fresh");
                then.status(401);
            })
            .await;
        let transport = test_transport(&server);
        let request = AdtRequest::new(Method::GET, AdtUri::parse("/sap/bc/adt/test/read").unwrap());

        let response = transport.send(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        initial.assert_hits_async(1).await;
        relogon.assert_hits_async(1).await;
        retried.assert_hits_async(1).await;
    }

    #[tokio::test]
    async fn reconnects_before_using_an_inactive_security_session() {
        let server = MockServer::start_async().await;
        let relogon = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(HTTP_SESSIONS_PATH)
                    .header("sap-adt-purpose", PREFLIGHT_LOGON_PURPOSE);
                then.status(200)
                    .header("content-type", SESSION_MEDIA_TYPE)
                    .header("set-cookie", "SAP_SESSIONID_A4H_001=fresh; Path=/")
                    .body(SESSION_XML);
            })
            .await;
        let read = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/sap/bc/adt/test/read")
                    .cookie("SAP_SESSIONID_A4H_001", "fresh");
                then.status(200);
            })
            .await;
        let transport = test_transport(&server);
        transport
            .security
            .set_inactive(Duration::from_secs(2), Duration::from_secs(1))
            .await;
        let request = AdtRequest::new(Method::GET, AdtUri::parse("/sap/bc/adt/test/read").unwrap());

        let response = transport.send(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        relogon.assert_hits_async(1).await;
        read.assert_hits_async(1).await;
    }
}
