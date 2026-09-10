//! Caller-owned orchestration: ZVFS lists objects, ZADT loads/saves resources,
//! and ZAFF projects/renders/merges immutable baselines without any I/O.
//! No HTTP client, credentials, sockets, or production async runtime are used.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use http::{HeaderMap, Method, StatusCode};
use serde_json::Value;
use zadt::{
    AccessMode, AdtRequest, AdtResponse, AdtUri, Client, Discovery, EntityTag, ObjectError,
    ObjectType, Operation, OperationError, PreconditionResult, Program, ProgramProperties,
    RepositoryContentOperation, RepositoryContentQuery, RepositoryPreselection, ToXml, Transport,
    TransportError, WorkbenchVersion, XmlCodec,
};
use zaff::{
    FileBacking, Projection, PropertiesProjection, prog::ProjectedProgramProperties, project,
};
use zvfs::{FacetPolicy, Mount, VirtualRepositoryTree};

// Deliberately not the location derived from Program's discovery collection/name.
const OBJECT_URI: &str = "/sap/bc/adt/custom/programs/Advertised%2FProgram";
const SOURCE_URI: &str = "/sap/bc/adt/custom/programs/Advertised%2FProgram/source/main";
const METADATA_FILE: &str = "z_test.prog.json";
const SOURCE_FILE: &str = "z_test.prog.abap";
const OBJECT_ETAG: &str = "\"properties-1\"";
const SOURCE_ETAG: &str = "\"source-response-1\"";
const SOURCE: &str = "REPORT z_test.\nWRITE / 'original'.\n";
const EDITED_SOURCE: &str = "REPORT z_test.\nWRITE / 'edited'.\n";
const PROGRAM_XML: &[u8] = include_bytes!("../../zadt/tests/fixtures/program-z-test.xml");

#[derive(Clone, Default)]
struct Script {
    steps: Arc<Mutex<VecDeque<(AdtRequest, AdtResponse)>>>,
    sent: Arc<AtomicUsize>,
}

impl Script {
    fn expect(&self, request: AdtRequest, response: AdtResponse) {
        self.steps
            .lock()
            .expect("script mutex is healthy")
            .push_back((request, response));
    }

    fn assert_idle(&self, sent: usize) {
        assert_eq!(
            self.sent.load(Ordering::SeqCst),
            sent,
            "unexpected I/O count"
        );
        assert!(
            self.steps
                .lock()
                .expect("script mutex is healthy")
                .is_empty(),
            "not all scripted requests were executed"
        );
    }
}

#[async_trait]
impl Transport for Script {
    async fn send(&self, actual: AdtRequest) -> Result<AdtResponse, TransportError> {
        self.sent.fetch_add(1, Ordering::SeqCst);
        let (expected, response) = self
            .steps
            .lock()
            .expect("script mutex is healthy")
            .pop_front()
            .unwrap_or_else(|| panic!("unscripted request: {actual:?}"));
        assert_eq!(actual.method(), expected.method(), "request method");
        assert_eq!(actual.target(), expected.target(), "authoritative target");
        assert_eq!(
            actual.query(),
            expected.query(),
            "complete query, including order"
        );
        assert_eq!(actual.headers(), expected.headers(), "complete headers");
        assert_eq!(
            std::str::from_utf8(actual.body()).expect("request body is UTF-8"),
            std::str::from_utf8(expected.body()).expect("expected body is UTF-8"),
            "complete wire body"
        );
        Ok(response)
    }
}

fn headers(values: &[(&str, &str)]) -> HeaderMap {
    values
        .iter()
        .map(|(name, value)| {
            (
                name.parse::<http::header::HeaderName>()
                    .expect("valid test header name"),
                value.parse().expect("valid test header value"),
            )
        })
        .collect()
}

fn request(
    method: Method,
    uri: &str,
    query: &[(&str, &str)],
    fields: &[(&str, &str)],
    body: impl Into<Vec<u8>>,
) -> AdtRequest {
    AdtRequest::from_parts(
        method,
        AdtUri::parse(uri).expect("valid test URI"),
        query
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        headers(fields),
        body.into(),
    )
}

