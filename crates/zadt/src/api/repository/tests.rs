use async_trait::async_trait;
use http::{HeaderMap, Method, StatusCode, header};

use super::{
    content::{
        REPOSITORY_CONTENT_REQUEST_MEDIA_TYPE, REPOSITORY_CONTENT_RESULT_MEDIA_TYPE,
        RepositoryContentRequest,
    },
    favorites::{FAVORITES_MEDIA_TYPE, FAVORITES_UPDATE_MEDIA_TYPE},
    object_properties::{
        OBJECT_PROPERTIES_MEDIA_TYPE, PACKAGE_RELATION, TRANSPORT_PROPERTIES_MEDIA_TYPE,
    },
    *,
};
use crate::{
    AccessMode, AdtRequest, AdtResponse, AdtUri, Advertised, Class, Client, CompatibilityError,
    DataElement, EncodedOperation, Include, ObjectError, ObjectRef, ObjectType, Operation,
    OperationError, OperationResponse, Package, Program, Ready, RepositoryError, ResolveError,
    Transport, TransportError,
};

const CONTENT_XML: &[u8] = include_bytes!("../../../tests/fixtures/repository-content.xml");
const FACETS_XML: &[u8] = include_bytes!("../../../tests/fixtures/repository-facets.xml");
const OBJECT_PROPERTIES_XML: &[u8] =
    include_bytes!("../../../tests/fixtures/repository-object-properties.xml");
const OBJECT_PROPERTIES_HIERARCHY_XML: &[u8] =
    include_bytes!("../../../tests/fixtures/repository-object-properties-hierarchy.xml");

struct UnusedTransport;

#[async_trait]
impl Transport for UnusedTransport {
    async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
        unreachable!("request construction tests do not send requests")
    }
}

fn ready_client(xml: &[u8]) -> Client<Ready> {
    Client::new(UnusedTransport).with_capabilities(
        crate::api::discovery::parse_capabilities(xml).unwrap(),
        crate::api::discovery::parse_capabilities(xml).unwrap(),
    )
}

#[test]
fn repository_content_request_matches_the_ris_contract() {
    let query = RepositoryContentQuery::builder()
        .search_pattern("Z*")
        .preselection(
            RepositoryPreselection::new(RepositoryFacet::PACKAGE, "$TMP").exclude("UI5/STRU"),
        )
        .facet(RepositoryFacet::GROUP)
        .operation(RepositoryContentOperation::Expand)
        .ignore_short_descriptions(true)
        .with_versions(false)
        .build()
        .unwrap();

    let request = query.encode().unwrap();
    let body = std::str::from_utf8(request.body()).unwrap();

    assert_eq!(request.method(), Method::POST);
    assert_eq!(
        request.query(),
        [
            ("ignoreShortDescriptions".to_owned(), "true".to_owned()),
            ("withVersions".to_owned(), "false".to_owned()),
            ("operation".to_owned(), "expand".to_owned()),
        ]
    );
    assert_eq!(
        request.headers().get(header::CONTENT_TYPE).unwrap(),
        REPOSITORY_CONTENT_REQUEST_MEDIA_TYPE
    );
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        REPOSITORY_CONTENT_RESULT_MEDIA_TYPE
    );
    assert!(body.contains("xmlns:vfs=\"http://www.sap.com/adt/ris/virtualFolders\""));
    assert!(body.contains("objectSearchPattern=\"Z*\""));
    assert!(body.contains("<vfs:preselection facet=\"PACKAGE\">"));
    assert!(body.contains("<vfs:value>-UI5/STRU</vfs:value>"));
    assert!(body.contains("<vfs:facet>GROUP</vfs:facet>"));
}

