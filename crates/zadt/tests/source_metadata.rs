use http::{HeaderMap, StatusCode, header};
use zadt::{
    AccessControl, AdtResponse, AdtUri, AdvertisedLink, AnnotationDefinition, Class,
    ClassProperties, DataDefinition, EntityTag, FunctionGroup, FunctionGroupInclude,
    FunctionModule, Identity, Include, Interface, InterfaceProperties, MetadataExtension,
    ObjectError, ObjectKey, ObjectSnapshot, ObjectType, Operation, OperationResponse, Program,
    ServiceDefinition, Source, ToXml, XmlCodec,
};

const CLASS_XML: &[u8] = include_bytes!("fixtures/class-cl-adt-uri-mapper-v4.xml");
const CLASS_URI: &str = "/sap/bc/adt/oo/classes/cl_adt_uri_mapper";
const SOURCE_RELATION: &str = "http://www.sap.com/adt/relations/source";

fn response(body: &[u8], uri: &str, media_type: &str, etag: Option<&str>) -> OperationResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, media_type.parse().unwrap());
    if let Some(etag) = etag {
        headers.insert(header::ETAG, etag.parse().unwrap());
    }
    OperationResponse::new(
        AdtResponse::new(StatusCode::OK, headers, body.to_vec()),
        AdtUri::parse(uri).unwrap(),
    )
}

fn snapshots<T: ObjectType>(xml: &[u8], uri: &str) -> (ObjectSnapshot<T>, ObjectSnapshot<()>) {
    let properties = T::Properties::from_xml(xml).unwrap();
    let key: ObjectKey<T> = serde_json::from_value(serde_json::json!({
        "name": properties.object_name(),
        "object_type": T::WORKBENCH_TYPE,
    }))
    .unwrap();
    let response = || response(xml, uri, T::MEDIA_TYPES[0], Some("object-etag"));
    (
        key.query().decode(response()).unwrap(),
        key.erase().query().decode(response()).unwrap(),
    )
}

fn assert_fixture<T: Source>(xml: &[u8], uri: &str, etag: &str) {
    let (typed, erased) = snapshots::<T>(xml, uri);
    let source = typed.source().unwrap();
    assert_eq!(source, erased.source().unwrap());
    assert_eq!(source.object, typed.reference().erase());
    assert_eq!(source.uri.as_str(), format!("{uri}/source/main"));
    assert!(source.query.is_empty());
    assert_eq!(source.fragment, None);
    assert_eq!(source.etag.as_deref(), Some(etag), "{uri}");
    assert_eq!(typed.etag().map(EntityTag::as_str), Some("object-etag"));
    assert_eq!(erased.etag(), typed.etag());
    if T::WORKBENCH_TYPE != Class::WORKBENCH_TYPE {
        // Flat properties supply metadata for main, not a component-level copy
        // of all their root links (structure, parser, unknown relations, etc.).
        assert_eq!(
            typed
                .resources()
                .iter()
                .filter(|resource| resource.component() == Some("main"))
                .count(),
            1,
            "{uri}"
        );
    }
}

fn source_link(href: &str, media_type: Option<&str>, etag: Option<&str>) -> AdvertisedLink {
    AdvertisedLink {
        href: href.to_owned(),
        relation: Some(SOURCE_RELATION.to_owned()),
        media_type: media_type.map(str::to_owned),
        hreflang: None,
        title: None,
        length: None,
        etag: etag.map(str::to_owned),
    }
}