fn response(status: StatusCode, fields: &[(&str, &str)], body: impl Into<Vec<u8>>) -> AdtResponse {
    AdtResponse::new(status, headers(fields), body.into())
}

fn program() -> ProgramProperties {
    let mut properties =
        ProgramProperties::from_xml(PROGRAM_XML).expect("ZADT program fixture decodes");
    // Select the source version explicitly: a properties snapshot's version
    // does not make a queryless source href select that same version.
    let href = "source/main?version=inactive";
    properties.source_uri = href.to_owned();
    for link in &mut properties.links {
        if link.relation.as_deref() == Some("http://www.sap.com/adt/relations/source") {
            link.href = href.to_owned();
        }
    }
    properties
}

fn metadata(projection: &Projection) -> &PropertiesProjection {
    let file = projection
        .file(METADATA_FILE)
        .expect("program advertises AFF metadata");
    let FileBacking::Properties(properties) = file.backing() else {
        panic!("AFF JSON must be backed by retained properties, not source");
    };
    properties
}

fn edited_metadata(properties: &PropertiesProjection) -> String {
    let mut document: ProjectedProgramProperties =
        serde_json::from_str(&properties.render().expect("baseline renders as AFF"))
            .expect("rendered document is an AFF program");
    assert_eq!(document.header.description, "dwadwad");
    document.header.description = "Edited in the LSP".to_owned();
    serde_json::to_string(&document).expect("edited AFF serializes")
}

fn expect_properties_query(script: &Script, properties: &ProgramProperties, etag: Option<&str>) {
    let mut fields = vec![("content-type", Program::MEDIA_TYPES[1])];
    if let Some(etag) = etag {
        fields.push(("etag", etag));
    }
    script.expect(
        request(
            Method::GET,
            OBJECT_URI,
            &[("version", "inactive")],
            &[
                ("accept", &Program::MEDIA_TYPES.as_slice().join(", ")),
                ("cache-control", "no-cache"),
            ],
            "",
        ),
        response(
            StatusCode::OK,
            &fields,
            properties.to_xml().expect("fixture serializes"),
        ),
    );
}