#[test]
fn repository_content_response_decodes_one_layer() {
    let query = RepositoryContentQuery::new();
    let response = AdtResponse::new(StatusCode::OK, HeaderMap::new(), CONTENT_XML.to_vec());
    let request_target = AdtUri::parse("/sap/bc/adt/advertised/repository/contents").unwrap();

    let content = <RepositoryContentQuery as Operation>::decode(
        &query,
        OperationResponse::new(response, request_target),
    )
    .unwrap();

    assert_eq!(content.object_count, 3);
    assert_eq!(content.folders.len(), 1);
    assert_eq!(content.objects.len(), 1);
    assert_eq!(
        content.objects[0].reference.object_type().as_str(),
        "CLAS/OC"
    );
    assert_eq!(content.objects[0].relations().len(), 1);
}

#[test]
fn favorite_objects_response_decodes_objects() {
    let user = crate::User::new("DEVELOPER");
    let query = user.favorites();
    let request = query.encode().unwrap();
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        FAVORITES_MEDIA_TYPE
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, FAVORITES_MEDIA_TYPE.parse().unwrap());
    let response = AdtResponse::new(
        StatusCode::OK,
        headers,
        br#"<vf:favorites
                xmlns:vf="http://www.sap.com/adt/ris/vf/favorites"
                xmlns:adtcore="http://www.sap.com/adt/core">
            <vf:favorite
                adtcore:uri="/sap/bc/adt/programs/programs/z_test"
                adtcore:type="PROG/P"
                adtcore:name="Z_TEST"
                listId="$" />
        </vf:favorites>"#
            .to_vec(),
    );
    let favorites = <FavoriteObjectsQuery as Operation>::decode(
        &query,
        OperationResponse::new(
            response,
            AdtUri::parse("/sap/bc/adt/repository/favorites/lists/$DEVELOPER").unwrap(),
        ),
    )
    .unwrap();

    assert_eq!(favorites.objects.len(), 1);
    assert_eq!(
        favorites.objects[0].uri,
        "/sap/bc/adt/programs/programs/z_test"
    );
    assert_eq!(favorites.objects[0].object_type.as_str(), "PROG/P");
    assert_eq!(favorites.objects[0].name, "Z_TEST");
    assert_eq!(favorites.objects[0].list.as_deref(), Some("$"));
}

#[test]
fn favorite_objects_update_serializes_transactions() {
    let object = ObjectRef::<Program>::for_test(
        "Z_TEST",
        AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
    )
    .erase();
    let mut update = FavoriteObjectsUpdate::new("TEAM");
    update.add(&object).remove(&object);

    let request = update.encode().unwrap();
    let body = std::str::from_utf8(request.body()).unwrap();

    assert_eq!(request.method(), Method::POST);
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        FAVORITES_MEDIA_TYPE
    );
    assert_eq!(
        request.headers().get(header::CONTENT_TYPE).unwrap(),
        FAVORITES_UPDATE_MEDIA_TYPE
    );
    assert!(body.contains("xmlns:vf=\"http://www.sap.com/adt/ris/vf/favorites\""));
    assert!(body.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
    assert!(body.contains("adtcore:uri=\"/sap/bc/adt/programs/programs/z_test\""));
    assert!(body.contains("adtcore:type=\"PROG/P\""));
    assert!(body.contains("adtcore:name=\"Z_TEST\""));
    assert!(body.contains("listId=\"TEAM\""));
    assert!(body.contains("operation=\"A\""));
    assert!(body.contains("operation=\"R\""));
}