#[test]
fn all_source_families_propagate_fixture_etags_with_typed_and_erased_parity() {
    assert_fixture::<Class>(CLASS_XML, CLASS_URI, "20210406145501001000181");
    assert_fixture::<Interface>(
        include_bytes!("fixtures/interface-if-adt-uri-mapper-v5.xml"),
        "/sap/bc/adt/oo/interfaces/if_adt_uri_mapper",
        "201701161858400011",
    );
    assert_fixture::<Program>(
        include_bytes!("fixtures/program-z-test.xml"),
        "/sap/bc/adt/programs/programs/z_test",
        "202607251959580001",
    );
    assert_fixture::<Include>(
        include_bytes!("fixtures/include-ztest.xml"),
        "/sap/bc/adt/programs/includes/ztest",
        "202601241617490011",
    );
    assert_fixture::<FunctionGroup>(
        include_bytes!("fixtures/function-group-z-test-group.xml"),
        "/sap/bc/adt/functions/groups/z_test_group",
        "202602011513320011",
    );
    assert_fixture::<FunctionModule>(
        include_bytes!("fixtures/function-module-zzzzfunc.xml"),
        "/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc",
        "202608051521490001",
    );
    assert_fixture::<FunctionGroupInclude>(
        include_bytes!("fixtures/function-group-include-lz-test-grouptop.xml"),
        "/sap/bc/adt/functions/groups/z_test_group/includes/lz_test_grouptop",
        "202602011513330011",
    );
    assert_fixture::<DataDefinition>(
        include_bytes!("fixtures/data-definition-i-businesspartner.xml"),
        "/sap/bc/adt/ddic/ddl/sources/i_businesspartner",
        "202407311918430011",
    );
    assert_fixture::<AccessControl>(
        include_bytes!("fixtures/access-control-sdsh-cds-domain-val-dcl.xml"),
        "/sap/bc/adt/acm/dcl/sources/sdsh_cds_domain_val_dcl",
        "201801261226400012",
    );
    assert_fixture::<AnnotationDefinition>(
        include_bytes!("fixtures/annotation-definition-ui.xml"),
        "/sap/bc/adt/ddic/ddla/sources/ui",
        "19710401000000001text/plain_lpyOs+SzZJEMV7O0meZa1OQvWK0=",
    );
    assert_fixture::<MetadataExtension>(
        include_bytes!("fixtures/metadata-extension-c-mdoapplicationscope.xml"),
        "/sap/bc/adt/ddic/ddlx/sources/c_mdoapplicationscope",
        "19710401000000001text/plain_6S5mImcIUSm07jSi3KPNPRdh93A=",
    );
    assert_fixture::<ServiceDefinition>(
        include_bytes!("fixtures/service-definition-managedistributions.xml"),
        "/sap/bc/adt/ddic/srvd/sources/managedistributions",
        "19710401000000001text/plain_ePZm8zDioCG3DivtaQS+CKSbFHQ=",
    );
}

#[test]
fn class_main_and_every_named_component_keep_their_own_fixture_etags() {
    for (xml, uri, components) in [
        (
            CLASS_XML,
            CLASS_URI,
            &[
                ("main", "source/main", "20210406145501001000181"),
                ("definitions", "includes/definitions", "201701161841300011"),
                (
                    "implementations",
                    "includes/implementations",
                    "201701161841300011",
                ),
                ("macros", "includes/macros", "201602231829380011"),
                ("testclasses", "includes/testclasses", "202003161335160011"),
            ][..],
        ),
        (
            include_bytes!("fixtures/class-cx-root-v4.xml").as_slice(),
            "/sap/bc/adt/oo/classes/cx_root",
            &[
                ("main", "source/main", "20180326130103001000061"),
                ("localtypes", "includes/localtypes", "201109092151410011"),
            ][..],
        ),
    ] {
        let (typed, erased) = snapshots::<Class>(xml, uri);
        for &(name, href, etag) in components {
            let source = typed.source_component(name).unwrap().unwrap();
            assert_eq!(Some(source.clone()), erased.source_component(name).unwrap());
            assert_eq!(source.object, typed.reference().erase());
            assert_eq!(source.uri.as_str(), format!("{uri}/{href}"));
            assert_eq!(source.etag.as_deref(), Some(etag), "{uri}: {name}");
            if name == "main" {
                assert_eq!(source, typed.source().unwrap());
                assert_eq!(source, erased.source().unwrap());
            }
        }
        for name in [
            "main",
            "definitions",
            "implementations",
            "macros",
            "testclasses",
            "localtypes",
            "unknown",
        ] {
            if !components.iter().any(|component| component.0 == name) {
                assert_eq!(typed.source_component(name).unwrap(), None);
                assert_eq!(erased.source_component(name).unwrap(), None);
            }
        }
    }
}