async fn open_program(script: &Script, etag: Option<&str>) -> (Client<Discovery>, Projection) {
    for (uri, xml) in [
        (
            "/sap/bc/adt/discovery",
            include_bytes!("../../zadt/tests/fixtures/discovery.xml").as_slice(),
        ),
        (
            "/sap/bc/adt/core/discovery",
            include_bytes!("../../zadt/tests/fixtures/core-discovery.xml").as_slice(),
        ),
    ] {
        script.expect(
            request(
                Method::GET,
                uri,
                &[],
                &[("accept", "application/atomsvc+xml")],
                "",
            ),
            response(StatusCode::OK, &[], xml),
        );
    }
    let client = Client::new(script.clone())
        .discover()
        .await
        .expect("scripted discovery succeeds");
    script.expect(
        request(
            Method::GET,
            "/sap/bc/adt/repository/informationsystem/virtualfolders/facets",
            &[],
            &[],
            "",
        ),
        response(
            StatusCode::OK,
            &[],
            include_bytes!("../../zadt/tests/fixtures/repository-facets.xml").as_slice(),
        ),
    );
    let selection = RepositoryPreselection::directly_assigned("$TMP");
    let tree = VirtualRepositoryTree::builder(client.clone())
        .mount(
            Mount::selection("Local Objects", [selection.clone()])
                .facet_policy(FacetPolicy::flat()),
        )
        .build()
        .await
        .expect("flat tree builds from advertised RIS facets");
    let mounts = tree
        .children(tree.root())
        .await
        .expect("root lists configured mounts");
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].label, "Local Objects");
    script.assert_idle(3);

    // Compare ZVFS's request against an explicit public ADT query for this selection.
    let listing = RepositoryContentQuery::builder()
        .preselection(selection)
        .ignore_short_descriptions(false)
        .operation(RepositoryContentOperation::Expand)
        .build()
        .expect("explicit flat listing is valid")
        .encode(client.discovery())
        .expect("RIS collection is advertised")
        .into_parts()
        .0;
    script.expect(listing, response(StatusCode::OK, &[], format!(
        r#"<vfs:virtualFoldersResult xmlns:vfs="http://www.sap.com/adt/ris/virtualFolders" objectCount="1">
            <vfs:object name="Z_TEST" package="$TMP" type="PROG/P" uri="{OBJECT_URI}"
                expandable="false" text="Listing is not a properties baseline" />
        </vfs:virtualFoldersResult>"#,
    )));
    let children = tree
        .children(mounts[0].id)
        .await
        .expect("RIS lists the program");
    assert_eq!(children.len(), 1);
    assert!(
        !children[0].is_directory(),
        "ZVFS objects remain leaves, not AFF directories"
    );
    let entry = tree
        .object_entry(children[0].id)
        .expect("object leaf retains its ADT entry");
    let reference = entry.object();
    assert_eq!(reference.uri().as_str(), OBJECT_URI);
    assert_eq!(reference.workbench_type(), &Program::WORKBENCH_TYPE);
    assert_eq!(
        tree.children(mounts[0].id).await.expect("cached listing"),
        children
    );
    script.assert_idle(4);

    expect_properties_query(script, &program(), etag);
    let snapshot = reference
        .query()
        .workbench_version(WorkbenchVersion::Inactive)
        .execute(&client)
        .await
        .expect("advertised reference loads runtime properties");
    assert_eq!(snapshot.reference(), &reference);
    assert_eq!(snapshot.etag().map(EntityTag::as_str), etag);
    assert_eq!(
        snapshot.media_type(),
        Program::MEDIA_TYPES[1],
        "server chose the older supported codec"
    );
    let projection = project(snapshot).expect("owned runtime snapshot projects");
    assert_eq!(
        projection
            .files()
            .iter()
            .map(|file| file.name())
            .collect::<Vec<_>>(),
        [METADATA_FILE, SOURCE_FILE]
    );
    let properties = metadata(&projection);
    assert!(std::ptr::eq(properties.subject(), projection.subject()));
    let rendered = properties
        .render()
        .expect("metadata file reads without I/O");
    assert!(rendered.ends_with('\n'));
    let merged = properties.merge(&rendered).expect("no-op AFF merge");
    // No save path means no writes, locks, or refetches. Any request is unscripted.
    if let Some(payload) = merged.as_ref() {
        properties
            .subject()
            .update_if_match(payload.clone())
            .expect("only changed properties need an ETag")
            .execute(&client)
            .await
            .expect("only changed properties are saved");
    }
    assert_eq!(merged, None, "unchanged AFF has no update payload");
    let expected = serde_json::to_value(program()).expect("complete fixture wire fields serialize");
    assert_eq!(
        properties
            .subject()
            .properties()
            .expect("runtime wire properties"),
        expected,
        "no-op retains links, package, syntax configuration, timestamps, and all other wire fields"
    );
    assert!(projection.file("other.prog.json").is_none());
    // project/render/merge/file have no transport; no source was eagerly fetched.
    script.assert_idle(5);
    (client, projection)
}

fn expect_metadata_save(script: &Script, result: AdtResponse) -> Value {
    let mut expected = program();
    expected.description = "Edited in the LSP".to_owned();
    script.expect(
        request(
            Method::PUT,
            OBJECT_URI,
            &[],
            &[
                ("accept", Program::MEDIA_TYPES[1]),
                ("content-type", Program::MEDIA_TYPES[1]),
                ("if-match", OBJECT_ETAG),
            ],
            expected
                .to_xml()
                .expect("complete expected update serializes"),
        ),
        result,
    );
    serde_json::to_value(expected).expect("expected update has wire-shaped JSON")
}