#[test]
fn object_properties_request_repeats_included_facets() {
    let object = ObjectRef::<Program>::for_test(
        "Z_TEST",
        AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
    );
    let query = RepositoryObjectPropertiesQuery::new(&object)
        .include_facet(RepositoryFacet::PACKAGE)
        .include_facet(RepositoryFacet::GROUP);

    let request = query.encode().unwrap();

    assert_eq!(request.method(), Method::GET);
    assert_eq!(
        request.query(),
        [
            (
                "uri".to_owned(),
                "/sap/bc/adt/programs/programs/z_test".to_owned(),
            ),
            ("facet".to_owned(), "PACKAGE".to_owned()),
            ("facet".to_owned(), "GROUP".to_owned()),
        ]
    );
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        OBJECT_PROPERTIES_MEDIA_TYPE
    );

    let response = AdtResponse::new(
        StatusCode::OK,
        HeaderMap::new(),
        OBJECT_PROPERTIES_XML.to_vec(),
    );
    let properties = <RepositoryObjectPropertiesQuery as Operation>::decode(
        &query,
        OperationResponse::new(
            response,
            AdtUri::parse("/sap/bc/adt/advertised/repository/object-properties").unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(properties.object.reference.uri(), object.uri());
    assert_eq!(properties.properties.len(), 3);
}

#[test]
fn assigned_transports_request_and_response_match_the_ris_contract() {
    let object = ObjectRef::<Program>::for_test(
        "Z_TEST",
        AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
    );
    let query = object.transport_requests();
    let request = query.encode().unwrap();

    assert_eq!(request.method(), Method::GET);
    assert_eq!(
        request.query(),
        [(
            "uri".to_owned(),
            "/sap/bc/adt/programs/programs/z_test".to_owned()
        )]
    );
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        TRANSPORT_PROPERTIES_MEDIA_TYPE
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        TRANSPORT_PROPERTIES_MEDIA_TYPE.parse().unwrap(),
    );
    let response = AdtResponse::new(
        StatusCode::OK,
        headers,
        br#"<tpr:transportProperties xmlns:tpr="http://www.sap.com/adt/ris/transportProperties">
            <tpr:transport number="DEVK900001" owner="DEVELOPER" status="D"
                description="Workbench request" />
        </tpr:transportProperties>"#
            .to_vec(),
    );
    let transports = <AssignedTransportsQuery as Operation>::decode(
        &query,
        OperationResponse::new(
            response,
            AdtUri::parse("/sap/bc/adt/advertised/repository/transport-properties").unwrap(),
        ),
    )
    .unwrap();

    assert_eq!(transports.len(), 1);
    assert_eq!(transports.requests[0].number.as_str(), "DEVK900001");
    assert_eq!(
        transports.requests[0].status,
        crate::TransportStatus::MODIFIABLE
    );
    assert_eq!(transports.requests[0].owner.as_str(), "DEVELOPER");
}

#[test]
fn facets_request_uses_the_advertised_collection_target() {
    let request: EncodedOperation<Advertised> = RepositoryFacetsQuery.encode().unwrap();

    assert_eq!(request.method(), Method::GET);
}

#[tokio::test]
async fn repository_request_requires_its_discovery_collection() {
    let client = ready_client(
        br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom">
                    <app:workspace><atom:title>Repository</atom:title></app:workspace>
                </app:service>"#,
    );

    let error = client
        .repository_content()
        .execute(&client)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        OperationError::Resolve(ResolveError::Compatibility(
            CompatibilityError::MissingCollection(category)
        ))
            if category.scheme
                == "http://www.sap.com/adt/categories/repository/virtualfolders"
                && category.term == "contents"
    ));
}

#[test]
fn serializes_virtual_folder_filters() {
    let selections = [
        RepositoryPreselection::new(RepositoryFacet::OWNER, "DEVELOPER").include("JOHN DOE"),
        RepositoryPreselection::new(RepositoryFacet::PACKAGE, "$TMP").exclude("UI5/STRU"),
    ];
    let facets = [RepositoryFacet::GROUP, RepositoryFacet::TYPE];

    let xml = RepositoryContentRequest::new("*", &selections, &facets)
        .serialize()
        .unwrap();

    assert!(xml.contains("objectSearchPattern=\"*\""));
    assert!(xml.contains("<vfs:value>JOHN DOE</vfs:value>"));
    assert!(xml.contains("<vfs:value>-UI5/STRU</vfs:value>"));
    assert!(xml.contains("<vfs:facet>GROUP</vfs:facet>"));
    assert!(xml.contains("<vfs:facet>TYPE</vfs:facet>"));
}

