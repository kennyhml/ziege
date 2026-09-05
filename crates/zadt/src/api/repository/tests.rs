use async_trait::async_trait;
use http::{HeaderMap, Method, StatusCode, header};

use super::{content::RepositoryContentRequest, *};
use crate::{
    AdtRequest, AdtResponse, AdtUri, Class, Client, CompatibilityError, DataElement, Discovery,
    EncodeError, Include, ObjectError, ObjectKey, ObjectRef, ObjectType, Operation, OperationError,
    OperationResponse, Package, Program, RepositoryError, ResolveError, Transport, TransportError,
};

const DISCOVERY_XML: &[u8] = include_bytes!("../../../tests/fixtures/discovery.xml");
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

fn discovered_client(xml: &[u8]) -> Client<Discovery> {
    Client::new(UnusedTransport).with_capabilities(
        crate::api::discovery::parse_capabilities(xml).unwrap(),
        crate::api::discovery::parse_capabilities(xml).unwrap(),
    )
}

fn repository_client() -> Client<Discovery> {
    let discovery = String::from_utf8(DISCOVERY_XML.to_vec()).unwrap().replace(
        "</app:service>",
        r#"
    <app:workspace>
        <atom:title>Additional Repository Information</atom:title>
        <app:collection href="/sap/bc/adt/repository/favorites/lists">
            <atom:title>Favorite Objects</atom:title>
            <atom:category term="objectFavorites"
                scheme="http://www.sap.com/adt/categories/repository/virtualfolders" />
        </app:collection>
        <app:collection href="/sap/bc/adt/repository/informationsystem/transportproperties/values">
            <atom:title>Transport Properties</atom:title>
            <atom:category term="transportProperties"
                scheme="http://www.sap.com/adt/categories/repository" />
        </app:collection>
    </app:workspace>
</app:service>"#,
    );
    discovered_client(discovery.as_bytes())
}

#[test]
fn repository_content_request_matches_the_ris_contract() {
    let client = repository_client();
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

    let request = query.encode(client.discovery()).unwrap();
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
        RepositoryContentRequest::MEDIA_TYPE
    );
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        RepositoryContent::MEDIA_TYPE
    );
    assert!(body.contains("xmlns:vfs=\"http://www.sap.com/adt/ris/virtualFolders\""));
    assert!(body.contains("objectSearchPattern=\"Z*\""));
    assert!(body.contains("<vfs:preselection facet=\"PACKAGE\">"));
    assert!(body.contains("<vfs:value>-UI5/STRU</vfs:value>"));
    assert!(body.contains("<vfs:facet>GROUP</vfs:facet>"));
}