#[tokio::test]
async fn metadata_save_reprojects_returned_or_explicitly_refetched_representation() {
    for returns_representation in [true, false] {
        let script = Script::default();
        let (client, projection) = open_program(&script, Some(OBJECT_ETAG)).await;
        let original = metadata(&projection).clone();
        let baseline = original.render().expect("original metadata reads");
        let edited = edited_metadata(&original);
        let merged = original
            .merge(&edited)
            .expect("editor changes merge into baseline")
            .expect("changed description produces an update payload");
        script.assert_idle(5);

        // Server normalization must win over the submitted editor buffer.
        let mut saved = program();
        saved.description = "Server-confirmed description".to_owned();
        saved.changed_by = "OTHER_EDITOR".into();
        let result = if returns_representation {
            response(
                StatusCode::OK,
                &[
                    ("content-type", Program::MEDIA_TYPES[1]),
                    ("etag", "\"properties-2\""),
                ],
                saved.to_xml().expect("saved fixture serializes"),
            )
        } else {
            response(StatusCode::NO_CONTENT, &[("etag", "\"save-ack-only\"")], "")
        };
        assert_eq!(merged, expect_metadata_save(&script, result));
        let outcome = original
            .subject()
            .update_if_match(merged)
            .expect("baseline ETag guards save")
            .execute(&client)
            .await
            .expect("save response decodes");
        script.assert_idle(6);
        let fresh = match outcome {
            PreconditionResult::Success(Some(snapshot)) => {
                assert!(
                    returns_representation,
                    "empty success must not invent a snapshot"
                );
                snapshot
            }
            PreconditionResult::Success(None) => {
                assert!(
                    !returns_representation,
                    "returned representation must be decoded"
                );
                assert_eq!(
                    original.render().expect("old baseline remains readable"),
                    baseline
                );
                expect_properties_query(&script, &saved, Some("\"properties-2\""));
                original
                    .subject()
                    .reference()
                    .query()
                    .workbench_version(original.subject().workbench_version())
                    .execute(&client)
                    .await
                    .expect("caller explicitly refetches after empty success")
            }
            other => panic!("expected successful save, got {other:?}"),
        };
        let refreshed = project(fresh).expect("caller reprojects confirmed server state");
        let current = metadata(&refreshed);
        assert_eq!(current.subject().reference().uri().as_str(), OBJECT_URI);
        assert_eq!(
            current.subject().etag().map(EntityTag::as_str),
            Some("\"properties-2\"")
        );
        assert_eq!(
            current
                .subject()
                .properties()
                .expect("fresh wire properties"),
            serde_json::to_value(&saved).expect("saved wire fields")
        );
        let mut document: ProjectedProgramProperties =
            serde_json::from_str(&current.render().expect("fresh metadata reads"))
                .expect("fresh AFF parses");
        assert_eq!(document.header.description, "Server-confirmed description");
        assert!(!std::ptr::eq(current.subject(), original.subject()));

        // A second editor save must start from properties-2, not the original
        // projection or the buffer submitted before server normalization.
        let general = document
            .general_information
            .as_mut()
            .expect("fixture represents arithmetic settings");
        assert!(general.fix_point_arithmetic);
        general.fix_point_arithmetic = false;
        let second_edit = serde_json::to_string(&document).expect("second AFF edit serializes");
        let merged = current
            .merge(&second_edit)
            .expect("second edit merges against refreshed baseline")
            .expect("changed arithmetic produces an update payload");
        let mut expected = saved.clone();
        expected.fix_point_arithmetic = false;
        let expected_wire = serde_json::to_value(&expected).expect("second save wire fields");
        assert_eq!(
            merged, expected_wire,
            "only arithmetic changes; normalized description and changed_by survive"
        );
        script.assert_idle(if returns_representation { 6 } else { 7 });
        script.expect(
            request(
                Method::PUT,
                OBJECT_URI,
                &[],
                &[
                    ("accept", Program::MEDIA_TYPES[1]),
                    ("content-type", Program::MEDIA_TYPES[1]),
                    ("if-match", "\"properties-2\""),
                ],
                expected
                    .to_xml()
                    .expect("complete second update serializes"),
            ),
            response(
                StatusCode::OK,
                &[
                    ("content-type", Program::MEDIA_TYPES[1]),
                    ("etag", "\"properties-3\""),
                ],
                expected
                    .to_xml()
                    .expect("second saved representation serializes"),
            ),
        );
        let outcome = current
            .subject()
            .update_if_match(merged)
            .expect("refreshed ETag guards second save")
            .execute(&client)
            .await
            .expect("second save response decodes");
        let PreconditionResult::Success(Some(second_saved)) = outcome else {
            panic!("second save must return its confirmed representation, got {outcome:?}");
        };
        assert_eq!(second_saved.reference().uri().as_str(), OBJECT_URI);
        assert_eq!(
            second_saved.etag().map(EntityTag::as_str),
            Some("\"properties-3\"")
        );
        assert_eq!(
            second_saved
                .properties()
                .expect("second confirmed wire properties"),
            expected_wire
        );
        assert_eq!(
            current.subject().etag().map(EntityTag::as_str),
            Some("\"properties-2\"")
        );
        assert_eq!(
            current
                .subject()
                .properties()
                .expect("second save does not mutate its baseline"),
            serde_json::to_value(saved).expect("first server-confirmed wire fields")
        );
        drop(projection);
        assert_eq!(
            original.subject().etag().map(EntityTag::as_str),
            Some(OBJECT_ETAG)
        );
        assert_eq!(
            original
                .render()
                .expect("clone retains original baseline after save and drop"),
            baseline
        );
        assert_eq!(
            original
                .merge(&baseline)
                .expect("old no-op remains unchanged"),
            None,
            "no-op compares with the retained baseline, not confirmed remote changes"
        );
        assert_eq!(
            original
                .subject()
                .properties()
                .expect("old no-op retains original wire properties"),
            serde_json::to_value(program()).expect("original wire fields")
        );
        script.assert_idle(if returns_representation { 7 } else { 8 });
    }
}