#[test]
fn creates_direct_package_preselections() {
    for package in ["/DMO/FLIGHT", "ZPACKAGE", "$TMP", "../DMO/FLIGHT"] {
        let selection = RepositoryPreselection::directly_assigned(package);

        assert_eq!(selection.facet(), &RepositoryFacet::PACKAGE);
        assert_eq!(
            selection.values(),
            [format!("..{}", package.trim_start_matches(".."))]
        );
    }
}

#[test]
fn parses_virtual_folders_and_repository_objects() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    let content = RepositoryContent::parse(CONTENT_XML, &base).unwrap();

    assert_eq!(content.object_count, 3);
    assert_eq!(
        content.preselection_info.unwrap().facet,
        RepositoryFacet::PACKAGE
    );
    assert_eq!(content.folders[0].name, "SOURCE_LIBRARY");
    assert_eq!(content.folders[0].uri, None);
    assert!(!content.folders[0].is_direct_assignment());
    assert_eq!(content.folders[0].relations().len(), 1);
    assert_eq!(
        content.folders[0].as_preselection().values(),
        ["SOURCE_LIBRARY"]
    );
    assert_eq!(content.objects[0].name, "ZCL_DEMO");
    assert_eq!(
        content.objects[0].reference.object_type().as_str(),
        "CLAS/OC"
    );
    assert_eq!(
        content.objects[0].reference.uri().as_str(),
        "/sap/bc/adt/oo/classes/zcl_demo"
    );
    assert_eq!(
        content.objects[0]
            .relations()
            .iter()
            .next()
            .unwrap()
            .unwrap()
            .target,
        *content.objects[0].reference.uri()
    );
}

#[test]
fn converts_ris_entries_to_checked_typed_references_without_changing_the_uri() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();
    let content = RepositoryContent::parse(CONTENT_XML, &base).unwrap();
    let entry = &content.objects[0];

    let class = entry.typed_reference::<Class>().unwrap();
    assert_eq!(class.name(), "ZCL_DEMO");
    assert_eq!(class.uri(), entry.reference.uri());

    let error = ObjectRef::<Program>::try_from(entry).unwrap_err();
    assert!(matches!(
        error,
        ObjectError::UnexpectedRepositoryObjectType { expected, actual }
            if expected == Program::WORKBENCH_TYPE && actual.as_str() == "CLAS/OC"
    ));
}

#[test]
fn converts_ris_entries_to_runtime_repository_objects() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    for object_type in ["PROG/P", "PROG/I", "CLAS/OC", "DEVC/K", "DTEL/DE"] {
        let xml = String::from_utf8(CONTENT_XML.to_vec())
            .unwrap()
            .replace("type=\"CLAS/OC\"", &format!("type=\"{object_type}\""));
        let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();
        let object = content.objects[0].repository_object();

        assert_eq!(object.object_type().as_str(), object_type);
        assert!(match object_type {
            "PROG/P" => object.typed::<Program>().is_some(),
            "PROG/I" => object.typed::<Include>().is_some(),
            "CLAS/OC" => object.typed::<Class>().is_some(),
            "DEVC/K" => object.typed::<Package>().is_some(),
            "DTEL/DE" => object.typed::<DataElement>().is_some(),
            _ => unreachable!(),
        });
    }

    let unknown_xml = String::from_utf8(CONTENT_XML.to_vec())
        .unwrap()
        .replace("type=\"CLAS/OC\"", "type=\"DDLS/DF\"");
    let content = RepositoryContent::parse(unknown_xml.as_bytes(), &base).unwrap();
    let object = content.objects[0].repository_object();

    assert_eq!(object.object_type().as_str(), "DDLS/DF");
    assert!(object.typed::<Class>().is_none());
}

