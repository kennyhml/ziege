#![cfg(feature = "reqwest")]

use httpmock::Mock;
use httpmock::prelude::*;
use zadt::{
    AccessMode, Client, EntityTag, Include, IncludeProperties, Logon, MediaTyped, Operation,
    Program, ProgramProperties, Ready, ReqwestTransport, Revalidation, WorkbenchVersion,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const LOCK_XML: &str = include_str!("fixtures/object-lock.xml");
// Captured from live A4H. Its V2 and V3 response bodies were byte-identical.
const PROGRAM_XML: &str = include_str!("fixtures/program-z-test.xml");
const INCLUDE_XML: &str = include_str!("fixtures/include-ztest.xml");
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const PROGRAM_PROPERTIES_ACCEPT: &str = "application/vnd.sap.adt.programs.programs.v3+xml, application/vnd.sap.adt.programs.programs.v2+xml";
const SOURCE: &str = "REPORT z_ziege_test.\nWRITE / 'updated'.\n";
const RUN_OUTPUT: &str = "Hello from Z_TEST\n";

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

async fn ready_client(transport: ReqwestTransport) -> Client<Ready> {
    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();
    client.discover().await.unwrap()
}

#[tokio::test]
async fn program_run_uses_the_advertised_profiled_template() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
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
                .path("/sap/bc/adt/programs/programrun/z_test")
                .query_param("profilerId", "TRACE ID")
                .header("accept", "text/plain")
                .header("x-csrf-token", "CSRF-TOKEN-RUN")
                .body("");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(RUN_OUTPUT);
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = ready_client(transport).await;
    let program = client.object::<Program>("z_test").unwrap();
    let output = program
        .run()
        .profiler_id("TRACE ID")
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(program.name(), "Z_TEST");
    assert_eq!(output.reference, program);
    assert_eq!(output.content, RUN_OUTPUT);
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    run.assert_async().await;
}

#[tokio::test]
async fn include_properties_query_converts_the_live_ztest_properties() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/includes/ztest")
                .query_param("version", "active")
                .header("accept", "application/vnd.sap.adt.programs.includes.v2+xml")
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.programs.includes.v2+xml; charset=utf-8",
                )
                .header("etag", "2026012416174900180")
                .body(INCLUDE_XML);
        })
        .await;
    let get_source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/includes/ztest/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(SOURCE);
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = ready_client(transport).await;
    let reference = client.object::<Include>("ZTEST").unwrap();
    let response = reference
        .query()
        .workbench_version(WorkbenchVersion::Active)
        .execute(&client)
        .await
        .unwrap();
    assert_eq!(response.media_type(), IncludeProperties::MEDIA_TYPES[0]);
    let include = response.properties();
    let source = response
        .source()
        .unwrap()
        .query()
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(include.name, "ZTEST");
    assert_eq!(include.object_type.to_string(), "PROG/I");
    assert_eq!(include.version, WorkbenchVersion::Active);
    assert_eq!(include.context_ref_count, 0);
    assert_eq!(include.package.name.as_deref(), Some("$TMP"));
    assert_eq!(include.links.len(), 7);
    assert_eq!(
        response.etag().map(EntityTag::as_str),
        Some("2026012416174900180")
    );
    assert_eq!(source.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
    get_source.assert_async().await;
}

