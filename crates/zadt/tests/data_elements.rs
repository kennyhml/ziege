#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    AccessMode, Client, DataElement, DataElementProperties, Discovery, Logon, MediaTyped,
    ObjectRef, Operation, ReqwestTransport, WorkbenchVersion,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const DATA_ELEMENT_XML: &str = include_str!("fixtures/data-element-ztfrwtfrt-v2.xml");
const LOCK_XML: &str = include_str!("fixtures/object-lock.xml");
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const DATA_ELEMENT_MEDIA_TYPE: &str = "application/vnd.sap.adt.dataelements.v2+xml";

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
async fn data_element_properties_use_one_read_write_representation() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/dataelements/ztfrwtfrt")
                .query_param("version", "workingArea")
                .header("accept", DATA_ELEMENT_MEDIA_TYPE)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.dataelements.v2+xml; charset=utf-8",
                )
                .header("etag", "data-element-etag")
                .body(DATA_ELEMENT_XML);
        })
        .await;

    let client = discovered_client(&server).await;
    let reference = ObjectRef::<DataElement>::new("ZTFRWTFRT");
    let response = reference
        .query()
        .workbench_version(WorkbenchVersion::WorkingArea)
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(response.media_type(), DataElementProperties::MEDIA_TYPES[0]);
    assert_eq!(
        response.etag().map(|etag| etag.as_str()),
        Some("data-element-etag")
    );
    assert_eq!(
        response.properties().definition.type_name.as_deref(),
        Some("CHAR0008")
    );

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
}

#[tokio::test]
async fn erased_data_element_update_uses_the_json_consumer_boundary() {
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
            then.status(200).header("x-csrf-token", "CSRF-TOKEN-DTEL");
        })
        .await;
    let get_properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/ddic/dataelements/ztfrwtfrt")
                .header("accept", DATA_ELEMENT_MEDIA_TYPE);
            then.status(200)
                .header("content-type", DATA_ELEMENT_MEDIA_TYPE)
                .header("etag", "data-element-etag-1")
                .body(DATA_ELEMENT_XML);
        })
        .await;
    let lock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/ddic/dataelements/ztfrwtfrt")
                .query_param("_action", "LOCK")
                .query_param("accessMode", "MODIFY")
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-DTEL");
            then.status(200)
                .header(
                    "set-cookie",
                    "sap-contextid=DATA-ELEMENT-SESSION; Path=/sap/bc/adt",
                )
                .body(LOCK_XML);
        })
        .await;
    let update = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/sap/bc/adt/ddic/dataelements/ztfrwtfrt")
                .query_param("lockHandle", "LOCK-HANDLE-1")
                .header("accept", DATA_ELEMENT_MEDIA_TYPE)
                .header("content-type", DATA_ELEMENT_MEDIA_TYPE)
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-DTEL")
                .header(
                    "cookie",
                    "sap-usercontext=sap-client=001&sap-language=EN; sap-contextid=DATA-ELEMENT-SESSION",
                )
                .body_includes("adtcore:name=\"ZTFRWTFRT\"")
                .body_includes("adtcore:changedAt=")
                .body_includes("<adtcore:packageRef")
                .body_includes("<atom:link")
                .body_includes("adtcore:description=\"Updated description\"")
                .body_includes("<dtel:dataType>CHAR</dtel:dataType>")
                .body_includes("<dtel:searchHelp");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.dataelements.v2+xml; charset=utf-8",
                )
                .header("etag", "data-element-etag-2")
                .body(DATA_ELEMENT_XML);
        })
        .await;
    let unlock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/ddic/dataelements/ztfrwtfrt")
                .query_param("_action", "UNLOCK")
                .query_param("lockHandle", "LOCK-HANDLE-1")
                .header("x-sap-adt-sessiontype", "stateful");
            then.status(200);
        })
        .await;
    let close_session = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("x-sap-adt-sessiontype", "stateless")
                .header(
                    "cookie",
                    "sap-usercontext=sap-client=001&sap-language=EN; sap-contextid=DATA-ELEMENT-SESSION",
                );
            then.status(200);
        })
        .await;

    let client = discovered_client(&server).await;
    let reference = ObjectRef::<DataElement>::new("ZTFRWTFRT");
    let object = reference.erase();
    let properties = object.query().execute(&client).await.unwrap();
    assert_eq!(&properties.reference().erase(), &object);
    let mut edited_properties = properties.properties().unwrap();
    edited_properties["@adtcore:description"] = "Updated description".into();
    let session = client.create_user_session();
    let object_lock = object
        .lock(AccessMode::Modify)
        .execute(&session)
        .await
        .unwrap();
    let result = properties
        .update_with_lock(object_lock.clone(), edited_properties)
        .unwrap()
        .execute(&session)
        .await
        .unwrap()
        .expect("ADT returned updated data element properties");
    object
        .unlock(object_lock)
        .unwrap()
        .execute(&session)
        .await
        .unwrap();
    session.close().await.unwrap();

    assert_eq!(result.media_type(), DATA_ELEMENT_MEDIA_TYPE);
    assert_eq!(
        result.etag().map(|etag| etag.as_str()),
        Some("data-element-etag-2")
    );
    assert_eq!(
        result.properties().unwrap()["@adtcore:description"],
        "tfarFAR"
    );
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    get_properties.assert_async().await;
    lock.assert_async().await;
    update.assert_async().await;
    unlock.assert_async().await;
    close_session.assert_async().await;
}
