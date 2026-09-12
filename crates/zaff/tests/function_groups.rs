//! Public AFF projections and caller-owned ZADT saves, without sockets or credentials.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use http::{HeaderMap, Method, StatusCode};
use serde_json::{Value, json};
use zadt::{
    AdtRequest, AdtResponse, AdtUri, Client, EntityTag, FunctionGroup, FunctionGroupInclude,
    FunctionGroupIncludeProperties, FunctionGroupProperties, FunctionModule,
    FunctionModuleProperties, ObjectError, ObjectRef, ObjectSnapshot, ObjectType, Operation,
    OperationResponse, PreconditionResult, ToXml, Transport, TransportError, WorkbenchVersion,
};
use zaff::{Cardinality, FileBacking, Projection, ProjectionError, PropertiesProjection, project};

const GROUP_XML: &str = include_str!("../../zadt/tests/fixtures/function-group-z-test-group.xml");
const GROUP_V2_XML: &str =
    include_str!("../../zadt/tests/fixtures/function-group-z-test-group-v2.xml");
const INCLUDE_XML: &str =
    include_str!("../../zadt/tests/fixtures/function-group-include-lz-test-grouptop.xml");
const MODULE_XML: &str = include_str!("../../zadt/tests/fixtures/function-module-zzzzfunc.xml");
const GROUP_JSON: &str = "z_test_group.fugr.json";
const MAIN_JSON: &str = "z_test_group.fugr.saplz_test_group.reps.json";
const INCLUDE_JSON: &str = "z_test_group.fugr.lz_test_grouptop.reps.json";
const MODULE_JSON: &str = "z_test_group.fugr.zzzzfunc.func.json";

#[derive(Clone, Copy)]
struct Fixture {
    kind: &'static str,
    name: &'static str,
    uri: &'static str,
    media: &'static str,
    etag: &'static str,
    xml: &'static str,
}

// Object locations and explicit parent_uri deliberately differ from logical names.
const GROUP: Fixture = Fixture {
    kind: "FUGR/F",
    name: "Z_TEST_GROUP",
    uri: "/sap/bc/adt/custom/Group%2FLocation",
    media: "application/vnd.sap.adt.functions.groups.v3+xml",
    etag: "\"group-properties\"",
    xml: GROUP_XML,
};
const INCLUDE: Fixture = Fixture {
    kind: "FUGR/I",
    name: "LZ_TEST_GROUPTOP",
    uri: "/sap/bc/adt/custom/Include%2FLocation",
    media: "application/vnd.sap.adt.functions.fincludes.v2+xml",
    etag: "\"include-properties\"",
    xml: INCLUDE_XML,
};
const MODULE: Fixture = Fixture {
    kind: "FUGR/FF",
    name: "ZZZZFUNC",
    uri: "/sap/bc/adt/custom/Module%2FLocation",
    media: "application/vnd.sap.adt.functions.fmodules.v3+xml",
    etag: "\"module-properties\"",
    xml: MODULE_XML,
};
const GROUP_V2: Fixture = Fixture {
    media: "application/vnd.sap.adt.functions.groups.v2+xml",
    xml: GROUP_V2_XML,
    ..GROUP
};

fn headers(fields: &[(&str, &str)]) -> HeaderMap {
    fields
        .iter()
        .map(|(key, value)| {
            (
                key.parse::<http::header::HeaderName>().unwrap(),
                value.parse().unwrap(),
            )
        })
        .collect()
}

fn response(status: StatusCode, fields: &[(&str, &str)], body: impl Into<Vec<u8>>) -> AdtResponse {
    AdtResponse::new(status, headers(fields), body.into())
}

impl Fixture {
    fn reference(self, name: &str, parent: Option<&str>) -> ObjectRef<()> {
        let mut key = json!({"name": name, "object_type": self.kind});
        if let Some(parent) = parent {
            key["parent"] = json!({"name": parent, "object_type": "FUGR/F"});
        }
        ObjectRef::new(
            serde_json::from_value(key).unwrap(),
            AdtUri::parse(self.uri).unwrap(),
        )
        .with_parent_uri(AdtUri::parse("/sap/bc/adt/custom/NotTheGroupName").unwrap())
    }

    fn snapshot(self, reference: &ObjectRef<()>, xml: &str) -> ObjectSnapshot<()> {
        reference
            .query()
            .decode(OperationResponse::new(
                response(
                    StatusCode::OK,
                    &[("content-type", self.media), ("etag", self.etag)],
                    xml,
                ),
                reference.uri().clone(),
            ))
            .unwrap()
    }

    fn baseline(self) -> ObjectSnapshot<()> {
        self.snapshot(&self.reference(self.name, None), self.xml)
    }

    fn pair<T: ObjectType>(self, reference: &ObjectRef<()>, xml: &str) -> [ObjectSnapshot<()>; 2] {
        let typed = reference
            .typed::<T>()
            .unwrap()
            .query()
            .decode(OperationResponse::new(
                response(
                    StatusCode::OK,
                    &[("content-type", self.media), ("etag", self.etag)],
                    xml,
                ),
                reference.uri().clone(),
            ))
            .unwrap();
        [typed.into_erased(), self.snapshot(reference, xml)]
    }
}

fn metadata<'a>(projection: &'a Projection, name: &str) -> &'a PropertiesProjection {
    let FileBacking::Properties(properties) = projection.file(name).unwrap().backing() else {
        panic!("{name} must be properties-backed");
    };
    properties
}

fn document(properties: &PropertiesProjection) -> Value {
    let rendered = properties.render().unwrap();
    assert!(rendered.ends_with('\n'));
    serde_json::from_str(&rendered).unwrap()
}