#[test]
fn snapshot_resources_preserve_scopes_unknown_links_and_wire_properties() {
    let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
    let mut future = source_link(
        "https://example.invalid/unrelated",
        Some("text/plain"),
        Some("future-tag"),
    );
    future.relation = Some("urn:future:relation".to_owned());
    properties.links.extend([future.clone(), future.clone()]);
    properties
        .syntax_configuration
        .as_mut()
        .unwrap()
        .language
        .as_mut()
        .unwrap()
        .links
        .push(future.clone());
    let main = properties
        .sources
        .iter_mut()
        .find(|source| source.include_type == "main")
        .unwrap();
    main.links.push(future.clone());
    let mut future_component = main.clone();
    future_component.include_type = "future-component".to_owned();
    properties.sources.push(future_component);
    let expected_properties = serde_json::to_value(&properties).unwrap();

    let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
    assert_eq!(typed.resources(), erased.resources());
    assert_eq!(
        serde_json::to_value(typed.properties()).unwrap(),
        expected_properties
    );
    assert_eq!(erased.properties().unwrap(), expected_properties);
    let future_resources = typed
        .resources()
        .iter()
        .filter(|resource| resource.relation() == Some("urn:future:relation"))
        .collect::<Vec<zadt::SnapshotResource<'_>>>();
    assert_eq!(
        future_resources
            .iter()
            .copied()
            .map(|resource| resource.component())
            .collect::<Vec<_>>(),
        [None, None, None, Some("main"), Some("future-component")]
    );
    let properties = typed.properties();
    let main = properties
        .sources
        .iter()
        .find(|s| s.include_type == "main")
        .unwrap();
    let future_component = properties.sources.last().unwrap();
    let language = properties
        .syntax_configuration
        .as_ref()
        .unwrap()
        .language
        .as_ref()
        .unwrap();
    let root_count = properties.links.len();
    for (resource, (link, name)) in future_resources.iter().copied().zip([
        (&properties.links[root_count - 2], None),
        (&properties.links[root_count - 1], None),
        (language.links.last().unwrap(), None),
        (main.links.last().unwrap(), Some(main.include_type.as_str())),
        (
            future_component.links.last().unwrap(),
            Some(future_component.include_type.as_str()),
        ),
    ]) {
        assert_eq!(resource.link(), Some(&future));
        assert!(std::ptr::eq(resource.link().unwrap(), link));
        assert!(std::ptr::eq(resource.href(), link.href.as_str()));
        if let Some(name) = name {
            assert!(std::ptr::eq(resource.component().unwrap(), name));
        }
        assert!(resource.resolve(typed.uri()).is_err());
    }
    assert_eq!(
        future_resources,
        typed
            .resources()
            .iter()
            .filter(|resource| resource.relation() == Some("urn:future:relation"))
            .collect::<Vec<_>>()
    );
    assert!(typed.object_structure().is_ok());
    assert!(erased.object_structure().is_ok());
    assert_eq!(typed.source().unwrap(), erased.source().unwrap());
    assert_eq!(
        typed.source_component("future-component").unwrap(),
        erased.source_component("future-component").unwrap()
    );
    assert!(
        typed
            .source_component("future-component")
            .unwrap()
            .is_some()
    );
}

#[test]
fn explicit_plain_text_metadata_wins_over_untyped_and_html_in_either_order() {
    let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
    let main = properties
        .sources
        .iter_mut()
        .find(|s| s.include_type == "main")
        .unwrap();
    main.links = vec![
        source_link("source/main", Some("text/html"), Some("html-etag")),
        source_link("source/main", None, Some("untyped-etag")),
        source_link(
            "source/main",
            Some("text/plain; charset=utf-8"),
            Some("plain-etag"),
        ),
    ];
    for reverse in [false, true] {
        if reverse {
            properties
                .sources
                .iter_mut()
                .find(|s| s.include_type == "main")
                .unwrap()
                .links
                .reverse();
        }
        let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
        let source = typed.source().unwrap();
        assert_eq!(source.etag.as_deref(), Some("plain-etag"));
        assert_eq!(source, erased.source().unwrap());
        assert_eq!(
            Some(source.clone()),
            typed.source_component("main").unwrap()
        );
        assert_eq!(Some(source), erased.source_component("main").unwrap());
    }
}