#[tokio::test]
async fn program_properties_query_converts_the_live_z_test_v3_properties() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/discovery")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/programs/z_test")
                .header("accept", PROGRAM_PROPERTIES_ACCEPT)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.programs.programs.v3+xml; charset=utf-8",
                )
                .header("etag", "202607251959580008")
                .body(PROGRAM_XML);
        })
        .await;
    let get_source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/programs/z_test/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .body(SOURCE);
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = ready_client(transport).await;
    let reference = client.object::<Program>("Z_TEST").unwrap();
    let response = reference.query().execute(&client).await.unwrap();
    assert_eq!(response.media_type(), ProgramProperties::MEDIA_TYPES[0]);
    let program = response.properties();
    let source = response
        .source()
        .unwrap()
        .query()
        .execute(&client)
        .await
        .unwrap();

    assert_eq!(program.name, "Z_TEST");
    assert_eq!(program.object_type.to_string(), "PROG/P");
    assert_eq!(program.version, WorkbenchVersion::Inactive);
    assert_eq!(program.program_type, "executableProgram");
    assert!(program.fix_point_arithmetic);
    assert!(program.unicode_check_active);
    assert_eq!(program.package.name.as_deref(), Some("$TMP"));
    assert_eq!(
        program.package.object_type.as_ref().unwrap().as_str(),
        "DEVC/K"
    );
    assert_eq!(
        program.package.uri.as_deref(),
        Some("/sap/bc/adt/packages/%24tmp")
    );
    assert_eq!(
        program.syntax_configuration.language.version,
        zadt::AbapLanguageVersion::StandardX
    );
    assert_eq!(
        program.syntax_configuration.language.description,
        "Standard ABAP"
    );
    assert_eq!(program.links.len(), 9);
    let text_elements_link = program
        .links
        .iter()
        .find(|link| {
            link.relation.as_deref()
                == Some("http://www.sap.com/adt/relations/sources/textelements")
        })
        .unwrap();
    assert_eq!(text_elements_link.title.as_deref(), Some("Text Elements"));
    assert_eq!(
        text_elements_link.href,
        "/sap/bc/adt/textelements/programs/z_test"
    );
    let parser_link = &program.syntax_configuration.language.links[0];
    assert_eq!(
        parser_link.relation.as_deref(),
        Some("http://www.sap.com/adt/relations/abapsource/parser")
    );
    assert_eq!(parser_link.media_type.as_deref(), Some("text/plain"));
    assert_eq!(parser_link.title.as_deref(), Some("Standard ABAP"));
    assert_eq!(parser_link.etag.as_deref(), Some("757"));
    assert_eq!(program.source_uri, "source/main");
    assert_eq!(program.links[0].href, "source/main/versions");
    assert_eq!(
        response.etag().map(EntityTag::as_str),
        Some("202607251959580008")
    );
    assert_eq!(source.content, SOURCE);

    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
    get_source.assert_async().await;
}

#[tokio::test]
async fn program_properties_query_accepts_server_selected_v2() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/programs/z_test")
                .query_param("version", "workingArea")
                .header("accept", PROGRAM_PROPERTIES_ACCEPT);
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.programs.programs.v2+xml; charset=utf-8",
                )
                .header("etag", "202607251959580008")
                .body(PROGRAM_XML);
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = ready_client(transport).await;
    let response = client
        .object::<Program>("Z_TEST")
        .unwrap()
        .query()
        .workbench_version(WorkbenchVersion::WorkingArea)
        .execute(&client)
        .await
        .unwrap();
    assert_eq!(response.media_type(), ProgramProperties::MEDIA_TYPES[1]);
    let program = response.properties();

    assert_eq!(program.name, "Z_TEST");
    assert_eq!(program.version, WorkbenchVersion::Inactive);
    assert_eq!(program.source_uri, "source/main");
    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
}

#[tokio::test]
async fn program_properties_query_returns_not_modified_for_a_current_etag() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/programs/z_test")
                .query_param("version", "inactive")
                .header("accept", PROGRAM_PROPERTIES_ACCEPT)
                .header("if-none-match", "202607251959580008");
            then.status(304).header("etag", "202607251959580008");
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = ready_client(transport).await;
    let response = client
        .object::<Program>("Z_TEST")
        .unwrap()
        .query()
        .workbench_version(WorkbenchVersion::Inactive)
        .if_none_match(EntityTag::from_static("202607251959580008"))
        .execute(&client)
        .await
        .unwrap();

    assert!(matches!(
        &response,
        Revalidation::NotModified {
            etag: Some(etag)
        } if etag == "202607251959580008"
    ));
    assert_eq!(response.not_modified_etag(), Some("202607251959580008"));
    assert!(response.as_modified().is_none());
    logon.assert_async().await;
    discovery.assert_async().await;
    metadata.assert_async().await;
}