#[tokio::test]
async fn conditional_conflict_does_not_advance_the_editor_baseline() {
    let script = Script::default();
    let (client, projection) = open_program(&script, Some(OBJECT_ETAG)).await;
    let properties = metadata(&projection);
    let baseline = properties.render().expect("original metadata reads");
    let edited = edited_metadata(properties);
    let merged = properties
        .merge(&edited)
        .expect("editor changes merge locally")
        .expect("changed description produces an update payload");
    script.assert_idle(5);
    assert_eq!(
        merged,
        expect_metadata_save(
            &script,
            response(
                StatusCode::PRECONDITION_FAILED,
                &[("etag", "\"concurrent-properties\"")],
                "",
            )
        )
    );
    let outcome = properties
        .subject()
        .update_if_match(merged.clone())
        .expect("guarded operation builds")
        .execute(&client)
        .await
        .expect("412 is a conditional result, not a decode error");
    assert!(
        matches!(outcome, PreconditionResult::Failed { etag } if etag.as_deref() == Some("\"concurrent-properties\""))
    );
    assert_eq!(properties.subject().reference().uri().as_str(), OBJECT_URI);
    assert_eq!(
        properties.subject().etag().map(EntityTag::as_str),
        Some(OBJECT_ETAG)
    );
    assert_eq!(
        properties.render().expect("conflict retains baseline"),
        baseline
    );
    assert_eq!(
        properties
            .merge(&edited)
            .expect("dirty buffer remains mergeable against original")
            .expect("conflict does not turn the dirty buffer into a no-op"),
        merged
    );
    assert_eq!(
        properties
            .subject()
            .properties()
            .expect("original wire properties"),
        serde_json::to_value(program()).expect("fixture wire fields")
    );
    // No automatic retry/refetch, and never adopt the conflict ETag for this baseline.
    let retry = properties
        .subject()
        .update_if_match(merged)
        .expect("caller could retry old baseline")
        .encode(client.discovery())
        .expect("retry encodes locally");
    assert_eq!(retry.headers()["if-match"], OBJECT_ETAG);
    script.assert_idle(6);
}