#[test]
fn metadata_rejects_positional_objects_and_object_shaped_enums() {
    for (fixture, filename) in [
        (GROUP, GROUP_JSON),
        (GROUP, MAIN_JSON),
        (INCLUDE, INCLUDE_JSON),
        (MODULE, MODULE_JSON),
    ] {
        let projection = project(fixture.baseline()).unwrap();
        let properties = metadata(&projection, filename);
        let baseline = document(properties);
        let mut header_array = baseline.clone();
        header_array["header"] = if filename == GROUP_JSON {
            json!([baseline["header"]["description"], "en"])
        } else {
            json!([baseline["header"]["description"]])
        };
        let root_array = if filename == GROUP_JSON {
            json!(["1", baseline["header"], true, "notClassified"])
        } else if filename.ends_with(".reps.json") {
            json!(["1", baseline["header"], false, baseline["includeType"]])
        } else {
            json!([
                "1",
                baseline["header"],
                "normal",
                null,
                null,
                "notReleased",
                null,
                false,
                false,
                "",
                "",
                false,
                "00",
                false,
                false,
                [],
                []
            ])
        };
        for edited in [header_array, root_array] {
            assert!(
                matches!(
                    properties.merge(&edited.to_string()),
                    Err(ProjectionError::Json(_))
                ),
                "{filename}: {edited}"
            );
        }

        let enum_fields = if filename == GROUP_JSON {
            vec![("status", "notClassified")]
        } else if filename.ends_with(".reps.json") {
            vec![("includeType", baseline["includeType"].as_str().unwrap())]
        } else {
            vec![
                ("processingType", "normal"),
                ("releaseState", "notReleased"),
            ]
        };
        for (field, variant) in enum_fields {
            let mut edited = baseline.clone();
            edited[field] = json!({variant: null});
            assert!(matches!(
                properties.merge(&edited.to_string()),
                Err(ProjectionError::Json(_))
            ));
        }
        if filename == GROUP_JSON {
            let mut edited = baseline.clone();
            edited["header"]["abapLanguageVersion"] = json!({"standard": null});
            assert!(matches!(
                properties.merge(&edited.to_string()),
                Err(ProjectionError::Json(_))
            ));
        }
        if filename == MODULE_JSON {
            for (field, value) in [
                ("rfcProperties", json!([false, "notClassified", "any"])),
                ("updateProperties", json!(["startImmediately"])),
                ("parameters", json!([["P", "Description"]])),
                ("exceptions", json!([["E", "Description"]])),
                (
                    "rfcProperties",
                    json!({"basxmlEnabled": false, "rfcScope": {"notClassified": null}, "rfcVersion": "any"}),
                ),
                (
                    "rfcProperties",
                    json!({"basxmlEnabled": false, "rfcScope": "notClassified", "rfcVersion": {"any": null}}),
                ),
                (
                    "updateProperties",
                    json!({"updateTaskKind": {"startImmediately": null}}),
                ),
            ] {
                let mut edited = baseline.clone();
                edited[field] = value;
                assert!(
                    matches!(
                        properties.merge(&edited.to_string()),
                        Err(ProjectionError::Json(_))
                    ),
                    "{edited}"
                );
            }
        }
    }
}

