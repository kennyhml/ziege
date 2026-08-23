#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    AccessMode, Client, Operation, Package, PackagePropertiesVersion, Ready, ReqwestTransport,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const PACKAGE_XML: &str = include_str!("fixtures/package-sadt-tools-core.xml");
const SUPER_TREE_XML: &str = include_str!("fixtures/package-tree-super.xml");
const SUB_TREE_XML: &str = include_str!("fixtures/package-tree-sub.xml");
const SETTINGS_XML: &str = include_str!("fixtures/package-settings.xml");
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const PACKAGE_PROPERTIES_ACCEPT: &str =
    "application/vnd.sap.adt.packages.v2+xml, application/vnd.sap.adt.packages.v1+xml";
const PACKAGE_V2_MEDIA_TYPE: &str = "application/vnd.sap.adt.packages.v2+xml";
const LOCK_XML: &str = include_str!("fixtures/object-lock.xml");

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
    client.logon().execute(&client).await.unwrap();
    client.discover().await.unwrap()
}

#[tokio::test]
async fn package_properties_advertise_all_supported_contracts() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/packages/sadt_tools_core")
                .header("accept", PACKAGE_PROPERTIES_ACCEPT)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.packages.v2+xml; charset=utf-8",
                )
                .header("etag", "package-etag")
                .body(PACKAGE_XML);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<Package>("sadt_tools_core").unwrap();
    let response = reference.query().execute(&client).await.unwrap();
    assert_eq!(response.media_version(), PackagePropertiesVersion::V2);
    let package = &response.properties;

    assert_eq!(package.name, "SADT_TOOLS_CORE");
    assert_eq!(
        response.etag.as_ref().map(zadt::EntityTag::as_str),
        Some("package-etag")
    );
    assert_eq!(
        package
            .package_interfaces
            .as_ref()
            .unwrap()
            .package_interface_ref
            .len(),
        1
    );
    assert_eq!(package.use_accesses.as_ref().unwrap().use_access.len(), 1);

    logon.assert_async().await;
    discovery.assert_async().await;
    properties.assert_async().await;
}

#[tokio::test]
async fn package_properties_use_the_universal_locked_update_flow() {
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
                .header("x-csrf-token", "CSRF-TOKEN-PACKAGE");
        })
        .await;
    let get_properties = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/packages/sadt_tools_core")
                .header("accept", PACKAGE_PROPERTIES_ACCEPT);
            then.status(200)
                .header("content-type", PACKAGE_V2_MEDIA_TYPE)
                .body(PACKAGE_XML);
        })
        .await;
    let lock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/packages/sadt_tools_core")
                .query_param("_action", "LOCK")
                .query_param("accessMode", "MODIFY")
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-PACKAGE");
            then.status(200)
                .header(
                    "set-cookie",
                    "sap-contextid=PACKAGE-SESSION; Path=/sap/bc/adt",
                )
                .body(LOCK_XML);
        })
        .await;
    let update = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/sap/bc/adt/packages/sadt_tools_core")
                .query_param("lockHandle", "LOCK-HANDLE-1")
                .header("accept", PACKAGE_V2_MEDIA_TYPE)
                .header("content-type", PACKAGE_V2_MEDIA_TYPE)
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-PACKAGE")
                .body_contains("xmlns:pak=\"http://www.sap.com/adt/packages\"")
                .body_contains("adtcore:name=\"SADT_TOOLS_CORE\"")
                .body_contains("adtcore:description=\"Updated package description\"")
                .body_contains("<pak:attributes")
                .body_contains("<atom:link");
            then.status(200);
        })
        .await;
    let unlock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/packages/sadt_tools_core")
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
                    "sap-usercontext=sap-client=001&sap-language=EN; sap-contextid=PACKAGE-SESSION",
                );
            then.status(200);
        })
        .await;

    let client = ready_client(&server).await;
    let reference = client.object::<Package>("SADT_TOOLS_CORE").unwrap();
    let mut package = reference.query().execute(&client).await.unwrap();
    package.properties.description = "Updated package description".to_owned();
    let session = client.create_user_session();
    let object_lock = reference
        .lock(AccessMode::Modify)
        .execute(&session)
        .await
        .unwrap();
    let result = package
        .update(&object_lock)
        .unwrap()
        .execute(&session)
        .await
        .unwrap();
    reference
        .unlock(object_lock)
        .unwrap()
        .execute(&session)
        .await
        .unwrap();
    session.close().await.unwrap();

    assert!(result.is_none());
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    get_properties.assert_async().await;
    lock.assert_async().await;
    update.assert_async().await;
    unlock.assert_async().await;
    close_session.assert_async().await;
}

#[tokio::test]
async fn package_tree_queries_expand_the_discovered_template() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let super_tree = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/packages/$tree")
                .query_param("packagename", "SADT_TOOLS_CORE")
                .query_param("type", "super")
                .header("accept", "application/vnd.sap.adt.packages.tree.v1+xml");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.packages.tree.v1+xml; charset=utf-8",
                )
                .body(SUPER_TREE_XML);
        })
        .await;
    let sub_tree = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/packages/$tree")
                .query_param("packagename", "SADT_TOOLS_CORE")
                .query_param("type", "sub")
                .header("accept", "application/vnd.sap.adt.packages.tree.v1+xml");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.packages.tree.v1+xml; charset=utf-8",
                )
                .body(SUB_TREE_XML);
        })
        .await;

    let client = ready_client(&server).await;
    let package = client.object::<Package>("SADT_TOOLS_CORE").unwrap();
    let ancestors = package.super_tree().execute(&client).await.unwrap();
    let children = package.sub_tree().execute(&client).await.unwrap();

    assert!(ancestors.is_super_tree);
    assert_eq!(ancestors.nodes.len(), 2);
    assert!(!children.is_super_tree);
    assert_eq!(children.nodes.len(), 1);

    logon.assert_async().await;
    discovery.assert_async().await;
    super_tree.assert_async().await;
    sub_tree.assert_async().await;
}

#[tokio::test]
async fn package_settings_use_the_discovered_collection() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = mock_discovery(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let settings = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/packages/settings")
                .header("accept", "application/vnd.sap.adt.packages.settings+xml");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.packages.settings+xml; charset=utf-8",
                )
                .body(SETTINGS_XML);
        })
        .await;

    let client = ready_client(&server).await;
    let response = client.package_settings().execute(&client).await.unwrap();

    assert!(!response.show_package_check_errors);
    logon.assert_async().await;
    discovery.assert_async().await;
    settings.assert_async().await;
}
