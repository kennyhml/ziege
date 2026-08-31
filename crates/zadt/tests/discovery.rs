#![cfg(feature = "reqwest")]

use async_trait::async_trait;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use httpmock::Mock;
use httpmock::prelude::*;
use std::sync::{Arc, Mutex};
use zadt::{
    AccessControl, AdtRequest, AdtResponse, AnnotationDefinition, CategoryId, Class, Client,
    CoreDiscoveryQuery, DataDefinition, DataElement, DiscoveryQuery, Domain, FunctionGroup,
    FunctionGroupInclude, FunctionModule, GlobalWorkbenchType, Interface, Logon, MetadataExtension,
    ObjectError, ObjectRef, ObjectType, Operation, OperationError, ReqwestTransport, ResponseError,
    ServiceDefinition, Transport, TransportError,
};

const DISCOVERY_XML: &str = include_str!("fixtures/discovery.xml");
const CORE_DISCOVERY_XML: &str = include_str!("fixtures/core-discovery.xml");
const INVALID_DISCOVERY_XML: &str = include_str!("fixtures/invalid-discovery.xml");
const SESSION_XML: &str = include_str!("fixtures/http-session-v3.xml");
const SESSION_MEDIA_TYPE: &str = "application/vnd.sap.adt.core.http.session.v3+xml";
const PROGRAMS_SCHEME: &str = "http://www.sap.com/adt/categories/programs";
const PROGRAMS_CATEGORY: CategoryId = CategoryId {
    scheme: PROGRAMS_SCHEME,
    term: "programs",
};
const COMPATIBILITY_SCHEME: &str = "http://www.sap.com/adt/categories/compatibility";
const FUNCTIONS_WITHOUT_SUBOBJECT_TEMPLATES: &str = r#"
    <app:service xmlns:app="http://www.w3.org/2007/app"
        xmlns:atom="http://www.w3.org/2005/Atom">
        <app:workspace>
            <atom:title>Function Builder</atom:title>
            <app:collection href="/sap/bc/adt/functions/groups">
                <atom:title>Function Groups</atom:title>
                <atom:category term="groups"
                    scheme="http://www.sap.com/adt/categories/functions" />
            </app:collection>
        </app:workspace>
    </app:service>
"#;

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
async fn core_discovery_is_available_before_central_discovery() {
    let transport = FixtureTransport::new(CORE_DISCOVERY_XML);
    let requests = Arc::clone(&transport.requests);
    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();

    let capabilities = CoreDiscoveryQuery.execute(&client).await.unwrap();
    let collection = capabilities
        .collection(COMPATIBILITY_SCHEME, "graph")
        .unwrap();

    assert_eq!(
        collection.target().unwrap().as_str(),
        "/sap/bc/adt/compatibility/graph"
    );
    assert!(
        capabilities
            .collection(PROGRAMS_SCHEME, "programs")
            .is_none()
    );
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        [
            "/sap/bc/adt/core/http/sessions",
            "/sap/bc/adt/core/discovery"
        ]
    );
}

#[tokio::test]
async fn client_discovery_transitions_and_retains_capabilities() {
    let transport = FixtureTransport::new(DISCOVERY_XML);
    let requests = Arc::clone(&transport.requests);
    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();
    let cloned_client = client.clone();

    let collection = client.collection(PROGRAMS_CATEGORY).unwrap();

    assert_eq!(collection.title(), Some("Programs"));
    assert_eq!(
        collection.accepted_media_types(),
        [
            "application/vnd.sap.adt.programs.programs.v2+xml",
            "application/vnd.sap.adt.programs.programs.v3+xml",
        ]
    );
    assert_eq!(collection.template_links().len(), 1);
    assert!(std::ptr::eq(
        client.capabilities(),
        cloned_client.capabilities()
    ));
    assert!(std::ptr::eq(
        client.core_capabilities(),
        cloned_client.core_capabilities()
    ));
    assert!(
        client
            .core_capabilities()
            .collection(
                "http://www.sap.com/adt/categories/system/communication/services",
                "batch"
            )
            .is_some()
    );
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        [
            "/sap/bc/adt/core/http/sessions",
            "/sap/bc/adt/discovery",
            "/sap/bc/adt/core/discovery",
        ]
    );
}