#[test]
fn metadata_matches_resolved_relative_and_root_relative_uri_query_and_fragment() {
    let relative = "source/main?version=inactive&note=a+b#section";
    let root_relative = format!("{CLASS_URI}/source/main?version=inactive&note=a%20b#section");
    for (href, metadata_href, media_type) in [
        (relative, root_relative.as_str(), Some("text/plain")),
        (root_relative.as_str(), relative, Some("text/plain")),
        (root_relative.as_str(), relative, None),
    ] {
        let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
        let main = properties
            .sources
            .iter_mut()
            .find(|s| s.include_type == "main")
            .unwrap();
        main.source_uri = href.to_owned();
        main.links = vec![source_link(metadata_href, media_type, Some("matched-etag"))];
        main.links[0].title = Some("Main source".to_owned());
        main.links[0].hreflang = Some("en".to_owned());
        main.links[0].length = Some("123".to_owned());
        let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
        assert_eq!(typed.resources(), erased.resources());
        let main = typed
            .properties()
            .sources
            .iter()
            .find(|s| s.include_type == "main")
            .unwrap();
        let raw = &main.links[0];
        let resource = typed
            .resources()
            .iter()
            .find(|r| r.component() == Some("main"))
            .unwrap();
        assert!(std::ptr::eq(resource.link().unwrap(), raw));
        assert!(std::ptr::eq(
            resource.component().unwrap(),
            main.include_type.as_str()
        ));
        assert!(std::ptr::eq(resource.href(), main.source_uri.as_str()));
        assert_eq!(resource.href(), href);
        assert_eq!(raw.href, metadata_href);
        assert_ne!(resource.href(), raw.href);
        assert_eq!(resource.media_type(), Some("text/plain"));
        assert_eq!(raw.media_type.as_deref(), media_type);
        for (actual, original) in [
            (resource.relation(), raw.relation.as_deref()),
            (resource.etag(), raw.etag.as_deref()),
            (resource.title(), raw.title.as_deref()),
            (resource.hreflang(), raw.hreflang.as_deref()),
            (resource.length(), raw.length.as_deref()),
        ] {
            assert_eq!(actual, original);
            assert!(std::ptr::eq(actual.unwrap(), original.unwrap()));
        }
        if let Some(media_type) = raw.media_type.as_deref() {
            assert!(std::ptr::eq(resource.media_type().unwrap(), media_type));
        }
        let source = typed.source().unwrap();
        assert_eq!(source, erased.source().unwrap());
        assert_eq!(source.uri.as_str(), format!("{CLASS_URI}/source/main"));
        assert_eq!(
            source.query,
            [
                ("version".to_owned(), "inactive".to_owned()),
                ("note".to_owned(), "a b".to_owned())
            ]
        );
        assert_eq!(source.fragment.as_deref(), Some("section"));
        assert_eq!(source.etag.as_deref(), Some("matched-etag"));
        assert_eq!(
            resource.resolve(typed.uri()).unwrap(),
            zadt::AdtLink {
                href: href.to_owned(),
                target: source.uri.clone(),
                query: source.query.clone(),
                fragment: source.fragment.clone(),
                relation: raw.relation.clone(),
                media_type: Some("text/plain".to_owned()),
                etag: raw.etag.clone(),
                title: raw.title.clone(),
                hreflang: raw.hreflang.clone(),
                length: raw.length.clone(),
            }
        );
        let request = source.query().encode(&()).unwrap();
        assert_eq!(request.target(), &source.uri);
        assert_eq!(request.query(), source.query);
    }
}

