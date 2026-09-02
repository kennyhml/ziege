#![cfg(feature = "reqwest")]

use httpmock::{Mock, prelude::*};
use zadt::{Class, Client, DeletionObject, ObjectDeletion, Operation, Ready, ReqwestTransport};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const CHECK_REQUEST_MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.check.request.v1+xml";
const CHECK_RESPONSE_MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.check.response.v1+xml";
const DELETE_REQUEST_MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.request.v1+xml";
const DELETE_RESPONSE_MEDIA_TYPE: &str = "application/vnd.sap.adt.deletion.response.v1+xml";
const CHECK_RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?><del:checkResponse xmlns:del="http://www.sap.com/adt/deletion"><del:object del:externalStrongReferences="0" del:externalWeakReferences="0" del:isDeletable="true" adtcore:uri="/sap/bc/adt/oo/classes/zmyclass" adtcore:type="CLAS/OC" adtcore:name="ZMYCLASS" adtcore:packageName="ZZZMYPACKAGE" xmlns:adtcore="http://www.sap.com/adt/core"><del:lockingTransport><del:recording>false</del:recording><del:result/><del:lockingTransport><del:trkorr>A4HK900148</del:trkorr><del:owner>KD</del:owner><del:description>transport</del:description></del:lockingTransport><del:transportLayer/></del:lockingTransport><del:message del:priority="0" del:type="S"><del:text/></del:message></del:object></del:checkResponse>"#;
const DELETE_RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?><del:deletionResult xmlns:del="http://www.sap.com/adt/deletion"><del:object del:isDeleted="true" adtcore:uri="/sap/bc/adt/oo/classes/zmyclass" adtcore:type="CLAS/OC" adtcore:name="ZMYCLASS" adtcore:packageName="ZZZMYPACKAGE" xmlns:adtcore="http://www.sap.com/adt/core"><del:message del:priority="0" del:type="S"><del:text/></del:message></del:object></del:deletionResult>"#;

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

async fn mock_csrf(server: &MockServer) -> Mock<'_> {
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("x-csrf-token", "Fetch");
            then.status(200).header("x-csrf-token", "CSRF-DELETION");
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
    Client::new(transport).discover().await.unwrap()
}

#[tokio::test]
async fn deletion_check_uses_the_discovered_contract() {
    let server = MockServer::start_async().await;
    let _discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let csrf = mock_csrf(&server).await;
    let check_request = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/deletion/check")
                .header("accept", CHECK_RESPONSE_MEDIA_TYPE)
                .header("content-type", CHECK_REQUEST_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-DELETION")
                .body_includes("<del:checkRequest")
                .body_includes("adtcore:uri=\"/sap/bc/adt/oo/classes/zmyclass\"");
            then.status(200)
                .header("content-type", CHECK_RESPONSE_MEDIA_TYPE)
                .body(CHECK_RESPONSE);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<Class>("ZMYCLASS").unwrap();
    let result = reference.deletion_check().execute(&client).await.unwrap();

    assert_eq!(result.objects.len(), 1);
    assert!(result.objects[0].is_deletable);
    assert_eq!(result.objects[0].name, "ZMYCLASS");
    csrf.assert_async().await;
    check_request.assert_async().await;
}

#[tokio::test]
async fn deletion_records_each_object_in_its_transport() {
    let server = MockServer::start_async().await;
    let _discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let csrf = mock_csrf(&server).await;
    let delete_request = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/deletion/delete")
                .header("accept", DELETE_RESPONSE_MEDIA_TYPE)
                .header("content-type", DELETE_REQUEST_MEDIA_TYPE)
                .header("x-csrf-token", "CSRF-DELETION")
                .body_includes("<del:deletionRequest")
                .body_includes("adtcore:uri=\"/sap/bc/adt/oo/classes/zmyclass\"")
                .body_includes("<del:transportNumber>A4HK900148</del:transportNumber>");
            then.status(200)
                .header("content-type", DELETE_RESPONSE_MEDIA_TYPE)
                .body(DELETE_RESPONSE);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<Class>("ZMYCLASS").unwrap();
    let mut deletion = ObjectDeletion::new();
    deletion.push_object(DeletionObject::new(&reference).transport("A4HK900148"));
    let result = deletion.execute(&client).await.unwrap();

    assert_eq!(result.objects.len(), 1);
    assert!(result.objects[0].is_deleted);
    assert_eq!(result.objects[0].name.as_deref(), Some("ZMYCLASS"));
    csrf.assert_async().await;
    delete_request.assert_async().await;
}
