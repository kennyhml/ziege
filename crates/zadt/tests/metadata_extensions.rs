#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    Client, EntityTag, Logon, MediaTyped, MetadataExtension, MetadataExtensionCreateProperties,
    MetadataExtensionProperties, ObjectVersion, Operation, Ready, ReqwestTransport,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const METADATA_EXTENSION_XML: &str =
    include_str!("fixtures/metadata-extension-c-mdoapplicationscope.xml");
const METADATA_EXTENSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.ddic.ddlx.v1+xml";
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const SOURCE: &str = "@Metadata.layer: #CORE\nannotate view Z_View with { @UI.hidden: true Id; }\n";

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
async fn metadata_extension_properties_advertise_the_primary_source() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/ddlx/sources/c_mdoapplicationscope")
                .query_param("version", "active")
                .header("accept", METADATA_EXTENSION_MEDIA_TYPE)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.ddic.ddlx.v1+xml; charset=utf-8",
                )
                .header("etag", "metadata-extension-etag")
                .body(METADATA_EXTENSION_XML);
        })
        .await;
    let source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/ddlx/sources/c_mdoapplicationscope/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(SOURCE);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client
        .object::<MetadataExtension>("C_MDOAPPLICATIONSCOPE")
        .unwrap();
    let object = reference
        .query()
        .workbench_version(ObjectVersion::Active)
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
        MetadataExtensionProperties::MEDIA_TYPES[0]
    );
    assert_eq!(
        object.properties().source_uri,
        "./c_mdoapplicationscope/source/main"
    );
    assert_eq!(
        object.etag().map(EntityTag::as_str),
        Some("metadata-extension-etag")
    );
    assert_eq!(source_code.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
    source.assert_async().await;
}

#[tokio::test]
async fn metadata_extension_creation_posts_only_the_sparse_properties_payload() {
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
                .header("x-csrf-token", "CSRF-TOKEN-DDLX-CREATE");
        })
        .await;
    let create = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/ddic/ddlx/sources")
                .header("content-type", METADATA_EXTENSION_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-TOKEN-DDLX-CREATE")
                .body_includes("<ddlx:ddlxSource")
                .body_includes("adtcore:name=\"Z_METADATA_EXTENSION\"")
                .body_includes("adtcore:type=\"DDLX/EX\"")
                .body_includes("adtcore:description=\"Created Metadata Extension\"")
                .body_includes("<adtcore:packageRef adtcore:name=\"$TMP\"");
            then.status(201);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client
        .object::<MetadataExtension>("Z_METADATA_EXTENSION")
        .unwrap();
    let properties = MetadataExtensionCreateProperties::builder()
        .description("Created Metadata Extension")
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