#[tokio::test]
async fn program_lock_and_update_share_one_user_session() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let _core_discovery = mock_core_discovery(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/discovery")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let csrf = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("x-csrf-token", "Fetch")
                .header("x-sap-adt-sessiontype", "stateless")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
            then.status(200).header("x-csrf-token", "CSRF-TOKEN-1");
        })
        .await;
    let metadata = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/programs/z_ziege_test")
                .header("accept", PROGRAM_PROPERTIES_ACCEPT)
                .header("cache-control", "no-cache");
            then.status(200)
                .header(
                    "content-type",
                    "application/vnd.sap.adt.programs.programs.v3+xml; charset=utf-8",
                )
                .body(
                    PROGRAM_XML
                        .replace("Z_TEST", "Z_ZIEGE_TEST")
                        .replace("z_test", "z_ziege_test"),
                );
        })
        .await;
    let get_source = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/programs/programs/z_ziege_test/source/main")
                .header("accept", "text/plain");
            then.status(200)
                .header("content-type", "text/plain; charset=utf-8")
                .header("etag", "SOURCE-ETAG-1")
                .body(SOURCE);
        })
        .await;
    let lock_program = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/programs/programs/z_ziege_test")
                .query_param("_action", "LOCK")
                .query_param("accessMode", "MODIFY")
                .header(
                    "accept",
                    "application/vnd.sap.as+xml; charset=utf-8; dataname=com.sap.adt.lock.Result2",
                )
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-1")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
            then.status(200)
                .header(
                    "set-cookie",
                    "sap-contextid=USER-SESSION-1; Path=/sap/bc/adt",
                )
                .body(LOCK_XML);
        })
        .await;
    let update_source = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/sap/bc/adt/programs/programs/z_ziege_test/source/main")
                .query_param("lockHandle", "LOCK-HANDLE-1")
                .header("content-type", "text/plain; charset=utf-8")
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-1")
                .header(
                    "cookie",
                    "sap-usercontext=sap-client=001&sap-language=EN; sap-contextid=USER-SESSION-1",
                )
                .body(SOURCE);
            then.status(200)
                .header("etag", "SOURCE-ETAG-2")
                .body(SOURCE);
        })
        .await;
    let unlock_program = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/sap/bc/adt/programs/programs/z_ziege_test")
                .query_param("_action", "UNLOCK")
                .query_param("lockHandle", "LOCK-HANDLE-1")
                .header("x-sap-adt-sessiontype", "stateful")
                .header("x-csrf-token", "CSRF-TOKEN-1")
                .header(
                    "cookie",
                    "sap-usercontext=sap-client=001&sap-language=EN; sap-contextid=USER-SESSION-1",
                );
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
                    "sap-usercontext=sap-client=001&sap-language=EN; sap-contextid=USER-SESSION-1",
                );
            then.status(200);
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = ready_client(transport).await;
    let program = client
        .object::<Program>("Z_ZIEGE_TEST")
        .unwrap()
        .query()
        .execute(&client)
        .await
        .unwrap();
    let source = program
        .source()
        .unwrap()
        .query()
        .execute(&client)
        .await
        .unwrap();
    let session = client.create_user_session();

    let object_lock = program
        .lock(AccessMode::Modify)
        .execute(&session)
        .await
        .unwrap();
    let updated = source
        .reference
        .update(&object_lock, source.content.as_str())
        .unwrap()
        .execute(&session)
        .await
        .unwrap();
    assert_eq!(object_lock.object().uri(), program.reference().uri());
    assert_eq!(object_lock.handle(), "LOCK-HANDLE-1");
    program
        .unlock(object_lock)
        .unwrap()
        .execute(&session)
        .await
        .unwrap();
    session.close().await.unwrap();

    assert_eq!(source.etag.as_deref(), Some("SOURCE-ETAG-1"));
    assert_eq!(updated.content.as_deref(), Some(SOURCE));
    assert_eq!(updated.etag.as_deref(), Some("SOURCE-ETAG-2"));
    logon.assert_async().await;
    discovery.assert_async().await;
    csrf.assert_async().await;
    metadata.assert_async().await;
    get_source.assert_async().await;
    lock_program.assert_async().await;
    update_source.assert_async().await;
    unlock_program.assert_async().await;
    close_session.assert_async().await;
}