#[test]
fn runtime_repository_objects_preserve_the_ris_identity() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();
    let content = RepositoryContent::parse(CONTENT_XML, &base).unwrap();
    let entry = &content.objects[0];
    let object = entry.repository_object();

    assert_eq!(object, entry.reference);
    assert_eq!(object.typed::<Class>().unwrap().name(), entry.name);
    assert_eq!(object.lock(AccessMode::Modify).object, entry.reference);
}

#[test]
fn parses_virtual_folder_resource_uris() {
    let xml = String::from_utf8(CONTENT_XML.to_vec()).unwrap().replace(
        "<vfs:virtualFolder name=\"SOURCE_LIBRARY\"",
        "<vfs:virtualFolder name=\"SOURCE_LIBRARY\" uri=\"/sap/bc/adt/packages/%2ftmp\"",
    );
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();

    assert_eq!(
        content.folders[0].uri.as_ref().unwrap().as_str(),
        "/sap/bc/adt/packages/%2ftmp"
    );
}

#[test]
fn rejects_virtual_folder_uris_outside_the_sap_namespace() {
    let xml = String::from_utf8(CONTENT_XML.to_vec()).unwrap().replace(
        "<vfs:virtualFolder name=\"SOURCE_LIBRARY\"",
        "<vfs:virtualFolder name=\"SOURCE_LIBRARY\" uri=\"https://attacker.example/package\"",
    );
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    let error = RepositoryContent::parse(xml.as_bytes(), &base).unwrap_err();

    assert!(matches!(error, RepositoryError::InvalidFolderUri { .. }));
}

#[test]
fn identifies_direct_package_assignment_folders() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    for package in ["/DMO/FLIGHT", "ZPACKAGE", "$TMP"] {
        let xml = String::from_utf8(CONTENT_XML.to_vec())
            .unwrap()
            .replace("name=\"SOURCE_LIBRARY\"", &format!("name=\"..{package}\""))
            .replace("facet=\"GROUP\"", "facet=\"PACKAGE\"");
        let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();

        assert!(content.folders[0].is_direct_assignment());
        assert_eq!(
            content.folders[0].direct_assignment_package(),
            Some(package)
        );
    }
}

#[test]
fn does_not_treat_other_facets_as_direct_package_assignments() {
    let xml = String::from_utf8(CONTENT_XML.to_vec())
        .unwrap()
        .replace("name=\"SOURCE_LIBRARY\"", "name=\"..SOURCE_LIBRARY\"");
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();

    assert!(!content.folders[0].is_direct_assignment());
    assert_eq!(content.folders[0].direct_assignment_package(), None);
}

#[test]
fn preserves_custom_facets_and_repository_types() {
    let xml = String::from_utf8(CONTENT_XML.to_vec())
        .unwrap()
        .replace("facet=\"GROUP\"", "facet=\"FUTURE\"")
        .replace("type=\"CLAS/OC\"", "type=\"ZZZZ/X\"");
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();

    assert_eq!(content.folders[0].facet.as_str(), "FUTURE");
    assert_eq!(
        content.objects[0].reference.object_type().as_str(),
        "ZZZZ/X"
    );
}

#[test]
fn accepts_unmodeled_global_workbench_type_shapes() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    for object_type in ["AUTH", "CLAS/OCN/definitions", "clas/oc"] {
        let xml = String::from_utf8(CONTENT_XML.to_vec())
            .unwrap()
            .replace("type=\"CLAS/OC\"", &format!("type=\"{object_type}\""));
        let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();

        assert_eq!(
            content.objects[0].reference.object_type().as_str(),
            object_type
        );
        assert!(content.objects[0].typed_reference::<Class>().is_err());
        let object = content.objects[0].repository_object();
        assert_eq!(object.object_type().as_str(), object_type);
    }
}

#[test]
fn rejects_repository_object_uris_outside_the_sap_namespace() {
    let xml = String::from_utf8(CONTENT_XML.to_vec()).unwrap().replace(
        "/sap/bc/adt/oo/classes/zcl_demo",
        "https://attacker.example/sap/bc/adt/oo/classes/zcl_demo",
    );
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();

    let error = RepositoryContent::parse(xml.as_bytes(), &base).unwrap_err();

    assert!(matches!(error, RepositoryError::InvalidObjectUri { .. }));
}