#[test]
fn typed_and_runtime_inventory_names_sources_and_metadata_match_exactly() {
    for namespaced in [false, true] {
        for fixture in [GROUP, GROUP_V2, INCLUDE, MODULE] {
            let group = if namespaced { "/ACME/DEMO" } else { GROUP.name };
            let name = if namespaced {
                match fixture.kind {
                    "FUGR/F" => group,
                    "FUGR/I" => "/ACME/LDEMOTOP",
                    _ => "/ACME/RUN",
                }
            } else {
                fixture.name
            };
            let xml = fixture
                .xml
                .replace(fixture.name, name)
                .replace("Z_TEST_GROUP", group);
            let reference = fixture.reference(name, (fixture.kind != "FUGR/F").then_some(group));
            let snapshots = match fixture.kind {
                "FUGR/F" => fixture.pair::<FunctionGroup>(&reference, &xml),
                "FUGR/I" => fixture.pair::<FunctionGroupInclude>(&reference, &xml),
                _ => fixture.pair::<FunctionModule>(&reference, &xml),
            };
            let [typed, runtime] = snapshots.map(|snapshot| project(snapshot).unwrap());
            assert_eq!(
                typed.subject().properties().unwrap(),
                runtime.subject().properties().unwrap()
            );
            let expected = match (namespaced, fixture.kind) {
                (false, "FUGR/F") => vec![
                    GROUP_JSON,
                    "z_test_group.fugr.saplz_test_group.reps.abap",
                    MAIN_JSON,
                ],
                (true, "FUGR/F") => vec![
                    "(acme)demo.fugr.json",
                    "(acme)demo.fugr.(acme)sapldemo.reps.abap",
                    "(acme)demo.fugr.(acme)sapldemo.reps.json",
                ],
                (false, "FUGR/I") => {
                    vec![INCLUDE_JSON, "z_test_group.fugr.lz_test_grouptop.reps.abap"]
                }
                (true, "FUGR/I") => vec![
                    "(acme)demo.fugr.(acme)ldemotop.reps.json",
                    "(acme)demo.fugr.(acme)ldemotop.reps.abap",
                ],
                (false, _) => vec![MODULE_JSON, "z_test_group.fugr.zzzzfunc.func.abap"],
                (true, _) => vec![
                    "(acme)demo.fugr.(acme)run.func.json",
                    "(acme)demo.fugr.(acme)run.func.abap",
                ],
            };
            for projection in [&typed, &runtime] {
                assert_eq!(
                    projection
                        .files()
                        .iter()
                        .map(|file| file.name())
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(projection.format().version(), "1");
                assert_eq!(
                    projection.format().object_type(),
                    match fixture.kind {
                        "FUGR/F" => "FUGR",
                        "FUGR/I" => "REPS",
                        _ => "FUNC",
                    }
                );
                assert_eq!(
                    projection.format().workbench_types(),
                    &[reference.workbench_type().clone()]
                );
                assert_eq!(projection.subject().reference(), &reference);
                assert_eq!(projection.subject().key(), reference.key());
                assert_eq!(
                    projection.subject().reference().parent_uri(),
                    reference.parent_uri()
                );
                assert_eq!(projection.subject().media_type(), fixture.media);
                assert_eq!(
                    projection.subject().etag().map(EntityTag::as_str),
                    Some(fixture.etag)
                );
                assert_eq!(
                    projection.subject().workbench_version(),
                    if fixture.kind == "FUGR/FF" {
                        WorkbenchVersion::Inactive
                    } else {
                        WorkbenchVersion::Active
                    }
                );
                assert!(projection.file("not-a-file").is_none());
                let unsupported: Vec<_> = projection
                    .format()
                    .files()
                    .iter()
                    .filter(|spec| !spec.is_supported())
                    .collect();
                assert_eq!(unsupported.len(), usize::from(fixture.kind == "FUGR/F"));
                for spec in unsupported {
                    assert_eq!(spec.template(), "<name>.fugr.texts.<lang>.properties");
                    assert_eq!(spec.cardinality(), Cardinality::ZeroOrMore);
                }
                for file in projection.files() {
                    assert!(file.specification().is_supported());
                    assert_eq!(file.specification().cardinality(), Cardinality::One);
                    match file.backing() {
                        FileBacking::Source(source) => {
                            assert!(file.name().ends_with(".abap"));
                            assert_eq!(source, &projection.subject().source().unwrap());
                            assert_eq!(&source.object, &reference);
                            assert_eq!(source.object.key(), reference.key());
                            assert_eq!(source.object.parent_uri(), reference.parent_uri());
                            assert_eq!(source.uri.as_str(), format!("{}/source/main", fixture.uri));
                            assert_eq!(
                                source.etag.as_deref(),
                                Some(match fixture.kind {
                                    "FUGR/F" => "202602011513320011",
                                    "FUGR/I" => "202602011513330011",
                                    _ => "202608051521490001",
                                })
                            );
                            assert!(
                                source.query.is_empty(),
                                "source must not inherit properties version"
                            );
                            assert!(source.fragment.is_none());
                            assert!(source.query().encode(&()).unwrap().query().is_empty());
                        }
                        FileBacking::Properties(properties) => {
                            assert!(std::ptr::eq(properties.subject(), projection.subject()));
                            let expected = if file.name().ends_with(".fugr.json") {
                                json!({"formatVersion": "1", "header": {"description": "Test", "originalLanguage": "en"}, "fixPointArithmetic": true})
                            } else if file.name().ends_with(".func.json") {
                                json!({"formatVersion": "1", "header": {"description": "ftfrtat"}, "processingType": "normal"})
                            } else {
                                json!({"formatVersion": "1", "header": {"description": if fixture.kind == "FUGR/F" { "Test" } else { "" }}, "includeType": if fixture.kind == "FUGR/F" { "functionGroup" } else { "include" }})
                            };
                            assert_eq!(document(properties), expected);
                            assert_eq!(properties.merge(&expected.to_string()).unwrap(), None);
                            assert_eq!(
                                properties.render().unwrap(),
                                metadata(&runtime, file.name()).render().unwrap()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn source_selectors_validators_and_missing_or_invalid_advertisements_are_preserved() {
    for fixture in [GROUP, GROUP_V2, INCLUDE, MODULE] {
        let reference = fixture.reference(fixture.name, None);
        let xml = fixture.xml.replace(
            "source/main\"",
            "source/main?version=inactive&amp;note=a%2Bb#section\"",
        );
        let projection = project(fixture.snapshot(&reference, &xml)).unwrap();
        let source = projection
            .files()
            .iter()
            .find_map(|file| match file.backing() {
                FileBacking::Source(source) => Some(source),
                _ => None,
            })
            .unwrap();
        assert_eq!(source.object, reference);
        assert_eq!(source.uri.as_str(), format!("{}/source/main", fixture.uri));
        assert_eq!(
            source.query,
            [
                ("version".into(), "inactive".into()),
                ("note".into(), "a+b".into())
            ]
        );
        assert_eq!(source.fragment.as_deref(), Some("section"));
        assert_eq!(source.etag, fixture.baseline().source().unwrap().etag);
        assert_ne!(source.etag.as_deref(), Some(fixture.etag));
        let query = source.query().encode(&()).unwrap();
        assert_eq!(query.target(), &source.uri);
        assert_eq!(query.query(), source.query);

        // These ADT families require sourceUri. Missing link metadata must not hide
        // that source or borrow the properties ETag; a missing attribute fails decode.
        let no_links = fixture
            .xml
            .lines()
            .filter(|line| !line.contains("rel=\"http://www.sap.com/adt/relations/source\""))
            .collect::<Vec<_>>()
            .join("\n");
        let projection = project(fixture.snapshot(&reference, &no_links)).unwrap();
        let source = projection
            .files()
            .iter()
            .find_map(|file| match file.backing() {
                FileBacking::Source(source) => Some(source),
                _ => None,
            })
            .unwrap();
        assert_eq!(source.uri.as_str(), format!("{}/source/main", fixture.uri));
        assert_eq!(source.object, reference);
        assert_eq!(source.etag, None);
        let missing = no_links.replace(" abapsource:sourceUri=\"source/main\"", "");
        let result = reference.query().decode(OperationResponse::new(
            response(StatusCode::OK, &[("content-type", fixture.media)], missing),
            reference.uri().clone(),
        ));
        assert!(result.is_err(), "{} requires sourceUri", fixture.kind);
        for href in [
            "",
            "https://example.invalid/source/main",
            "source\\main",
            "/outside/adt",
        ] {
            let invalid = fixture.xml.replace("source/main\"", &format!("{href}\""));
            assert!(
                matches!(project(fixture.snapshot(&reference, &invalid)),
                Err(ProjectionError::InvalidObjectReference(ObjectError::InvalidLink { href: actual, .. })) if actual == href),
                "{}: {href}",
                fixture.kind
            );
        }
    }
}

#[test]
fn child_group_names_use_logical_or_container_names_never_uris() {
    for fixture in [INCLUDE, MODULE] {
        for (parent, advertised, expected) in [
            (None, Some("Z_TEST_GROUP"), Some("z_test_group")),
            (Some("Z_TEST_GROUP"), None, Some("z_test_group")),
            (Some("Z_TEST_GROUP"), Some(""), Some("z_test_group")),
            (
                Some("Z_TEST_GROUP"),
                Some("z_test_group"),
                Some("z_test_group"),
            ),
            (None, Some("/ACME/DEMO"), Some("(acme)demo")),
            (Some("/ACME/DEMO"), None, Some("(acme)demo")),
            (None, None, None),
            (None, Some(""), None),
            (Some("Z_TEST_GROUP"), Some("Z_OTHER"), None),
        ] {
            let replacement = advertised
                .map(|name| format!("adtcore:name=\"{name}\""))
                .unwrap_or_default();
            let xml = fixture
                .xml
                .replace("adtcore:name=\"Z_TEST_GROUP\"", &replacement);
            let reference = fixture.reference(fixture.name, parent);
            let result = project(fixture.snapshot(&reference, &xml));
            if let Some(group) = expected {
                let projection = result.unwrap();
                let extension = if fixture.kind == "FUGR/I" {
                    "reps"
                } else {
                    "func"
                };
                assert_eq!(
                    projection
                        .files()
                        .iter()
                        .map(|file| file.name().to_owned())
                        .collect::<Vec<_>>(),
                    [
                        format!(
                            "{group}.fugr.{}.{extension}.json",
                            fixture.name.to_ascii_lowercase()
                        ),
                        format!(
                            "{group}.fugr.{}.{extension}.abap",
                            fixture.name.to_ascii_lowercase()
                        )
                    ]
                );
                assert_eq!(projection.subject().key(), reference.key());
            } else if parent.is_some() {
                assert!(matches!(
                    result,
                    Err(ProjectionError::InvalidObjectReference(
                        ObjectError::InvalidParentObject { .. }
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProjectionError::InvalidObjectReference(
                        ObjectError::ParentObjectRequired { .. }
                    ))
                ));
            }
        }
        for parent in [None, Some("Z_TEST_GROUP")] {
            let xml = fixture
                .xml
                .replace("adtcore:type=\"FUGR/F\"", "adtcore:type=\"PROG/P\"");
            assert!(
                matches!(project(fixture.snapshot(&fixture.reference(fixture.name, parent), &xml)),
                Err(ProjectionError::InvalidObjectReference(ObjectError::UnexpectedObjectType { expected, actual }))
                    if expected == FunctionGroup::WORKBENCH_TYPE && actual.as_str() == "PROG/P")
            );
            let xml = fixture.xml.replace(" adtcore:type=\"FUGR/F\"", "");
            assert!(
                project(fixture.snapshot(&fixture.reference(fixture.name, parent), &xml)).is_ok()
            );
        }
    }
}

#[test]
fn metadata_edits_return_complete_owner_payloads_without_mutating_baselines() {
    for (fixture, name) in [
        (GROUP, GROUP_JSON),
        (GROUP_V2, GROUP_JSON),
        (GROUP, MAIN_JSON),
        (INCLUDE, INCLUDE_JSON),
        (MODULE, MODULE_JSON),
    ] {
        let projection = project(fixture.baseline()).unwrap();
        let properties = metadata(&projection, name).clone();
        let original = projection.subject().properties().unwrap();
        let baseline = properties.render().unwrap();
        let mut edited = document(&properties);
        edited["header"]["description"] = json!("Edited description");
        let mut expected = original.clone();
        expected["@adtcore:description"] = json!("Edited description");
        if name == GROUP_JSON {
            edited["fixPointArithmetic"] = json!(false);
            edited["header"]["originalLanguage"] = json!("de");
            expected["@abapsource:fixPointArithmetic"] = json!(false);
            expected["@adtcore:masterLanguage"] = json!("DE");
        } else if name == MAIN_JSON {
            edited["editLocked"] = json!(true);
            expected["@group:lockedByEditor"] = json!(true);
        } else if name == MODULE_JSON {
            edited["processingType"] = json!("rfc");
            edited["releaseState"] = json!("released");
            expected["@fmodule:processingType"] = json!("rfc");
            expected["@fmodule:releaseState"] = json!("external");
            expected["@fmodule:basXMLEnabled"] = json!(false);
            expected["@fmodule:abapFromJava"] = json!(false);
            expected["@fmodule:javaFromAbap"] = json!(false);
            expected["@fmodule:javaRemote"] = json!(false);
            expected["@fmodule:rfcScope"] = json!("notClassified");
            expected["@fmodule:rfcVersion"] = json!("any");
        }
        assert_eq!(
            properties.merge(&edited.to_string()).unwrap(),
            Some(expected)
        );
        assert_eq!(properties.subject().properties().unwrap(), original);
        assert_eq!(
            properties.subject().etag().map(EntityTag::as_str),
            Some(fixture.etag)
        );
        assert_eq!(properties.subject().uri().as_str(), fixture.uri);
        let version = properties.subject().workbench_version();
        drop(projection);
        assert_eq!(properties.subject().workbench_version(), version);
        assert_eq!(properties.render().unwrap(), baseline);
        assert_eq!(properties.merge(&baseline).unwrap(), None);
    }

    let projection = project(GROUP.baseline()).unwrap();
    let group = metadata(&projection, GROUP_JSON);
    let main = metadata(&projection, MAIN_JSON);
    assert!(std::ptr::eq(group.subject(), main.subject()));
    let mut group_edit = document(group);
    let mut main_edit = document(main);
    group_edit["header"]["description"] = json!("Shared description");
    main_edit["header"]["description"] = group_edit["header"]["description"].clone();
    let payload = group.merge(&group_edit.to_string()).unwrap().unwrap();
    assert_eq!(
        main.merge(&main_edit.to_string()).unwrap(),
        Some(payload.clone())
    );
    let saved: FunctionGroupProperties = serde_json::from_value(payload).unwrap();
    let fresh = project(GROUP.snapshot(
        projection.subject().reference(),
        std::str::from_utf8(&saved.to_xml().unwrap()).unwrap(),
    ))
    .unwrap();
    for name in [GROUP_JSON, MAIN_JSON] {
        assert_eq!(
            document(metadata(&fresh, name))["header"]["description"],
            "Shared description"
        );
        assert_eq!(
            document(metadata(&projection, name))["header"]["description"],
            "Test"
        );
    }
}

#[test]
fn group_languages_status_and_noop_guard_preserve_unexposed_properties() {
    for (wire, aff) in [
        (None, "standard"),
        (Some(""), "standard"),
        (Some(" "), "standard"),
        (Some("X"), "standard"),
        (Some("2"), "keyUser"),
        (Some("5"), "cloudDevelopment"),
    ] {
        let replacement = wire
            .map(|value| format!("adtcore:abapLanguageVersion=\"{value}\""))
            .unwrap_or_default();
        let xml = GROUP_XML.replace("adtcore:abapLanguageVersion=\"X\"", &replacement);
        for snapshot in GROUP.pair::<FunctionGroup>(&GROUP.reference(GROUP.name, None), &xml) {
            let projection = project(snapshot).unwrap();
            let properties = metadata(&projection, GROUP_JSON);
            let original = properties.subject().properties().unwrap();
            let mut doc = document(properties);
            assert_eq!(
                doc["header"].get("abapLanguageVersion"),
                (aff != "standard").then(|| json!(aff)).as_ref()
            );
            assert!(doc.get("status").is_none());
            doc["header"]["abapLanguageVersion"] = json!(aff);
            doc["status"] = json!("notClassified");
            assert_eq!(properties.merge(&doc.to_string()).unwrap(), None);
            for (target, value) in [
                ("standard", "X"),
                ("keyUser", "2"),
                ("cloudDevelopment", "5"),
            ] {
                let mut edited = doc.clone();
                edited["header"]["abapLanguageVersion"] = json!(target);
                let mut expected = original.clone();
                if target != aff {
                    expected["@adtcore:abapLanguageVersion"] = json!(value);
                }
                assert_eq!(
                    properties.merge(&edited.to_string()).unwrap(),
                    (target != aff).then_some(expected)
                );
            }
            // Description-only edits must not canonicalize blank/absent Standard or parser metadata.
            doc["header"]["description"] = json!("Other");
            let mut expected = original.clone();
            expected["@adtcore:description"] = json!("Other");
            assert_eq!(properties.merge(&doc.to_string()).unwrap(), Some(expected));
            assert_eq!(properties.subject().properties().unwrap(), original);
        }
    }
    let projection = project(GROUP.baseline()).unwrap();
    let properties = metadata(&projection, GROUP_JSON);
    let doc = document(properties);
    for (adt, aff) in [
        ("EN", "en"),
        ("DE", "de"),
        ("6N", "en-GB"),
        ("ZF", "zh-Hant"),
    ] {
        let xml = GROUP_XML.replace(
            "adtcore:masterLanguage=\"EN\"",
            &format!("adtcore:masterLanguage=\"{adt}\""),
        );
        let projection = project(GROUP.snapshot(&GROUP.reference(GROUP.name, None), &xml)).unwrap();
        let properties = metadata(&projection, GROUP_JSON);
        let rendered = document(properties);
        assert_eq!(rendered["header"]["originalLanguage"], aff);
        assert_eq!(properties.merge(&rendered.to_string()).unwrap(), None);
    }
    for (status, wire) in [
        ("sapProgram", "SAPStandardProduction"),
        ("customerProgram", "customerProduction"),
        ("systemProgram", "system"),
        ("testProgram", "test"),
    ] {
        let mut edited = doc.clone();
        edited["status"] = json!(status);
        let payload = properties.merge(&edited.to_string()).unwrap().unwrap();
        assert_eq!(payload["@abapsource:sourceObjectStatus"], wire);
    }
    for (field, value) in [
        ("originalLanguage", "not-supported"),
        ("abapLanguageVersion", "0"),
    ] {
        let (attribute, path) = if field == "originalLanguage" {
            ("masterLanguage=\"EN\"", "header.originalLanguage")
        } else {
            ("abapLanguageVersion=\"X\"", "header.abapLanguageVersion")
        };
        let xml = GROUP_XML.replace(
            attribute,
            &format!("{}=\"{value}\"", attribute.split('=').next().unwrap()),
        );
        let invalid = project(GROUP.snapshot(&GROUP.reference(GROUP.name, None), &xml)).unwrap();
        let invalid = metadata(&invalid, GROUP_JSON);
        for result in [
            invalid.render().map(|_| ()),
            invalid.merge(&doc.to_string()).map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(ProjectionError::InvalidAffField { field, .. }) if field == path)
            );
        }
    }
    let mut edited = doc;
    edited["header"]["originalLanguage"] = json!("not-supported");
    assert!(matches!(
        properties.merge(&edited.to_string()),
        Err(ProjectionError::InvalidAffField {
            field: "header.originalLanguage",
            ..
        })
    ));

    let reference = GROUP.reference(GROUP.name, None);
    let snapshot = reference
        .query()
        .decode(OperationResponse::new(
            response(StatusCode::OK, &[("content-type", GROUP.media)], GROUP_XML),
            reference.uri().clone(),
        ))
        .unwrap();
    let projection = project(snapshot).unwrap();
    for name in [GROUP_JSON, MAIN_JSON] {
        let properties = metadata(&projection, name);
        assert!(properties.subject().etag().is_none());
        let mut doc = document(properties);
        if name == GROUP_JSON {
            doc["status"] = json!("notClassified");
            doc["header"]["abapLanguageVersion"] = json!("standard");
        } else {
            doc["editLocked"] = json!(false);
        }
        assert_eq!(
            properties.merge(&doc.to_string()).unwrap(),
            None,
            "no-op needs no write validator"
        );
        doc["header"]["description"] = json!("Needs an ETag");
        let payload = properties.merge(&doc.to_string()).unwrap().unwrap();
        assert!(matches!(
            properties.subject().update_if_match(payload),
            Err(ObjectError::MissingEntityTag)
        ));
    }
}

#[test]
fn headers_required_fields_types_and_description_limits_are_strict() {
    for (fixture, name, required, header_required, limit) in [
        (
            GROUP,
            GROUP_JSON,
            &["formatVersion", "header", "fixPointArithmetic"][..],
            &["description", "originalLanguage"][..],
            40,
        ),
        (
            GROUP,
            MAIN_JSON,
            &["formatVersion", "header", "includeType"][..],
            &["description"][..],
            40,
        ),
        (
            INCLUDE,
            INCLUDE_JSON,
            &["formatVersion", "header", "includeType"][..],
            &["description"][..],
            70,
        ),
        (
            MODULE,
            MODULE_JSON,
            &["formatVersion", "header", "processingType"][..],
            &["description"][..],
            74,
        ),
    ] {
        let projection = project(fixture.baseline()).unwrap();
        let properties = metadata(&projection, name);
        let doc = document(properties);
        for (pointer, fields) in [("", required), ("/header", header_required)] {
            for &field in fields {
                let mut edited = doc.clone();
                edited
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                assert!(
                    matches!(
                        properties.merge(&edited.to_string()),
                        Err(ProjectionError::Json(_))
                    ),
                    "{name} missing {pointer}/{field}"
                );
                edited.pointer_mut(pointer).unwrap()[field] = Value::Null;
                assert!(
                    matches!(
                        properties.merge(&edited.to_string()),
                        Err(ProjectionError::Json(_))
                    ),
                    "{name} null {pointer}/{field}"
                );
            }
            let mut edited = doc.clone();
            edited.pointer_mut(pointer).unwrap()["unknown"] = json!(false);
            assert!(matches!(
                properties.merge(&edited.to_string()),
                Err(ProjectionError::Json(_))
            ));
        }
        for field in ["originalLanguage", "abapLanguageVersion"] {
            let mut edited = doc.clone();
            edited["header"][field] = if name == GROUP_JSON {
                Value::Null
            } else {
                json!("standard")
            };
            assert!(matches!(
                properties.merge(&edited.to_string()),
                Err(ProjectionError::Json(_))
            ));
        }
        for length in [0, limit, limit + 1] {
            let mut edited = doc.clone();
            edited["header"]["description"] = json!("\u{00e9}".repeat(length));
            let result = properties.merge(&edited.to_string());
            if length <= limit {
                assert!(result.is_ok(), "{name}: {length}: {result:?}");
            } else if name == MAIN_JSON {
                assert!(matches!(
                    result,
                    Err(ProjectionError::InvalidAffField {
                        field: "header.description",
                        ..
                    })
                ));
            } else {
                assert!(matches!(result, Err(ProjectionError::Validation(_))));
            }
        }
        let mut edited = doc.clone();
        edited["formatVersion"] = json!("2");
        assert!(matches!(
            properties.merge(&edited.to_string()),
            Err(ProjectionError::Validation(_))
        ));
        edited = doc.clone();
        edited["header"]["description"] = json!(42);
        assert!(matches!(
            properties.merge(&edited.to_string()),
            Err(ProjectionError::Json(_))
        ));
        if name.ends_with(".reps.json") {
            edited = doc.clone();
            edited["includeType"] = json!(if fixture.kind == "FUGR/F" {
                "include"
            } else {
                "functionGroup"
            });
            assert!(matches!(
                properties.merge(&edited.to_string()),
                Err(ProjectionError::InvalidAffField {
                    field: "includeType",
                    ..
                })
            ));
            edited["includeType"] = json!("unknown");
            assert!(matches!(
                properties.merge(&edited.to_string()),
                Err(ProjectionError::Json(_))
            ));
            for length in [70, 71] {
                edited = doc.clone();
                edited["header"]["description"] = json!("x".repeat(length));
                let result = properties.merge(&edited.to_string());
                if length == 71 {
                    assert!(matches!(result, Err(ProjectionError::Validation(_))));
                } else if name == MAIN_JSON {
                    assert!(matches!(
                        result,
                        Err(ProjectionError::InvalidAffField {
                            field: "header.description",
                            ..
                        })
                    ));
                } else {
                    assert!(result.unwrap().is_some());
                }
            }
        }
        if name == GROUP_JSON {
            for value in ["", "e"] {
                edited = doc.clone();
                edited["header"]["originalLanguage"] = json!(value);
                assert!(matches!(
                    properties.merge(&edited.to_string()),
                    Err(ProjectionError::Validation(_))
                ));
            }
            for value in [Value::Null, json!("unknown"), json!(false)] {
                edited = doc.clone();
                edited["status"] = value.clone();
                assert!(matches!(
                    properties.merge(&edited.to_string()),
                    Err(ProjectionError::Json(_))
                ));
                edited = doc.clone();
                edited["header"]["abapLanguageVersion"] = value;
                assert!(matches!(
                    properties.merge(&edited.to_string()),
                    Err(ProjectionError::Json(_))
                ));
            }
        } else {
            for value in [Value::Null, json!("false"), json!(0)] {
                edited = doc.clone();
                edited["editLocked"] = value;
                assert!(matches!(
                    properties.merge(&edited.to_string()),
                    Err(ProjectionError::Json(_))
                ));
            }
        }
        assert_eq!(properties.merge(&doc.to_string()).unwrap(), None);
    }
}

#[test]
fn include_sparse_noops_editor_locks_and_module_placeholder_are_not_invented() {
    for description in [None, Some(""), Some("Original")] {
        let xml = INCLUDE_XML.replace(
            "adtcore:language=\"EN\"",
            &format!(
                "adtcore:language=\"EN\" {}",
                description
                    .map(|text| format!("adtcore:description=\"{text}\""))
                    .unwrap_or_default()
            ),
        );
        let projection =
            project(INCLUDE.snapshot(&INCLUDE.reference(INCLUDE.name, None), &xml)).unwrap();
        let properties = metadata(&projection, INCLUDE_JSON);
        let original = properties
            .subject()
            .typed_properties::<FunctionGroupInclude>()
            .unwrap();
        assert_eq!(original.description.as_deref(), description);
        assert_eq!(original.description_text_limit, None);
        let mut doc = document(properties);
        assert_eq!(
            doc["header"]["description"],
            description.unwrap_or_default()
        );
        assert!(doc.get("editLocked").is_none());
        doc["editLocked"] = json!(false);
        assert_eq!(properties.merge(&doc.to_string()).unwrap(), None);
        doc["editLocked"] = json!(true);
        assert!(matches!(
            properties.merge(&doc.to_string()),
            Err(ProjectionError::UnsupportedAffProperty {
                object_type: "REPS",
                field: "editLocked"
            })
        ));
        doc["editLocked"] = json!(false);
        doc["header"]["description"] = json!(if description == Some("Original") {
            ""
        } else {
            "Changed"
        });
        let mut expected = original.clone();
        expected.description = Some(doc["header"]["description"].as_str().unwrap().to_owned());
        assert_eq!(
            properties.merge(&doc.to_string()).unwrap(),
            Some(serde_json::to_value(expected).unwrap())
        );
        assert_eq!(
            properties
                .subject()
                .typed_properties::<FunctionGroupInclude>()
                .unwrap()
                .description
                .as_deref(),
            description
        );
    }
    for locked in [false, true] {
        let xml = GROUP_XML.replace(
            "lockedByEditor=\"false\"",
            &format!("lockedByEditor=\"{locked}\""),
        );
        let projection = project(GROUP.snapshot(&GROUP.reference(GROUP.name, None), &xml)).unwrap();
        let properties = metadata(&projection, MAIN_JSON);
        let mut doc = document(properties);
        assert_eq!(
            doc.get("editLocked"),
            locked.then_some(json!(true)).as_ref()
        );
        assert_eq!(properties.merge(&doc.to_string()).unwrap(), None);
        doc["editLocked"] = json!(!locked);
        let mut expected = properties.subject().properties().unwrap();
        expected["@group:lockedByEditor"] = json!(!locked);
        assert_eq!(properties.merge(&doc.to_string()).unwrap(), Some(expected));
    }
    let projection = project(MODULE.baseline()).unwrap();
    let properties = metadata(&projection, MODULE_JSON);
    let doc = document(properties);
    assert!(doc.get("includeNumber").is_none());
    for value in ["00", "0", "01", "99"] {
        let mut edited = doc.clone();
        edited["includeNumber"] = json!(value);
        assert!(matches!(
            properties.merge(&edited.to_string()),
            Err(ProjectionError::UnsupportedAffProperty {
                object_type: "FUNC",
                field: "includeNumber"
            })
        ));
    }
    for value in ["", "000", "AB", "\u{0660}"] {
        let mut edited = doc.clone();
        edited["includeNumber"] = json!(value);
        assert!(matches!(
            properties.merge(&edited.to_string()),
            Err(ProjectionError::Validation(_))
        ));
    }
    let mut edited = doc.clone();
    edited["includeNumber"] = json!(0);
    assert!(matches!(
        properties.merge(&edited.to_string()),
        Err(ProjectionError::Json(_))
    ));
    assert_eq!(properties.merge(&doc.to_string()).unwrap(), None);
}

#[derive(Clone, Default)]
struct Script(Arc<Mutex<VecDeque<(AdtRequest, AdtResponse)>>>);

impl Script {
    fn expect(
        &self,
        method: Method,
        uri: &str,
        query: &[(&str, &str)],
        fields: &[(&str, &str)],
        body: impl Into<Vec<u8>>,
        response: AdtResponse,
    ) {
        let request = AdtRequest::from_parts(
            method,
            AdtUri::parse(uri).unwrap(),
            query
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            headers(fields),
            body.into(),
        );
        self.0.lock().unwrap().push_back((request, response));
    }

    fn assert_idle(&self) {
        assert!(
            self.0.lock().unwrap().is_empty(),
            "not all scripted requests were executed"
        );
    }
}

#[async_trait]
impl Transport for Script {
    async fn send(&self, actual: AdtRequest) -> Result<AdtResponse, TransportError> {
        let (expected, response) = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected I/O (including group lookup): {actual:?}"));
        assert_eq!(actual.method(), expected.method());
        assert_eq!(
            actual.target(),
            expected.target(),
            "actual child URI must win over discovery/name"
        );
        assert_eq!(actual.query(), expected.query());
        assert_eq!(actual.headers(), expected.headers());
        assert_eq!(actual.body(), expected.body());
        Ok(response)
    }
}

#[tokio::test]
async fn real_child_queries_and_independent_conditional_writes_never_load_the_group() {
    let script = Script::default();
    for (uri, xml) in [
        (
            "/sap/bc/adt/discovery",
            include_str!("../../zadt/tests/fixtures/discovery.xml"),
        ),
        (
            "/sap/bc/adt/core/discovery",
            include_str!("../../zadt/tests/fixtures/core-discovery.xml"),
        ),
    ] {
        script.expect(
            Method::GET,
            uri,
            &[],
            &[("accept", "application/atomsvc+xml")],
            "",
            response(StatusCode::OK, &[], xml),
        );
    }
    let client = Client::new(script.clone()).discover().await.unwrap();
    let mut projections = Vec::new();
    for fixture in [INCLUDE, MODULE] {
        let reference = fixture.reference(fixture.name, None);
        let xml = fixture.xml.replace(
            "source/main\"",
            "source/main?version=inactive&amp;note=child#body\"",
        );
        script.expect(
            Method::GET,
            fixture.uri,
            &[("version", "inactive")],
            &[("accept", fixture.media), ("cache-control", "no-cache")],
            "",
            response(
                StatusCode::OK,
                &[("content-type", fixture.media), ("etag", fixture.etag)],
                xml,
            ),
        );
        let snapshot = reference
            .query()
            .workbench_version(WorkbenchVersion::Inactive)
            .execute(&client)
            .await
            .unwrap();
        assert!(
            snapshot.key().parent().is_none(),
            "container fallback does not require a group snapshot"
        );
        script.assert_idle();
        let projection = project(snapshot).unwrap();
        for file in projection.files() {
            if let FileBacking::Properties(properties) = file.backing() {
                let baseline = properties.render().unwrap();
                assert_eq!(properties.merge(&baseline).unwrap(), None);
            }
        }
        script.assert_idle();
        projections.push(projection);
    }
    // Both children stay open across both writes so accidentally shared ETags are observable.
    for ((fixture, name, returned_etag), projection) in [
        (INCLUDE, INCLUDE_JSON, "\"include-saved\""),
        (MODULE, MODULE_JSON, "\"module-saved\""),
    ]
    .into_iter()
    .zip(&projections)
    {
        let properties = metadata(projection, name);
        let original = properties.subject().properties().unwrap();
        let mut doc = document(properties);
        doc["header"]["description"] = json!(format!("Edited {}", fixture.name));
        let payload = properties.merge(&doc.to_string()).unwrap().unwrap();
        let mut expected = original.clone();
        expected["@adtcore:description"] = doc["header"]["description"].clone();
        assert_eq!(payload, expected);
        let xml = if fixture.kind == "FUGR/I" {
            serde_json::from_value::<FunctionGroupIncludeProperties>(expected.clone())
                .unwrap()
                .to_xml()
                .unwrap()
        } else {
            serde_json::from_value::<FunctionModuleProperties>(expected.clone())
                .unwrap()
                .to_xml()
                .unwrap()
        };
        script.expect(
            Method::PUT,
            fixture.uri,
            &[],
            &[
                ("accept", fixture.media),
                ("content-type", fixture.media),
                ("if-match", fixture.etag),
            ],
            xml.clone(),
            response(
                StatusCode::OK,
                &[("content-type", fixture.media), ("etag", returned_etag)],
                xml,
            ),
        );
        let result = properties
            .subject()
            .update_if_match(payload)
            .unwrap()
            .execute(&client)
            .await
            .unwrap();
        let PreconditionResult::Success(Some(saved)) = result else {
            panic!("expected returned representation")
        };
        assert_eq!(saved.uri().as_str(), fixture.uri);
        assert_eq!(saved.etag().map(EntityTag::as_str), Some(returned_etag));
        assert_eq!(saved.properties().unwrap(), expected);
        let saved = project(saved).unwrap();
        assert_eq!(document(metadata(&saved, name)), doc);
        assert_eq!(properties.subject().properties().unwrap(), original);
        script.assert_idle();

        let source = projection
            .files()
            .iter()
            .find_map(|file| match file.backing() {
                FileBacking::Source(source) => Some(source),
                _ => None,
            })
            .unwrap();
        assert_eq!(&source.object, properties.subject().reference());
        assert_eq!(source.object.uri().as_str(), fixture.uri);
        let source_uri = format!("{}/source/main", fixture.uri);
        let source_etag = format!("\"{}-source-response\"", fixture.name);
        script.expect(
            Method::GET,
            &source_uri,
            &[("version", "inactive"), ("note", "child")],
            &[("accept", "text/plain")],
            "",
            response(
                StatusCode::OK,
                &[("content-type", "text/plain"), ("etag", &source_etag)],
                "* Untransformed child source\n",
            ),
        );
        let loaded = source.query().execute(&client).await.unwrap();
        assert_eq!(&loaded.reference, source);
        assert_eq!(loaded.reference.object.uri().as_str(), fixture.uri);
        assert_eq!(loaded.content, "* Untransformed child source\n");
        assert_eq!(loaded.etag.as_deref(), Some(source_etag.as_str()));
        assert_ne!(loaded.etag.as_deref(), source.etag.as_deref());
        for (fixture, projection) in [INCLUDE, MODULE].into_iter().zip(&projections) {
            assert_eq!(
                projection.subject().etag().map(EntityTag::as_str),
                Some(fixture.etag)
            );
            assert_eq!(projection.subject().uri().as_str(), fixture.uri);
        }
        script.assert_idle();
    }
}