#[test]
fn unrelated_or_invalid_metadata_cannot_override_the_authoritative_source() {
    let href = "source/main?version=inactive#section";
    let mut versions = source_link(href, Some("text/plain"), Some("wrong-etag"));
    versions.relation = Some("http://www.sap.com/adt/relations/versions".to_owned());
    let mut no_relation = source_link(href, Some("text/plain"), Some("wrong-etag"));
    no_relation.relation = None;
    for candidate in [
        source_link(href, Some("text/html"), Some("wrong-etag")),
        versions,
        no_relation,
        source_link(
            "source/other?version=inactive#section",
            Some("text/plain"),
            Some("wrong-etag"),
        ),
        source_link(
            "source/main?version=active#section",
            Some("text/plain"),
            Some("wrong-etag"),
        ),
        source_link(
            "source/main?version=inactive#other",
            Some("text/plain"),
            Some("wrong-etag"),
        ),
        source_link(
            "source/main?version=inactive",
            Some("text/plain"),
            Some("wrong-etag"),
        ),
        source_link(
            "source/main#section",
            Some("text/plain"),
            Some("wrong-etag"),
        ),
        source_link(
            "https://example.invalid/source/main",
            Some("text/plain"),
            Some("wrong-etag"),
        ),
        source_link("", Some("text/plain"), Some("wrong-etag")),
    ] {
        let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
        let main = properties
            .sources
            .iter_mut()
            .find(|s| s.include_type == "main")
            .unwrap();
        main.source_uri = href.to_owned();
        main.links = vec![candidate.clone()];
        let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
        let source = typed.source().unwrap();
        assert_eq!(source, erased.source().unwrap());
        assert_eq!(source.uri.as_str(), format!("{CLASS_URI}/source/main"));
        assert_eq!(
            source.query,
            [("version".to_owned(), "inactive".to_owned())]
        );
        assert_eq!(source.fragment.as_deref(), Some("section"));
        assert_eq!(source.etag, None, "{candidate:?}");

        properties
            .sources
            .iter_mut()
            .find(|s| s.include_type == "main")
            .unwrap()
            .links
            .push(source_link(href, None, Some("fallback-etag")));
        let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
        assert_eq!(
            typed.source().unwrap().etag.as_deref(),
            Some("fallback-etag")
        );
        assert_eq!(typed.source().unwrap(), erased.source().unwrap());
    }
}

#[test]
fn a_missing_preferred_tag_does_not_borrow_from_other_representations_or_scopes() {
    let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
    properties.links.push(source_link(
        "source/main",
        Some("text/plain"),
        Some("root-etag"),
    ));
    for source in &mut properties.sources {
        source.links = if source.include_type == "main" {
            vec![
                source_link("source/main", None, Some("untyped-etag")),
                source_link("source/main", Some("text/html"), Some("html-etag")),
                source_link("source/main", Some("text/plain"), None),
            ]
        } else {
            vec![source_link(
                "source/main",
                Some("text/plain"),
                Some("sibling-etag"),
            )]
        };
    }
    let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
    assert_eq!(typed.source().unwrap().etag, None);
    assert_eq!(typed.source().unwrap(), erased.source().unwrap());
    assert_eq!(typed.source_component("main").unwrap().unwrap().etag, None);
    assert_eq!(erased.source_component("main").unwrap().unwrap().etag, None);

    properties
        .sources
        .iter_mut()
        .find(|s| s.include_type == "main")
        .unwrap()
        .links
        .clear();
    let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
    assert_eq!(typed.source().unwrap().etag, None);
    assert_eq!(typed.source().unwrap(), erased.source().unwrap());
}

