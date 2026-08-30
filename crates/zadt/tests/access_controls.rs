#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    AccessControl, AccessControlCreateProperties, AccessControlProperties, Client, EntityTag,
    Logon, MediaTyped, ObjectVersion, Operation, Ready, ReqwestTransport,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const ACCESS_CONTROL_XML: &str =
    include_str!("fixtures/access-control-sdsh-cds-domain-val-dcl.xml");
const ACCESS_CONTROL_MEDIA_TYPE: &str = "application/vnd.sap.adt.dclSource+xml";
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const SOURCE: &str =
    "@MappingRole: true\ndefine role Z_Access_Control { grant select on Z_View; }\n";

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
async fn access_control_properties_advertise_the_primary_source() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/acm/dcl/sources/sdsh_cds_domain_val_dcl")
                .query_param("version", "active")
                .header("accept", ACCESS_CONTROL_MEDIA_TYPE)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.dclSource+xml; charset=utf-8",
                )
                .header("etag", "access-control-etag")
                .body(ACCESS_CONTROL_XML);
        })
        .await;
    let source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/acm/dcl/sources/sdsh_cds_domain_val_dcl/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(SOURCE);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client
        .object::<AccessControl>("SDSH_CDS_DOMAIN_VAL_DCL")
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

    assert_eq!(object.media_type(), AccessControlProperties::MEDIA_TYPES[0]);
    assert_eq!(object.properties().source_uri, "source/main");
    assert_eq!(
        object.etag.as_ref().map(EntityTag::as_str),
        Some("access-control-etag")
    );
    assert_eq!(source_code.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
    source.assert_async().await;
}

#[tokio::test]
async fn access_control_creation_posts_only_the_sparse_properties_payload() {
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
                .header("x-csrf-token", "CSRF-TOKEN-DCLS-CREATE");
        })
        .await;
    let create = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/acm/dcl/sources")
                .header("content-type", ACCESS_CONTROL_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-TOKEN-DCLS-CREATE")
                .body_includes("<dcl:dclSource")
                .body_includes("adtcore:name=\"Z_ACCESS_CONTROL\"")
                .body_includes("adtcore:type=\"DCLS/DL\"")
                .body_includes("adtcore:description=\"Created Access Control\"")
                .body_includes("<adtcore:packageRef adtcore:name=\"$TMP\"");
            then.status(201);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<AccessControl>("Z_ACCESS_CONTROL").unwrap();
    let properties = AccessControlCreateProperties::builder()
        .description("Created Access Control")
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
