#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    Client, FunctionGroup, FunctionGroupInclude, FunctionGroupIncludeProperties,
    FunctionGroupProperties, FunctionModule, FunctionModuleProperties, Logon, MediaTyped,
    Operation, Ready, ReqwestTransport,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const GROUP_XML: &str = include_str!("fixtures/function-group-z-test-group.xml");
const MODULE_XML: &str = include_str!("fixtures/function-module-zzzzfunc.xml");
const INCLUDE_XML: &str = include_str!("fixtures/function-group-include-lz-test-grouptop.xml");
const MODULE_SOURCE: &str = "FUNCTION ZZZZFUNC.\nENDFUNCTION.\n";

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
async fn function_group_family_uses_discovered_subobject_targets() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let group_properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/functions/groups/z_test_group")
                .header(
                    "accept",
                    "application/vnd.sap.adt.functions.groups.v3+xml, application/vnd.sap.adt.functions.groups.v2+xml",
                );
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.functions.groups.v3+xml; charset=utf-8",
                )
                .body(GROUP_XML);
        })
        .await;
    let module_properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc")
                .header(
                    "accept",
                    "application/vnd.sap.adt.functions.fmodules.v3+xml",
                );
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.functions.fmodules.v3+xml; charset=utf-8",
                )
                .body(MODULE_XML);
        })
        .await;
    let include_properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/functions/groups/z_test_group/includes/lz_test_grouptop")
                .header(
                    "accept",
                    "application/vnd.sap.adt.functions.fincludes.v2+xml",
                );
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.functions.fincludes.v2+xml; charset=utf-8",
                )
                .body(INCLUDE_XML);
        })
        .await;
    let module_source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(MODULE_SOURCE);
        })
        .await;

    let client = ready_client(&server).await;
    let group_reference = client.object::<FunctionGroup>("Z_TEST_GROUP").unwrap();
    let module_reference = group_reference
        .subobject::<FunctionModule>("ZZZZFUNC")
        .unwrap();
    let include_reference = group_reference
        .subobject::<FunctionGroupInclude>("LZ_TEST_GROUPTOP")
        .unwrap();
    let group = group_reference.query().execute(&client).await.unwrap();
    let module = module_reference.query().execute(&client).await.unwrap();
    let include = include_reference.query().execute(&client).await.unwrap();
    let source = module
        .source()
        .unwrap()
        .query()
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(group.media_type(), FunctionGroupProperties::MEDIA_TYPES[0]);
    assert_eq!(
        module.media_type(),
        FunctionModuleProperties::MEDIA_TYPES[0]
    );
    assert_eq!(
        include.media_type(),
        FunctionGroupIncludeProperties::MEDIA_TYPES[0]
    );
    assert_eq!(
        module.properties().container.name.as_deref(),
        Some("Z_TEST_GROUP")
    );
    assert_eq!(
        include.properties().container.name.as_deref(),
        Some("Z_TEST_GROUP")
    );
    assert_eq!(source.content, MODULE_SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    group_properties.assert_async().await;
    module_properties.assert_async().await;
    include_properties.assert_async().await;
    module_source.assert_async().await;
}