#[tokio::test]
async fn missing_properties_etag_skips_noop_save_but_rejects_changes_without_io() {
    let script = Script::default();
    let (client, projection) = open_program(&script, None).await;
    let properties = metadata(&projection);
    let baseline = properties.render().expect("original metadata reads");
    let mut document: Value = serde_json::from_str(&baseline).expect("baseline AFF parses");
    let formatting_only = serde_json::to_string(&document).expect("compact AFF serializes");
    assert!(document["generalInformation"].get("programType").is_none());
    document["generalInformation"]["programType"] = "executableProgram".into();
    let explicit_default = serde_json::to_string(&document).expect("explicit default serializes");
    for edited in [formatting_only, explicit_default] {
        assert_ne!(edited, baseline, "editor buffer differs from rendered AFF");
        let merged = properties.merge(&edited).expect("no-op edit validates");
        if let Some(payload) = merged.as_ref() {
            properties
                .subject()
                .update_if_match(payload.clone())
                .expect("only a real edit should require the missing ETag")
                .execute(&client)
                .await
                .expect("only changed properties are saved");
        }
        assert_eq!(merged, None, "equivalent typed properties need no save");
        assert_eq!(
            properties
                .subject()
                .properties()
                .expect("no-op retains original wire properties"),
            serde_json::to_value(program()).expect("original wire fields")
        );
        script.assert_idle(5);
    }
    let merged = properties
        .merge(&edited_metadata(properties))
        .expect("merge does not require a validator")
        .expect("a real edit still produces an update payload without an ETag");
    assert!(matches!(
        properties.subject().update_if_match(merged),
        Err(ObjectError::MissingEntityTag)
    ));
    script.assert_idle(5);
}