#[tokio::test]
async fn class_references_use_the_discovered_oo_collection() {
    let client = Client::new(FixtureTransport::new(DISCOVERY_XML));
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();

    let class = client.object::<Class>("ZCL_EXAMPLE").unwrap();

    assert_eq!(class.name(), "ZCL_EXAMPLE");
    assert_eq!(class.uri().as_str(), "/sap/bc/adt/oo/classes/zcl_example");
}

#[tokio::test]
async fn runtime_object_types_use_the_registered_descriptor() {
    let client = Client::new(FixtureTransport::new(DISCOVERY_XML));
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();

    for (object_type, name, expected_uri) in [
        ("PROG/P", "Z_TEST", "/sap/bc/adt/programs/programs/z_test"),
        ("PROG/I", "ZTEST", "/sap/bc/adt/programs/includes/ztest"),
        (
            "CLAS/OC",
            "ZCL_EXAMPLE",
            "/sap/bc/adt/oo/classes/zcl_example",
        ),
        ("DEVC/K", "ZPACKAGE", "/sap/bc/adt/packages/zpackage"),
        (
            "DTEL/DE",
            "ZTFRWTFRT",
            "/sap/bc/adt/ddic/dataelements/ztfrwtfrt",
        ),
        (
            "DDLS/DF",
            "I_BUSINESSPARTNER",
            "/sap/bc/adt/ddic/ddl/sources/i_businesspartner",
        ),
        (
            "DCLS/DL",
            "Z_ACCESS_CONTROL",
            "/sap/bc/adt/acm/dcl/sources/z_access_control",
        ),
        (
            "INTF/OI",
            "ZIF_EXAMPLE",
            "/sap/bc/adt/oo/interfaces/zif_example",
        ),
        (
            "DDLX/EX",
            "Z_METADATA_EXTENSION",
            "/sap/bc/adt/ddic/ddlx/sources/z_metadata_extension",
        ),
        (
            "SRVD/SRV",
            "Z_SERVICE_DEFINITION",
            "/sap/bc/adt/ddic/srvd/sources/z_service_definition",
        ),
        (
            "DDLA/ADF",
            "Z_ANNOTATION_DEFINITION",
            "/sap/bc/adt/ddic/ddla/sources/z_annotation_definition",
        ),
        ("DOMA/DD", "Z_DOMAIN", "/sap/bc/adt/ddic/domains/z_domain"),
        (
            "FUGR/F",
            "Z_TEST_GROUP",
            "/sap/bc/adt/functions/groups/z_test_group",
        ),
    ] {
        let parsed_type: GlobalWorkbenchType = object_type.parse().unwrap();
        let object = client.repository_object(&parsed_type, name).unwrap();

        assert_eq!(object.object_type().as_str(), object_type);
        assert_eq!(object.uri().as_str(), expected_uri);
        assert!(match object_type {
            "PROG/P" => object.typed::<zadt::Program>().is_some(),
            "PROG/I" => object.typed::<zadt::Include>().is_some(),
            "CLAS/OC" => object.typed::<Class>().is_some(),
            "DEVC/K" => object.typed::<zadt::Package>().is_some(),
            "DTEL/DE" => object.typed::<DataElement>().is_some(),
            "DDLS/DF" => object.typed::<DataDefinition>().is_some(),
            "DCLS/DL" => object.typed::<AccessControl>().is_some(),
            "INTF/OI" => object.typed::<Interface>().is_some(),
            "DDLX/EX" => object.typed::<MetadataExtension>().is_some(),
            "SRVD/SRV" => object.typed::<ServiceDefinition>().is_some(),
            "DDLA/ADF" => object.typed::<AnnotationDefinition>().is_some(),
            "DOMA/DD" => object.typed::<Domain>().is_some(),
            "FUGR/F" => object.typed::<FunctionGroup>().is_some(),
            _ => unreachable!(),
        });
    }

    let child_type: GlobalWorkbenchType = "FUGR/FF".parse().unwrap();
    assert!(matches!(
        client.repository_object(&child_type, "ZZZZFUNC"),
        Err(ObjectError::ParentObjectRequired { object_type })
            if object_type.as_str() == "FUGR/FF"
    ));

    let unsupported_type: GlobalWorkbenchType = "ENQU/DL".parse().unwrap();
    assert!(matches!(
        client.repository_object(&unsupported_type, "EZABAPGIT"),
        Err(ObjectError::UnsupportedObjectType { object_type })
            if object_type.as_str() == "ENQU/DL"
    ));
}

