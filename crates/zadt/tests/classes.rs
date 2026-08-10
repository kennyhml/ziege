#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    Class, ClassProperties, ClassPropertiesVersion, ClassSourceComponent, Client, EntityTag, Logon,
    ObjectVersion, Operation, Ready, RepositoryObject, ReqwestTransport, Revalidation,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const CLASS_XML: &str = include_str!("fixtures/class-cl-adt-uri-mapper-v4.xml");
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const SOURCE: &str = "CLASS cl_adt_uri_mapper DEFINITION.\nENDCLASS.\n";
const RUN_OUTPUT: &str = "Hello from IF_OO_ADT_CLASSRUN\n";

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

#[tokio::test]
async fn class_run_uses_the_advertised_plain_text_contract() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = mock_discovery(&server).await;
    let csrf = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("x-csrf-token", "Fetch")
                .header("x-sap-adt-sessiontype", "stateless");
            then.status(200).header("x-csrf-token", "CSRF-TOKEN-RUN");
        })
        .await;
    let run = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/oo/classrun/cl_adt_uri_mapper")
                .query_param("profilerId", "TRACE ID")
                .header("accept", "text/plain")
                .header("x-csrf-token", "CSRF-TOKEN-RUN")
                .body("");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(RUN_OUTPUT);
        })
        .await;

    let client = ready_client(transport(&server)).await;
    let class = client.object::<Class>("CL_ADT_URI_MAPPER").unwrap();
    let output = class
        .run()
        .profiler_id("TRACE ID")
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(output.reference, class);
    assert_eq!(output.content, RUN_OUTPUT);
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    run.assert_async().await;
}

#[tokio::test]
async fn repository_object_fetches_class_properties_as_json() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = mock_discovery(&server).await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/classes/cl_adt_uri_mapper")
                .query_param("version", "active")
                .header("accept", "application/vnd.sap.adt.oo.classes.v4+xml")
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.oo.classes.v4+xml; charset=utf-8",
                )
                .header("etag", "20210406145501001000181")
                .body(CLASS_XML);
        })
        .await;

    let client = ready_client(transport(&server)).await;
    let reference = client.object::<Class>("CL_ADT_URI_MAPPER").unwrap();
    let object = RepositoryObject::from(reference);
    let json = object
        .properties()
        .unwrap()
        .version(ObjectVersion::Active)
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(json["mediaVersion"], "v4");
    assert_eq!(json["properties"]["name"], "CL_ADT_URI_MAPPER");
    assert_eq!(json["properties"]["objectType"], "CLAS/OC");
    assert_eq!(json["properties"]["version"], "active");
    assert_eq!(
        json["properties"]["reference"]["uri"],
        "/sap/bc/adt/oo/classes/cl_adt_uri_mapper"
    );
    assert_eq!(
        json["properties"]["sourceComponents"][0]["source"]["etag"],
        "201701161841300011"
    );
    assert_eq!(
        json["properties"]["mainSource"]["source"]["uri"],
        "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main"
    );
    assert_eq!(json["properties"]["relations"].as_array().unwrap().len(), 7);

    let mut batch = client.batch().unwrap();
    let _key = batch.push(object.properties().unwrap());
    assert_eq!(batch.len(), 1);

    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
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

async fn mock_discovery(server: &MockServer) -> Mock<'_> {
    server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(200).body(DISCOVERY_XML);
        })
        .await
}

async fn ready_client(transport: ReqwestTransport) -> Client<Ready> {
    let client = Client::new(transport);
    Logon.execute(&client).await.unwrap();
    client.discover().await.unwrap()
}

fn transport(server: &MockServer) -> ReqwestTransport {
    ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap()
}

#[tokio::test]
async fn class_properties_query_converts_the_live_v4_manifest() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = mock_discovery(&server).await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/classes/cl_adt_uri_mapper")
                .query_param("version", "active")
                .header("accept", "application/vnd.sap.adt.oo.classes.v4+xml")
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.oo.classes.v4+xml; charset=utf-8",
                )
                .header("etag", "20210406145501001000181")
                .body(CLASS_XML);
        })
        .await;
    let get_source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main")
                .header("accept", "text/plain");
            then.status(200).body(SOURCE);
        })
        .await;

    let client = ready_client(transport(&server)).await;
    let reference = client.object::<Class>("CL_ADT_URI_MAPPER").unwrap();
    let response = reference
        .query()
        .version(ObjectVersion::Active)
        .execute(&client)
        .await
        .unwrap();
    assert_eq!(response.media_version(), ClassPropertiesVersion::V4);
    let class = match response {
        ClassProperties::V4(class) => *class,
        _ => panic!("unexpected class-properties version"),
    };
    let source = class
        .main_source()
        .source
        .query()
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(class.reference, reference);
    assert_eq!(class.name, "CL_ADT_URI_MAPPER");
    assert_eq!(class.version, ObjectVersion::Active);
    assert_eq!(class.package.name(), "SADT_TOOLS_CORE");
    assert_eq!(class.source_components.len(), 4);
    assert_eq!(class.etag.as_deref(), Some("20210406145501001000181"));
    assert_eq!(
        class
            .source(ClassSourceComponent::Definitions)
            .unwrap()
            .source
            .etag
            .as_deref(),
        Some("201701161841300011")
    );
    assert_eq!(source.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
    get_source.assert_async().await;
}

#[tokio::test]
async fn class_properties_query_honors_v2_and_v3_priority() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = mock_discovery(&server).await;
    let v2 = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/classes/cl_adt_uri_mapper")
                .header("accept", "application/vnd.sap.adt.oo.classes.v2+xml");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.oo.classes.v2+xml; charset=utf-8",
                )
                .body(CLASS_XML);
        })
        .await;
    let v3 = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/classes/cl_adt_uri_mapper")
                .header("accept", "application/vnd.sap.adt.oo.classes.v3+xml");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.oo.classes.v3+xml; charset=utf-8",
                )
                .body(CLASS_XML);
        })
        .await;

    let client = ready_client(transport(&server)).await;
    let reference = client.object::<Class>("CL_ADT_URI_MAPPER").unwrap();
    let response = reference
        .query()
        .priority([ClassPropertiesVersion::V2])
        .execute(&client)
        .await
        .unwrap();
    assert!(matches!(response, ClassProperties::V2(_)));
    let response = reference
        .query()
        .priority([ClassPropertiesVersion::V3])
        .execute(&client)
        .await
        .unwrap();
    assert!(matches!(response, ClassProperties::V3(_)));

    logon.assert_async().await;
    discovery.assert_async().await;
    v2.assert_async().await;
    v3.assert_async().await;
}

#[tokio::test]
async fn class_properties_query_returns_not_modified_for_a_current_etag() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = mock_discovery(&server).await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/oo/classes/cx_root")
                .query_param("version", "active")
                .header("accept", "application/vnd.sap.adt.oo.classes.v4+xml")
                .header("if-none-match", "20180326130103001000061");
            then.status(304).header("etag", "20180326130103001000061");
        })
        .await;

    let client = ready_client(transport(&server)).await;
    let reference = client.object::<Class>("CX_ROOT").unwrap();
    let object = RepositoryObject::from(reference);
    let response = object
        .properties()
        .unwrap()
        .version(ObjectVersion::Active)
        .if_none_match(EntityTag::from_static("20180326130103001000061"))
        .execute(&client)
        .await
        .unwrap();

    assert!(matches!(response, Revalidation::NotModified { .. }));
    assert_eq!(
        response.not_modified_etag(),
        Some("20180326130103001000061")
    );
    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
}