#[test]
fn parses_available_facets_and_value_templates() {
    let facets = RepositoryFacets::parse(FACETS_XML).unwrap();

    assert_eq!(facets.facets.len(), 2);
    assert_eq!(facets.facets[0].key, "appl");
    assert_eq!(
        facets.facets[0].facet(),
        RepositoryFacet::APPLICATION_COMPONENT
    );
    assert!(facets.facets[0].is_hierarchical);
    assert_eq!(
        facets.facets[0].values.as_ref().unwrap().template,
        "/sap/bc/adt/repository/informationsystem/properties/values?data=appl{&name}"
    );
    assert!(facets.facets[1].values.is_none());
}

#[test]
fn parses_uniform_object_properties() {
    let object_uri = AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap();

    let properties = RepositoryObjectProperties::parse(OBJECT_PROPERTIES_XML, &object_uri).unwrap();

    assert_eq!(properties.object.name, "CL_ADT_URI_MAPPER");
    assert_eq!(properties.object.object_type.to_string(), "CLAS/OC");
    assert_eq!(properties.object.reference.uri(), &object_uri);
    assert_eq!(properties.object.relations().len(), 1);
    assert_eq!(properties.properties[0].facet, RepositoryFacet::PACKAGE);
    let package_hierarchy = properties.package_hierarchy().unwrap();
    let package = &package_hierarchy[0];
    assert_eq!(package.name(), "SADT_TOOLS_CORE");
    assert_eq!(
        package.uri().as_str(),
        "/sap/bc/adt/packages/sadt_tools_core"
    );
    assert_eq!(properties.properties[0].value, "SADT_TOOLS_CORE");
    assert_eq!(properties.properties[0].relations().len(), 1);
    assert_eq!(properties.properties[2].facet.as_str(), "FUTURE");
}

#[test]
fn returns_the_complete_package_hierarchy_in_response_order() {
    let object_uri = AdtUri::parse("/sap/bc/adt/oo/classes/cl_ris_adt_res_obj_properties").unwrap();
    let properties =
        RepositoryObjectProperties::parse(OBJECT_PROPERTIES_HIERARCHY_XML, &object_uri).unwrap();

    let hierarchy = properties.package_hierarchy().unwrap();

    assert_eq!(
        hierarchy.iter().map(ObjectRef::name).collect::<Vec<_>>(),
        ["BASIS", "SRIS", "SRIS_ADT"]
    );
    assert_eq!(hierarchy[0].uri().as_str(), "/sap/bc/adt/packages/basis");
    assert_eq!(
        hierarchy.last().unwrap().uri().as_str(),
        "/sap/bc/adt/packages/sris_adt"
    );
    assert_eq!(
        properties
            .properties
            .iter()
            .filter(|property| property.facet == RepositoryFacet::APPLICATION_COMPONENT)
            .map(|property| property.value.as_str())
            .collect::<Vec<_>>(),
        ["BC", "BC-DWB", "BC-DWB-AIE"]
    );
}

#[test]
fn package_hierarchy_requires_a_relation_for_every_level() {
    let xml = String::from_utf8(OBJECT_PROPERTIES_HIERARCHY_XML.to_vec())
        .unwrap()
        .replacen(PACKAGE_RELATION, "urn:unexpected", 1);
    let object_uri = AdtUri::parse("/sap/bc/adt/oo/classes/cl_ris_adt_res_obj_properties").unwrap();
    let properties = RepositoryObjectProperties::parse(xml.as_bytes(), &object_uri).unwrap();

    let error = properties.package_hierarchy().unwrap_err();

    assert!(matches!(
        error,
        ObjectError::MissingRelation {
            relation: PACKAGE_RELATION
        }
    ));
}