#[test]
fn rejects_unknown_repository_content_fields() {
    let xml = std::str::from_utf8(CONTENT_XML).unwrap();
    let base = AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders").unwrap();
    for tag in [
        "vfs:virtualFoldersResult",
        "vfs:virtualFolder",
        "vfs:object",
    ] {
        for (from, to) in [
            (format!("<{tag} "), format!("<{tag} unexpected=\"true\" ")),
            (format!("</{tag}>"), format!("<unexpected/></{tag}>")),
        ] {
            let body = xml.replacen(&from, &to, 1);
            let error = RepositoryContent::parse(body.as_bytes(), &base)
                .unwrap_err()
                .to_string();
            assert!(error.contains("unknown field"), "{tag}: {error}");
            assert!(error.contains("unexpected"), "{tag}: {error}");
        }
    }
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
    let client = repository_client();
    let user = crate::User::new("DEVELOPER");
    let query = user.favorites();
    let request = query.encode(client.discovery()).unwrap();
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        FavoriteObjectList::MEDIA_TYPE
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        FavoriteObjectList::MEDIA_TYPE.parse().unwrap(),
    );
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
    let client = repository_client();
    let object = ObjectKey::<Program>::new("Z_TEST").erase();
    let mut update = FavoriteObjectsUpdate::new("TEAM");
    update.add(&object).remove(&object);

    let request = update.encode(client.discovery()).unwrap();
    let body = std::str::from_utf8(request.body()).unwrap();

    assert_eq!(request.method(), Method::POST);
    assert_eq!(
        request.headers().get(header::ACCEPT).unwrap(),
        FavoriteObjectList::MEDIA_TYPE
    );
    assert_eq!(
        request.headers().get(header::CONTENT_TYPE).unwrap(),
        FavoriteObjectList::UPDATE_MEDIA_TYPE
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
fn located_parentless_ris_children_preserve_their_uri_in_secondary_operations() {
    let client = repository_client();
    let base = AdtUri::parse("/sap/bc/adt/repository/contents").unwrap();
    let uri = "/sap/bc/adt/custom/children/42";
    let xml = std::str::from_utf8(CONTENT_XML)
        .unwrap()
        .replace("type=\"CLAS/OC\"", "type=\"FUGR/FF\"")
        .replace("/sap/bc/adt/oo/classes/zcl_demo", uri);
    let content = RepositoryContent::parse(xml.as_bytes(), &base).unwrap();
    let entry = &content.objects[0];
    let object = entry.object();
    let typed = ObjectRef::<crate::FunctionModule>::try_from(entry).unwrap();
    assert_eq!(object.uri().as_str(), uri);
    assert_eq!(entry.uri(), object.uri());
    assert_eq!(typed.uri(), object.uri());
    assert!(typed.key().parent().is_none());
    assert!(object.key().parent().is_none());
    assert!(object.parent_uri().is_none());
    assert_eq!(typed.erase(), object);

    for request in [
        RepositoryObjectPropertiesQuery::from_ref(&typed)
            .encode(client.discovery())
            .unwrap(),
        RepositoryObjectPropertiesQuery::from_ref(&object)
            .encode(client.discovery())
            .unwrap(),
        entry.properties().encode(client.discovery()).unwrap(),
        typed
            .transport_requests()
            .encode(client.discovery())
            .unwrap(),
        object
            .transport_requests()
            .encode(client.discovery())
            .unwrap(),
    ] {
        assert_eq!(request.query(), [("uri".to_owned(), uri.to_owned())]);
    }
    let mut update = FavoriteObjectsUpdate::new("TEAM");
    update
        .add_ref(&typed)
        .remove_ref(&typed)
        .add_ref(&object)
        .remove_ref(&object);
    let request = update.encode(client.discovery()).unwrap();
    let body = std::str::from_utf8(request.body()).unwrap();
    assert_eq!(body.matches(&format!("adtcore:uri=\"{uri}\"")).count(), 4);
    assert!(body.contains("operation=\"A\""));
    assert!(body.contains("operation=\"R\""));
}

#[test]
fn ris_property_summary_preserves_the_advertised_object_location() {
    let base = AdtUri::parse("/sap/bc/adt/repository/objectproperties").unwrap();
    let uri = "/sap/bc/adt/custom/objects/42";
    let xml = std::str::from_utf8(OBJECT_PROPERTIES_XML)
        .unwrap()
        .replace("/sap/bc/adt/oo/classes/cl_adt_uri_mapper", uri);
    let properties = RepositoryObjectProperties::parse(xml.as_bytes(), &base).unwrap();
    assert_eq!(properties.object.reference.uri().as_str(), uri);
    assert_eq!(
        properties
            .object
            .reference
            .typed::<Class>()
            .unwrap()
            .uri()
            .as_str(),
        uri
    );
}

#[test]
fn object_properties_request_repeats_included_facets() {
    let client = repository_client();
    let object = ObjectKey::<Program>::new("Z_TEST");
    assert_eq!(
        client
            .discovery()
            .resolve_object_uri(&object)
            .unwrap()
            .as_str(),
        "/sap/bc/adt/programs/programs/z_test"
    );
    let query = RepositoryObjectPropertiesQuery::new(&object)
        .include_facet(RepositoryFacet::PACKAGE)
        .include_facet(RepositoryFacet::GROUP);

    let request = query.encode(client.discovery()).unwrap();

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
        RepositoryObjectProperties::MEDIA_TYPE
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
    assert_eq!(properties.object.reference.name(), "CL_ADT_URI_MAPPER");
    assert_eq!(properties.properties.len(), 3);
}

#[test]
fn object_properties_use_the_native_adt_identity_relation() {
    let query = RepositoryObjectPropertiesQuery::for_uri(
        AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
    );
    let response = AdtResponse::new(
        StatusCode::OK,
        HeaderMap::new(),
        br#"<opr:objectProperties xmlns:opr="http://www.sap.com/adt/ris/objectProperties">
            <opr:object name="CL_ADT_URI_MAPPER" text="URI Mapper" package="SADT_TOOLS_CORE"
                    type="CLAS/OC" expandable="true">
                <atom:link href="/sap/bc/adt/vit/wb/object_type/clasoc/object_name/CL_ADT_URI_MAPPER"
                    rel="http://www.sap.com/adt/relations/objects"
                    type="application/vnd.sap.sapgui" xmlns:atom="http://www.w3.org/2005/Atom" />
                <atom:link href="/sap/bc/adt/oo/classes/cl_adt_uri_mapper"
                    rel="http://www.sap.com/adt/relations/objects"
                    xmlns:atom="http://www.w3.org/2005/Atom" />
            </opr:object>
        </opr:objectProperties>"#
            .to_vec(),
    );

    let properties = query
        .decode(OperationResponse::new(
            response,
            AdtUri::parse("/sap/bc/adt/repository/objectproperties").unwrap(),
        ))
        .unwrap();

    assert_eq!(properties.object.reference.name(), "CL_ADT_URI_MAPPER");
}

#[test]
fn assigned_transports_request_and_response_match_the_ris_contract() {
    let client = repository_client();
    let object = ObjectKey::<Program>::new("Z_TEST");
    let query = object.transport_requests();
    let request = query.encode(client.discovery()).unwrap();

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
        AssignedTransportRequests::MEDIA_TYPE
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        AssignedTransportRequests::MEDIA_TYPE.parse().unwrap(),
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
    let client = repository_client();
    let request = RepositoryFacetsQuery.encode(client.discovery()).unwrap();

    assert_eq!(request.method(), Method::GET);
    assert_eq!(
        request.target().as_str(),
        "/sap/bc/adt/repository/informationsystem/virtualfolders/facets"
    );
}

#[tokio::test]
async fn repository_request_requires_its_discovery_collection() {
    let client = discovered_client(
        br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom">
                    <app:workspace><atom:title>Repository</atom:title></app:workspace>
                </app:service>"#,
    );

    let error = RepositoryContentQuery::new()
        .execute(&client)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        OperationError::Encode(EncodeError::Resolve(ResolveError::Compatibility(
            CompatibilityError::MissingCollection(category)
        )))
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
        content.objects[0].uri().as_str(),
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
        content.objects[0].uri().clone()
    );
}

