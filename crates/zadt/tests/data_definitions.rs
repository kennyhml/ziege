#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    Client, DataDefinition, DataDefinitionCreateProperties, DataDefinitionProperties, EntityTag,
    Logon, MediaTyped, Operation, Ready, ReqwestTransport, WorkbenchVersion,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const DATA_DEFINITION_XML: &str = include_str!("fixtures/data-definition-i-businesspartner.xml");
const DATA_DEFINITION_MEDIA_TYPE: &str = "application/vnd.sap.adt.ddlSource+xml";
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const SOURCE: &str = "define view entity I_BUSINESSPARTNER as select from but000 { key partner }\n";

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

async fn ready_client(server: &MockServer) -> Client<Ready> {
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
async fn data_definition_properties_advertise_the_primary_source() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/ddl/sources/i_businesspartner")
                .query_param("version", "active")
                .header("accept", DATA_DEFINITION_MEDIA_TYPE)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.ddlSource+xml; charset=utf-8",
                )
                .header("etag", "data-definition-etag")
                .body(DATA_DEFINITION_XML);
        })
        .await;
    let source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/ddl/sources/i_businesspartner/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(SOURCE);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client
        .object::<DataDefinition>("I_BUSINESSPARTNER")
        .unwrap();
    let object = reference
        .query()
        .workbench_version(WorkbenchVersion::Active)
        .execute(&client)
        .await
        .unwrap();
    let source_code = object
        .source()
        .unwrap()
        .query()
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(
        object.media_type(),
        DataDefinitionProperties::MEDIA_TYPES[0]
    );
    assert_eq!(object.properties().source_type.as_deref(), Some("view"));
    assert_eq!(object.properties().source_uri, "source/main");
    assert_eq!(
        object.etag().map(EntityTag::as_str),
        Some("data-definition-etag")
    );
    assert_eq!(source_code.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
    source.assert_async().await;
}

#[tokio::test]
async fn data_definition_creation_posts_only_the_sparse_properties_payload() {
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
                .header("x-csrf-token", "CSRF-TOKEN-DDLS-CREATE");
        })
        .await;
    let create = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/ddic/ddl/sources")
                .header("content-type", DATA_DEFINITION_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-TOKEN-DDLS-CREATE")
                .body_includes("<ddl:ddlSource")
                .body_includes("adtcore:name=\"Z_DATA_DEFINITION\"")
                .body_includes("adtcore:type=\"DDLS/DF\"")
                .body_includes("adtcore:description=\"Created Data Definition\"")
                .body_includes("<adtcore:packageRef adtcore:name=\"$TMP\"");
            then.status(201);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client
        .object::<DataDefinition>("Z_DATA_DEFINITION")
        .unwrap();
    let properties = DataDefinitionCreateProperties::builder()
        .description("Created Data Definition")
        .package("$TMP")
        .build()
        .unwrap();
    let created = reference.create(properties).execute(&client).await.unwrap();

    assert!(created.is_none());
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    create.assert_async().await;
}