#[tokio::test]
async fn source_save_rereads_under_the_lock_and_aborts_on_intervening_changes() {
    for (baseline_etag, locked_content, expect_save) in [
        (Some(SOURCE_ETAG), SOURCE, true),
        (None, "REPORT z_test.\nWRITE / 'concurrent edit'.\n", false),
    ] {
        let script = Script::default();
        let (client, projection) = open_program(&script, Some(OBJECT_ETAG)).await;
        let FileBacking::Source(source) = projection
            .file(SOURCE_FILE)
            .expect("source file exists")
            .backing()
        else {
            panic!("ABAP text must be backed by SourceRef");
        };
        assert_eq!(source.object, *projection.subject().reference());
        assert_eq!(source.uri.as_str(), SOURCE_URI);
        assert_eq!(source.fragment, None);
        assert_eq!(source.etag.as_deref(), Some("202607251959580001"));
        let mut fields = vec![("content-type", "text/plain; charset=utf-8")];
        if let Some(etag) = baseline_etag {
            fields.push(("etag", etag));
        }
        script.expect(
            request(
                Method::GET,
                SOURCE_URI,
                &[("version", "inactive")],
                &[("accept", "text/plain")],
                "",
            ),
            response(StatusCode::OK, &fields, SOURCE),
        );
        let baseline = source
            .query()
            .execute(&client)
            .await
            .expect("source file explicitly loads");
        assert_eq!(baseline.content, SOURCE);
        assert_eq!(&baseline.reference, source);
        assert_eq!(
            baseline.etag.as_deref(),
            baseline_etag,
            "missing response ETag must not borrow the advertised/object tag"
        );
        script.assert_idle(6);
        let session = client.create_user_session();
        script.expect(
            request(
                Method::POST,
                OBJECT_URI,
                &[("_action", "LOCK"), ("accessMode", "MODIFY")],
                &[
                    (
                        "accept",
                        concat!(
                            "application/vnd.sap.as+xml; charset=utf-8; ",
                            "dataname=com.sap.adt.lock.Result2"
                        ),
                    ),
                    ("x-sap-adt-sessiontype", "stateful"),
                ],
                "",
            ),
            response(
                StatusCode::OK,
                &[(
                    "set-cookie",
                    "sap-contextid=SCRIPTED-SESSION; Path=/sap/bc/adt",
                )],
                include_bytes!("../../zadt/tests/fixtures/object-lock.xml").as_slice(),
            ),
        );
        let lock = projection
            .subject()
            .lock(AccessMode::Modify)
            .execute(&session)
            .await
            .expect("real public lock operation issues session-bound lock");
        assert_eq!(lock.object(), &source.object);
        assert_eq!(lock.handle(), "LOCK-HANDLE-1");
        assert_eq!(lock.access_mode(), AccessMode::Modify);
        script.assert_idle(7);
        // Acquiring a lock does not validate the pre-lock editor baseline. The
        // caller rereads the same source version while holding that session's lock.
        script.expect(
            request(
                Method::GET,
                SOURCE_URI,
                &[("version", "inactive")],
                &[
                    ("accept", "text/plain"),
                    ("x-sap-adt-sessiontype", "stateful"),
                    ("cookie", "sap-contextid=SCRIPTED-SESSION"),
                ],
                "",
            ),
            response(
                StatusCode::OK,
                &[
                    ("content-type", "text/plain; charset=utf-8"),
                    ("etag", "\"source-under-lock\""),
                ],
                locked_content,
            ),
        );
        let current = baseline
            .reference
            .query()
            .execute(&session)
            .await
            .expect("caller rereads under the acquired lock");
        assert_eq!(current.reference, baseline.reference);
        assert_eq!(current.content, locked_content);
        assert_eq!(current.etag.as_deref(), Some("\"source-under-lock\""));
        script.assert_idle(8);
        if expect_save {
            // This checks existing ZADT PUT parameters: lockHandle (and transport when
            // supplied), not GET's version selector. It does not claim arbitrary source
            // query selectors can safely be dropped or are supported by source updates.
            script.expect(
                request(
                    Method::PUT,
                    SOURCE_URI,
                    &[("lockHandle", "LOCK-HANDLE-1")],
                    &[
                        ("content-type", "text/plain; charset=utf-8"),
                        ("x-sap-adt-sessiontype", "stateful"),
                        ("cookie", "sap-contextid=SCRIPTED-SESSION"),
                    ],
                    EDITED_SOURCE,
                ),
                response(
                    StatusCode::OK,
                    &[("etag", "\"source-response-2\"")],
                    EDITED_SOURCE,
                ),
            );
        }
        let saved = if current.content == baseline.content {
            let update = current
                .reference
                .update_with_lock(EDITED_SOURCE, lock.clone())
                .expect("owning object lock permits source update");
            let other_session = client.create_user_session();
            assert!(matches!(
                update.execute(&other_session).await,
                Err(OperationError::UserSessionMismatch)
            ));
            other_session
                .close()
                .await
                .expect("unused session closes locally");
            let saved = update
                .execute(&session)
                .await
                .expect("source save uses lock's actual session");
            assert_eq!(&saved.reference, source);
            assert_eq!(saved.content.as_deref(), Some(EDITED_SOURCE));
            assert_eq!(saved.etag.as_deref(), Some("\"source-response-2\""));
            Some(saved)
        } else {
            // Conflict belongs to the caller: retain the baseline/dirty buffer and
            // abort without PUT, but still release the lock and session below.
            None
        };
        assert_eq!(
            saved.is_some(),
            expect_save,
            "save only when the locked reread matches the retained baseline"
        );
        assert_eq!(baseline.content, SOURCE);
        assert_eq!(baseline.etag.as_deref(), baseline_etag);
        script.assert_idle(if expect_save { 9 } else { 8 });
        assert_eq!(
            projection.subject().etag().map(EntityTag::as_str),
            Some(OBJECT_ETAG)
        );
        assert_eq!(source.etag.as_deref(), Some("202607251959580001"));
        script.expect(
            request(
                Method::POST,
                OBJECT_URI,
                &[("_action", "UNLOCK"), ("lockHandle", "LOCK-HANDLE-1")],
                &[
                    ("x-sap-adt-sessiontype", "stateful"),
                    ("cookie", "sap-contextid=SCRIPTED-SESSION"),
                ],
                "",
            ),
            response(StatusCode::OK, &[], ""),
        );
        lock.remove()
            .execute(&session)
            .await
            .expect("caller releases its lock");
        script.expect(
            request(
                Method::GET,
                "/sap/bc/adt/core/discovery",
                &[],
                &[
                    ("x-sap-adt-sessiontype", "stateless"),
                    ("cookie", "sap-contextid=SCRIPTED-SESSION"),
                ],
                "",
            ),
            response(StatusCode::OK, &[], ""),
        );
        session
            .close()
            .await
            .expect("caller closes established session");
        script.assert_idle(if expect_save { 11 } else { 10 });
    }
}
