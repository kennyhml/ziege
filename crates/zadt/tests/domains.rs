#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    Client, Discovery, Domain, DomainCreateProperties, DomainProperties, EntityTag, Logon,
    MediaTyped, ObjectKey, Operation, ReqwestTransport, WorkbenchVersion,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const DOMAIN_XML: &str = include_str!("fixtures/domain-trkorr.xml");
const DOMAIN_MEDIA_TYPE: &str = "application/vnd.sap.adt.domains.v2+xml";
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";

async fn mock_logon(server: &MockServer) -> Mock<'_> {
    server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/core/http/sessions");
            then.status(200)
                .header("content-type", SESSION_MEDIA_TYPE)
                .body(SESSION_XML);
        })
        .await
}

async fn mock_discovery(server: &MockServer) -> Mock<'_> {
    server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(200).body(DISCOVERY_XML);
        })
        .await
}

async fn mock_core_discovery(server: &MockServer) -> Mock<'_> {
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("accept", "application/atomsvc+xml");
            then.status(200).body(CORE_DISCOVERY_XML);
        })
        .await
}

async fn discovered_client(server: &MockServer) -> Client<Discovery> {
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();
    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();
    client.discover().await.unwrap()
}

#[tokio::test]
async fn domain_properties_preserve_the_nested_v2_contract() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/domains/trkorr")
                .query_param("version", "active")
                .header("accept", DOMAIN_MEDIA_TYPE)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.domains.v2+xml; charset=utf-8",
                )
                .header("etag", "domain-etag")
                .body(DOMAIN_XML);
        })
        .await;

    let client = discovered_client(&server).await;
    let reference = ObjectKey::<Domain>::new("TRKORR");
    let object = reference
        .query()
        .workbench_version(WorkbenchVersion::Active)
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(object.media_type(), DomainProperties::MEDIA_TYPES[0]);
    assert_eq!(
        object.properties().content.type_information.length,
        "000020"
    );
    assert_eq!(
        object
            .properties()
            .content
            .value_information
            .as_ref()
            .and_then(|values| values.value_table.name.as_deref()),
        Some("E070")
    );
    assert_eq!(object.etag().map(EntityTag::as_str), Some("domain-etag"));

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
}

#[tokio::test]
async fn domain_creation_posts_only_the_sparse_properties_payload() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let csrf = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("x-csrf-token", "Fetch")
                .header("x-sap-adt-sessiontype", "stateless");
            then.status(200)
                .header("x-csrf-token", "CSRF-TOKEN-DOMAIN-CREATE");
        })
        .await;
    let create = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/ddic/domains")
                .header("content-type", DOMAIN_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-TOKEN-DOMAIN-CREATE")
                .body_includes("<doma:domain")
                .body_includes("adtcore:name=\"Z_DOMAIN\"")
                .body_includes("adtcore:type=\"DOMA/DD\"")
                .body_includes("adtcore:description=\"Created Domain\"")
                .body_includes("<adtcore:packageRef adtcore:name=\"$TMP\"");
            then.status(201);
        })
        .await;

    let client = discovered_client(&server).await;
    let reference = ObjectKey::<Domain>::new("Z_DOMAIN");
    let properties = DomainCreateProperties::builder()
        .description("Created Domain")
        .package("$TMP")
        .build()
        .unwrap();
    reference.create(properties).execute(&client).await.unwrap();
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    create.assert_async().await;
}