#[tokio::test]
async fn function_groups_resolve_typed_and_runtime_subobjects() {
    let client = Client::new(FixtureTransport::new(DISCOVERY_XML));
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();

    let group = client.object::<FunctionGroup>("Z_TEST_GROUP").unwrap();
    let module = group.subobject::<FunctionModule>("ZZZZFUNC").unwrap();
    let include = group
        .subobject::<FunctionGroupInclude>("LZ_TEST_GROUPTOP")
        .unwrap();
    let namespaced = group.subobject::<FunctionModule>("/DMO/FUNCTION").unwrap();
    let runtime = group.erase();
    let runtime_module = runtime
        .subobject(&FunctionModule::WORKBENCH_TYPE, "ZZZZFUNC")
        .unwrap();
    let restored_module: ObjectRef<FunctionModule> =
        serde_json::from_value(serde_json::to_value(&module).unwrap()).unwrap();
    let detached: ObjectRef<FunctionGroup> =
        serde_json::from_value(serde_json::to_value(&group).unwrap()).unwrap();

    assert_eq!(
        module.uri().as_str(),
        "/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc"
    );
    assert_eq!(
        include.uri().as_str(),
        "/sap/bc/adt/functions/groups/z_test_group/includes/lz_test_grouptop"
    );
    assert_eq!(
        namespaced.uri().as_str(),
        "/sap/bc/adt/functions/groups/z_test_group/fmodules/%2Fdmo%2Ffunction"
    );
    assert_eq!(runtime_module, module.erase());
    assert!(runtime_module.typed::<FunctionModule>().is_some());
    let advertised: zadt::AdvertisedObjectReference = (&restored_module).into();
    assert_eq!(
        advertised.parent_uri.as_deref(),
        Some("/sap/bc/adt/functions/groups/z_test_group")
    );
    assert!(matches!(
        detached.subobject::<FunctionModule>("ZZZZFUNC"),
        Err(ObjectError::MissingTemplate { .. })
    ));
}

#[tokio::test]
async fn function_group_subobjects_require_the_advertised_template() {
    let client = Client::new(FixtureTransport::new(FUNCTIONS_WITHOUT_SUBOBJECT_TEMPLATES));
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();
    let group = client.object::<FunctionGroup>("Z_TEST_GROUP").unwrap();

    let error = group.subobject::<FunctionModule>("ZZZZFUNC").unwrap_err();

    assert!(matches!(
        error,
        ObjectError::MissingTemplate { relation }
            if relation
                == "http://www.sap.com/adt/categories/functiongroups/functionmodules"
    ));
}

#[tokio::test]
async fn function_group_subobjects_require_the_supported_template_shape() {
    let unsupported = r#"<app:service xmlns:app="http://www.w3.org/2007/app"
            xmlns:atom="http://www.w3.org/2005/Atom"
            xmlns:adtcomp="http://www.sap.com/adt/compatibility">
            <app:workspace>
                <atom:title>Function Builder</atom:title>
                <app:collection href="/sap/bc/adt/functions/groups">
                    <atom:category term="groups"
                        scheme="http://www.sap.com/adt/categories/functions" />
                    <adtcomp:templateLinks>
                        <adtcomp:templateLink
                            rel="http://www.sap.com/adt/categories/functiongroups/functionmodules"
                            template="/sap/bc/adt/functions/groups/{groupname}/fmodules/{module}" />
                    </adtcomp:templateLinks>
                </app:collection>
            </app:workspace>
        </app:service>"#;
    let client = Client::new(FixtureTransport::new(unsupported));
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();
    let group = client.object::<FunctionGroup>("Z_TEST_GROUP").unwrap();

    assert!(matches!(
        group.subobject::<FunctionModule>("ZZZZFUNC"),
        Err(ObjectError::MissingTemplate { .. })
    ));

    let malformed = unsupported.replacen(
        "/sap/bc/adt/functions/groups/{groupname}/fmodules/{module}\" />",
        "//other-host/{groupname}\" />",
        1,
    );
    let client = Client::new(FixtureTransport::new(malformed));
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();

    assert!(matches!(
        client.object::<FunctionGroup>("Z_TEST_GROUP"),
        Err(ObjectError::InvalidExpandedTarget { .. })
    ));
}