#[test]
fn href_only_components_succeed_without_metadata_and_absent_components_stay_absent() {
    let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
    properties.links.clear();
    for source in &mut properties.sources {
        source.links.clear();
    }
    let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
    assert_eq!(typed.source().unwrap().etag, None);
    assert_eq!(typed.source().unwrap(), erased.source().unwrap());
    assert_eq!(typed.resources(), erased.resources());
    let resources = typed
        .resources()
        .iter()
        .filter(|resource| resource.component().is_some())
        .collect::<Vec<zadt::SnapshotResource<'_>>>();
    assert_eq!(resources.len(), typed.properties().sources.len());
    for (resource, component) in resources.iter().copied().zip(&typed.properties().sources) {
        assert_eq!(resource.link(), None);
        assert!(std::ptr::eq(
            resource.component().unwrap(),
            component.include_type.as_str()
        ));
        assert!(std::ptr::eq(resource.href(), component.source_uri.as_str()));
        assert_eq!(resource.relation(), Some(SOURCE_RELATION));
        assert_eq!(resource.media_type(), Some("text/plain"));
        assert_eq!(
            [
                resource.etag(),
                resource.title(),
                resource.hreflang(),
                resource.length()
            ],
            [None; 4]
        );
        let source = typed
            .source_component(&component.include_type)
            .unwrap()
            .unwrap();
        assert_eq!(source.etag, None);
        assert_eq!(
            Some(source),
            erased.source_component(&component.include_type).unwrap()
        );
    }
    assert_eq!(
        resources,
        typed
            .resources()
            .iter()
            .filter(|resource| resource.component().is_some())
            .collect::<Vec<_>>()
    );

    properties.sources.clear();
    properties.links.push(source_link(
        "source/main",
        Some("text/plain"),
        Some("root-etag"),
    ));
    let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
    for result in [typed.source(), erased.source()] {
        assert!(matches!(
            result,
            Err(ObjectError::MissingRelation { relation: "source" })
        ));
    }
    for name in [
        "main",
        "definitions",
        "implementations",
        "macros",
        "testclasses",
        "localtypes",
    ] {
        assert_eq!(typed.source_component(name).unwrap(), None);
        assert_eq!(erased.source_component(name).unwrap(), None);
    }
}

#[test]
fn invalid_authoritative_source_href_errors_despite_valid_metadata() {
    for href in ["", "https://example.invalid/source/main", "source\\main"] {
        let mut properties = ClassProperties::from_xml(CLASS_XML).unwrap();
        properties
            .sources
            .iter_mut()
            .find(|s| s.include_type == "main")
            .unwrap()
            .source_uri = href.to_owned();
        let (typed, erased) = snapshots::<Class>(&properties.to_xml().unwrap(), CLASS_URI);
        for result in [typed.source(), erased.source()] {
            assert!(
                matches!(result, Err(ObjectError::InvalidLink { href: actual, .. }) if actual == href)
            );
        }
        for result in [
            typed.source_component("main"),
            erased.source_component("main"),
        ] {
            assert!(
                matches!(result, Err(ObjectError::InvalidLink { href: actual, .. }) if actual == href)
            );
        }
    }
}

#[test]
fn modeled_interface_does_not_gain_source_from_advertised_metadata() {
    let mut properties = InterfaceProperties::from_xml(include_bytes!(
        "fixtures/interface-if-adt-uri-mapper-v5.xml"
    ))
    .unwrap();
    properties.modeled = true;
    let (typed, erased) = snapshots::<Interface>(
        &properties.to_xml().unwrap(),
        "/sap/bc/adt/oo/interfaces/if_adt_uri_mapper",
    );
    for result in [typed.source(), erased.source()] {
        assert!(matches!(
            result,
            Err(ObjectError::MissingRelation { relation: "source" })
        ));
    }
}

#[test]
fn object_advertised_source_and_fresh_source_response_etags_remain_distinct() {
    let (typed, erased) = snapshots::<Class>(CLASS_XML, CLASS_URI);
    for source in [typed.source().unwrap(), erased.source().unwrap()] {
        for etag in [Some("fresh-source-etag"), None] {
            let fetched = source
                .query()
                .decode(response(
                    b"CLASS cl_adt_uri_mapper DEFINITION.\nENDCLASS.\n",
                    source.uri.as_str(),
                    "text/plain; charset=utf-8",
                    etag,
                ))
                .unwrap();
            assert_eq!(
                fetched.content,
                "CLASS cl_adt_uri_mapper DEFINITION.\nENDCLASS.\n"
            );
            assert_eq!(fetched.etag.as_deref(), etag);
            assert_eq!(fetched.reference, source);
            assert_eq!(
                fetched.reference.etag.as_deref(),
                Some("20210406145501001000181")
            );
        }
    }
    assert_eq!(typed.etag().map(EntityTag::as_str), Some("object-etag"));
    assert_eq!(erased.etag(), typed.etag());
}