#[test]
fn converts_ris_entries_to_checked_typed_references() {
    let base =
        AdtUri::parse("/sap/bc/adt/repository/informationsystem/virtualfolders/contents").unwrap();
    let content = RepositoryContent::parse(CONTENT_XML, &base).unwrap();
    let entry = &content.objects[0];

    let class = entry.typed_reference::<Class>().unwrap();
    assert_eq!(class.name(), "ZCL_DEMO");
    assert_eq!(class.object_type(), entry.reference.object_type());

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
    assert_eq!(object.uri(), entry.uri());
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
    assert_eq!(properties.object.relations().len(), 1);
    assert_eq!(properties.properties[0].facet, RepositoryFacet::PACKAGE);
    let package_hierarchy = properties.package_hierarchy();
    let package = &package_hierarchy[0];
    assert_eq!(package.name(), "SADT_TOOLS_CORE");
    assert_eq!(properties.properties[0].value, "SADT_TOOLS_CORE");
    assert_eq!(properties.properties[0].relations().len(), 1);
    assert_eq!(properties.properties[2].facet.as_str(), "FUTURE");
}

#[test]
fn returns_the_complete_package_hierarchy_in_response_order() {
    let object_uri = AdtUri::parse("/sap/bc/adt/oo/classes/cl_ris_adt_res_obj_properties").unwrap();
    let properties =
        RepositoryObjectProperties::parse(OBJECT_PROPERTIES_HIERARCHY_XML, &object_uri).unwrap();

    let hierarchy = properties.package_hierarchy();

    assert_eq!(
        hierarchy.iter().map(ObjectKey::name).collect::<Vec<_>>(),
        ["BASIS", "SRIS", "SRIS_ADT"]
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