#[tokio::test]
async fn reqwest_transport_sends_the_discovery_contract() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    let discovery = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/discovery")
                .header("accept", "application/atomsvc+xml")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN")
                .header("authorization", "Basic VVNFUjpQQVNTV09SRA==");
            then.status(200)
                .header("content-type", "application/atomsvc+xml")
                .body(DISCOVERY_XML);
        })
        .await;
    let core_discovery = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/discovery")
                .header("accept", "application/atomsvc+xml")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN")
                .header("authorization", "Basic VVNFUjpQQVNTV09SRA==");
            then.status(200)
                .header("content-type", "application/atomsvc+xml")
                .body(CORE_DISCOVERY_XML);
        })
        .await;

    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();

    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();
    let client = client.discover().await.unwrap();

    logon.assert_async().await;
    discovery.assert_async().await;
    core_discovery.assert_async().await;
    assert!(
        client
            .capabilities()
            .collection(PROGRAMS_SCHEME, "programs")
            .is_some()
    );
}

#[tokio::test]
async fn reqwest_transport_reuses_security_session_cookies() {
    let server = MockServer::start_async().await;
    let logon = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/sap/bc/adt/core/http/sessions")
                .header("cookie", "sap-usercontext=sap-client=001&sap-language=EN");
            then.status(200)
                .header("set-cookie", "SAP_SESSIONID_A4H_001=session; Path=/")
                .header("content-type", SESSION_MEDIA_TYPE)
                .body(SESSION_XML);
        })
        .await;
    let central_discovery_user_context_first = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery").header(
                "cookie",
                "sap-usercontext=sap-client=001&sap-language=EN; SAP_SESSIONID_A4H_001=session",
            );
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let central_discovery_session_first = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery").header(
                "cookie",
                "SAP_SESSIONID_A4H_001=session; sap-usercontext=sap-client=001&sap-language=EN",
            );
            then.status(200).body(DISCOVERY_XML);
        })
        .await;
    let _core_discovery = server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/core/discovery");
            then.status(200).body(CORE_DISCOVERY_XML);
        })
        .await;
    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "PASSWORD")
        .build()
        .unwrap();
    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();

    client.discover().await.unwrap();

    logon.assert_async().await;
    assert_eq!(
        central_discovery_user_context_first.calls_async().await
            + central_discovery_session_first.calls_async().await,
        1
    );
}

#[tokio::test]
async fn unexpected_status_is_an_operation_response_error() {
    let server = MockServer::start_async().await;
    let logon = mock_logon(&server).await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/sap/bc/adt/discovery");
            then.status(401).body("authentication required");
        })
        .await;

    let transport = ReqwestTransport::builder()
        .destination(server.base_url())
        .sap_client("001")
        .language("EN")
        .basic_auth("USER", "WRONG")
        .build()
        .unwrap();

    let client = Client::new(transport);
    Logon::default().execute(&client).await.unwrap();
    let error = match client.discover().await {
        Ok(_) => panic!("discovery unexpectedly succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        OperationError::Response(ResponseError::UnexpectedStatus {
            status: StatusCode::UNAUTHORIZED,
            ..
        })
    ));
    logon.assert_calls_async(2).await;
}

#[tokio::test]
async fn discovery_defers_collection_url_validation_until_use() {
    let client = Client::new(FixtureTransport::new(INVALID_DISCOVERY_XML));
    Logon::default().execute(&client).await.unwrap();
    let capabilities = DiscoveryQuery.execute(&client).await.unwrap();
    let collection = capabilities
        .collection("http://www.sap.com/adt/categories/programs", "programs")
        .unwrap();

    assert!(collection.target().is_err());
}

struct FixtureTransport {
    response: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FixtureTransport {
    fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Transport for FixtureTransport {
    async fn send(&self, request: AdtRequest) -> Result<AdtResponse, TransportError> {
        self.requests
            .lock()
            .unwrap()
            .push(request.target().as_str().to_owned());
        if request.target().as_str() == "/sap/bc/adt/core/http/sessions" {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(SESSION_MEDIA_TYPE),
            );
            return Ok(AdtResponse::new(
                StatusCode::OK,
                headers,
                SESSION_XML.as_bytes().to_vec(),
            ));
        }
        let response = if request.target().as_str() == "/sap/bc/adt/core/discovery" {
            CORE_DISCOVERY_XML
        } else {
            self.response.as_str()
        };
        Ok(AdtResponse::new(
            StatusCode::OK,
            HeaderMap::new(),
            response.as_bytes().to_vec(),
        ))
    }
}
