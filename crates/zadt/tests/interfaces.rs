#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    Client, EntityTag, Interface, InterfaceCreateProperties, InterfacePropertiesVersion, Logon,
    ObjectVersion, Operation, Ready, ReqwestTransport,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const INTERFACE_XML: &str = include_str!("fixtures/interface-if-adt-uri-mapper-v5.xml");
const INTERFACE_V5_MEDIA_TYPE: &str = "application/vnd.sap.adt.oo.interfaces.v5+xml";
const INTERFACE_V4_MEDIA_TYPE: &str = "application/vnd.sap.adt.oo.interfaces.v4+xml";
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const SOURCE: &str = "interface IF_ADT_URI_MAPPER public.\nendinterface.\n";

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
async fn interface_properties_advertise_source_and_structure() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/interfaces/if_adt_uri_mapper")
                .query_param("version", "active")
                .header(
                    "accept",
                    format!("{INTERFACE_V5_MEDIA_TYPE}, {INTERFACE_V4_MEDIA_TYPE}"),
                )
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.oo.interfaces.v5+xml; charset=utf-8",
                )
                .header("etag", "interface-etag")
                .body(INTERFACE_XML);
        })
        .await;
    let source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/interfaces/if_adt_uri_mapper/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(SOURCE);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<Interface>("IF_ADT_URI_MAPPER").unwrap();
    let object = reference
        .query()
        .version(ObjectVersion::Active)
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
    let structure = object.object_structure().unwrap();

    assert_eq!(object.media_version(), InterfacePropertiesVersion::V5);
    assert_eq!(object.properties.source_uri, "source/main");
    assert_eq!(
        structure.resource.uri.as_str(),
        "/sap/bc/adt/oo/interfaces/if_adt_uri_mapper/objectstructure"
    );
    assert_eq!(
        object.etag.as_ref().map(EntityTag::as_str),
        Some("interface-etag")
    );
    assert_eq!(source_code.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
    source.assert_async().await;
}

#[tokio::test]
async fn interface_creation_posts_only_the_sparse_properties_payload() {
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
                .header("x-csrf-token", "CSRF-TOKEN-INTERFACE-CREATE");
        })
        .await;
    let create = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/oo/interfaces")
                .header("content-type", INTERFACE_V5_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-TOKEN-INTERFACE-CREATE")
                .body_contains("<intf:abapInterface")
                .body_contains("adtcore:name=\"ZIF_EXAMPLE\"")
                .body_contains("adtcore:type=\"INTF/OI\"")
                .body_contains("adtcore:description=\"Created Interface\"")
                .body_contains("<adtcore:packageRef adtcore:name=\"$TMP\"");
            then.status(201);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<Interface>("ZIF_EXAMPLE").unwrap();
    let properties = InterfaceCreateProperties::builder()
        .description("Created Interface")
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
