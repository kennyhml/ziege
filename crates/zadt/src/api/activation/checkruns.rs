use std::borrow::Cow;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use http::{Method, StatusCode};
use serde::{Deserialize, Serialize, Serializer};

use crate::{
    AdtUri, AdvertisedLink, CategoryId, Discovery, EncodeError, EncodedOperation, ObjectError,
    ObjectKey, ObjectRef, Operation, OperationResponse, RequiresDiscovery, ResponseError,
    Stateless, WorkbenchVersion, objects::ObjectTarget,
};

/// Retrieves the check-run reporters advertised by the backend.
///
/// This does not seem to be an exhaustive list, as reporters not advertised,
/// such as the extended check run reporters, still seem to function.
///
/// However, this does allow to dynamically check backend reporter compatibility
/// and kernel release versioning. The advertised check runners also contain a
/// list of object types which they are valid for.
#[derive(Debug, Default)]
pub struct CheckRunReportersQuery;

impl CheckRunReportersQuery {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/check",
        term: "reporters",
    };

    pub fn new() -> Self {
        Self
    }
}

impl Operation for CheckRunReportersQuery {
    type Kind = Stateless;
    type Response = SupportedCheckReporters;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::GET, target);
        request.set_accept(SupportedCheckReporterList::MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[SupportedCheckReporterList::MEDIA_TYPE])?;
        serde_xml_rs::from_reader::<SupportedCheckReporterList, _>(response.body())
            .map(|reporters| reporters.entries)
            .map_err(ObjectError::InvalidResponse)
            .map_err(Into::into)
    }
}

/// Executes checks on a set of objects with the given reporters.
///
/// Multiple reporters can be specified. The available reporters can be
/// queried using [`CheckRunReportersQuery`]. Common reporters are
/// specified as constants in [`CheckRunReporter`], but these cannot account
/// for release-based check runners or custom enhancements.
///
/// Backend handler: `CL_SEU_ADT_RES_CHECK_RUN`
#[derive(Debug, Default)]
pub struct ObjectCheckRun {
    /// A list of objects to check
    objects: CheckObjectList,
    /// A list of reporters to run on the objects, can be omitted for a backend default
    reporters: Vec<CheckRunReporter>,
}

impl ObjectCheckRun {
    const CATEGORY: CategoryId = CategoryId {
        scheme: "http://www.sap.com/adt/categories/check",
        term: "checkruns",
    };

    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an object to be checked by all given reporters.
    ///
    /// The relevant messages must then be extracted from the check report response.
    pub fn push_object(&mut self, object: CheckRunObject) -> &mut Self {
        self.objects.objects.push(object);
        self
    }

    /// Adds a reporter to execute its checks.
    pub fn push_reporter(&mut self, reporter: CheckRunReporter) -> &mut Self {
        self.reporters.push(reporter);
        self
    }

    /// Adds multiple reporters, including a discovered reporter collection.
    pub fn extend_reporters<I>(&mut self, reporters: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: Into<CheckRunReporter>,
    {
        self.reporters.extend(reporters.into_iter().map(Into::into));
        self
    }
}

impl Operation for ObjectCheckRun {
    type Kind = Stateless;
    type Response = CheckRunReports;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let body = self.objects.serialize(resolver)?;
        let target = resolver.require_collection_target(Self::CATEGORY)?;
        let mut request = EncodedOperation::new(Method::POST, target);
        for reporter in &self.reporters {
            request.push_query("reporters", reporter.as_str());
        }
        request.set_accept(CheckRunReports::MEDIA_TYPE);
        request.set_content_type(CheckObjectList::MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[CheckRunReports::MEDIA_TYPE])?;
        serde_xml_rs::from_reader(response.body())
            .map_err(ObjectError::InvalidResponse)
            .map_err(Into::into)
    }
}

/// A check run reporter, such as `abapCheckRun`.
///
/// Reporters are defined in `IF_ADT_CHECK_REPORTER`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CheckRunReporter(Cow<'static, str>);

impl CheckRunReporter {
    pub const PACKAGE_CHECK_RUNNER: Self = Self(Cow::Borrowed("abapPackageCheck"));
    pub const EXTENDED_CHECK_RUNNER: Self = Self(Cow::Borrowed("abapExtendedProgramCheck"));
    pub const SYNTAX_CHECK_RUNNER: Self = Self(Cow::Borrowed("abapCheckRun"));
    pub const DDIC_TABLE_CHECK_RUNNER: Self = Self(Cow::Borrowed("tableStatusCheck"));

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Check-run reporters advertised by the backend.
pub type SupportedCheckReporters = Vec<SupportedCheckReporter>;

#[derive(Debug, Deserialize)]
#[serde(rename = "chkrun:checkReporters", deny_unknown_fields)]
struct SupportedCheckReporterList {
    #[serde(rename = "chkrun:reporter", default)]
    entries: SupportedCheckReporters,
}

impl SupportedCheckReporterList {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.reporters+xml";
}

/// One advertised check-run reporter and its supported object types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SupportedCheckReporter {
    #[serde(rename = "@chkrun:name")]
    pub name: CheckRunReporter,

    #[serde(rename = "chkrun:supportedType", default)]
    pub supported_types: Vec<String>,
}

impl From<SupportedCheckReporter> for CheckRunReporter {
    fn from(reporter: SupportedCheckReporter) -> Self {
        reporter.name
    }
}

impl From<&SupportedCheckReporter> for CheckRunReporter {
    fn from(reporter: &SupportedCheckReporter) -> Self {
        reporter.name.clone()
    }
}

#[derive(Debug, Default)]
struct CheckObjectList {
    objects: Vec<CheckRunObject>,
}

impl CheckObjectList {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.checkobjects+xml";
    const CHECK_RUN_NAMESPACE: &str = "http://www.sap.com/adt/checkrun";
    const ADT_CORE_NAMESPACE: &str = "http://www.sap.com/adt/core";

    fn serialize(&self, resolver: &Discovery) -> Result<String, EncodeError> {
        let objects = self
            .objects
            .iter()
            .map(|object| -> Result<EncodedCheckRunObject<'_>, EncodeError> {
                Ok(EncodedCheckRunObject {
                    uri: object.object.resolve_uri(resolver)?,
                    version: object.version,
                    artifacts: (!object.artifacts.is_empty()).then_some(&object.artifacts),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        serde_xml_rs::SerdeXml::new()
            .namespace("chkrun", Self::CHECK_RUN_NAMESPACE)
            .namespace("adtcore", Self::ADT_CORE_NAMESPACE)
            .to_string(&EncodedCheckObjectList { objects })
            .map_err(ObjectError::InvalidRequest)
            .map_err(Into::into)
    }
}

/// One repository object included in a check run.
#[derive(Debug)]
pub struct CheckRunObject {
    object: ObjectTarget<()>,
    version: WorkbenchVersion,
    artifacts: CheckRunArtifacts,
}

impl CheckRunObject {
    pub fn new<T>(object: &ObjectKey<T>, version: WorkbenchVersion) -> Self {
        Self {
            object: object.erase().into(),
            version,
            artifacts: CheckRunArtifacts::default(),
        }
    }

    /// Creates a check entry preserving the advertised object URI.
    pub fn from_ref<T>(object: &ObjectRef<T>, version: WorkbenchVersion) -> Self {
        Self {
            object: object.erase().into(),
            version,
            artifacts: CheckRunArtifacts::default(),
        }
    }

    #[must_use]
    pub fn artifact(mut self, artifact: CheckRunArtifact) -> Self {
        self.artifacts.entries.push(artifact);
        self
    }
}

#[derive(Serialize)]
#[serde(rename = "chkrun:checkObjectList")]
struct EncodedCheckObjectList<'a> {
    #[serde(rename = "chkrun:checkObject")]
    objects: Vec<EncodedCheckRunObject<'a>>,
}

#[derive(Serialize)]
struct EncodedCheckRunObject<'a> {
    #[serde(rename = "@adtcore:uri")]
    uri: AdtUri,

    #[serde(rename = "@chkrun:version")]
    version: WorkbenchVersion,

    #[serde(rename = "chkrun:artifacts", skip_serializing_if = "Option::is_none")]
    artifacts: Option<&'a CheckRunArtifacts>,
}

#[derive(Debug, Default, Serialize)]
struct CheckRunArtifacts {
    #[serde(rename = "chkrun:artifact")]
    entries: Vec<CheckRunArtifact>,
}

impl CheckRunArtifacts {
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Unsaved content supplied for one checkable object artifact.
#[derive(Debug, Serialize)]
pub struct CheckRunArtifact {
    #[serde(rename = "@chkrun:uri")]
    uri: AdtUri,

    #[serde(rename = "@chkrun:contentType")]
    content_type: String,

    #[serde(rename = "chkrun:content", serialize_with = "serialize_base64")]
    content: Vec<u8>,
}

impl CheckRunArtifact {
    pub fn new(uri: AdtUri, content_type: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            uri,
            content_type: content_type.into(),
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "chkrun:checkRunReports", deny_unknown_fields)]
pub struct CheckRunReports {
    #[serde(rename = "chkrun:checkReport", default)]
    pub reports: Vec<CheckRunReport>,
}

impl CheckRunReports {
    const MEDIA_TYPE: &str = "application/vnd.sap.adt.checkmessages+xml";
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CheckRunReport {
    #[serde(rename = "@chkrun:reporter")]
    pub reporter: CheckRunReporter,

    #[serde(rename = "@chkrun:triggeringUri")]
    pub triggering_uri: AdtUri,

    #[serde(rename = "@chkrun:status")]
    pub status: String,

    #[serde(rename = "@chkrun:statusText")]
    pub status_text: String,

    #[serde(rename = "chkrun:checkMessageList", default)]
    pub messages: CheckRunMessageList,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CheckRunMessageList {
    #[serde(rename = "chkrun:checkMessage", default)]
    pub messages: Vec<CheckRunMessage>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CheckRunMessage {
    #[serde(rename = "@chkrun:uri")]
    pub uri: String,

    #[serde(rename = "@chkrun:type")]
    pub message_type: String,

    #[serde(rename = "@chkrun:shortText")]
    pub short_text: String,

    #[serde(rename = "@chkrun:category", default)]
    pub category: Option<String>,

    #[serde(rename = "@chkrun:code", default)]
    pub code: Option<String>,

    #[serde(rename = "@chkrun:line", default)]
    pub line: Option<u32>,

    #[serde(rename = "@chkrun:column", default)]
    pub column: Option<u32>,

    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,

    #[serde(rename = "chkrun:t100Key", default)]
    pub t100_key: Option<CheckRunT100Key>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CheckRunT100Key {
    #[serde(rename = "@chkrun:msgid")]
    pub message_class: String,

    #[serde(rename = "@chkrun:msgno")]
    pub message_number: String,

    #[serde(rename = "@chkrun:msgv1", default)]
    pub variable_1: String,

    #[serde(rename = "@chkrun:msgv2", default)]
    pub variable_2: String,

    #[serde(rename = "@chkrun:msgv3", default)]
    pub variable_3: String,

    #[serde(rename = "@chkrun:msgv4", default)]
    pub variable_4: String,
}

fn serialize_base64<S>(content: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtRequest, AdtResponse, Client, Program, TransportError};
    use async_trait::async_trait;
    use http::{HeaderMap, header};

    const DISCOVERY_XML: &[u8] = br#"
        <app:service xmlns:app="http://www.w3.org/2007/app"
                xmlns:atom="http://www.w3.org/2005/Atom">
            <app:workspace>
                <atom:title>Checks</atom:title>
                <app:collection href="/sap/bc/adt/checkruns">
                    <atom:category scheme="http://www.sap.com/adt/categories/check"
                        term="checkruns" />
                </app:collection>
                <app:collection href="/sap/bc/adt/checkruns/reporters">
                    <atom:category scheme="http://www.sap.com/adt/categories/check"
                        term="reporters" />
                </app:collection>
                <app:collection href="/sap/bc/adt/programs/programs">
                    <atom:category scheme="http://www.sap.com/adt/categories/programs"
                        term="programs" />
                </app:collection>
            </app:workspace>
        </app:service>
    "#;

    const CHECK_REPORTS_XML: &[u8] = br#"
        <chkrun:checkRunReports xmlns:chkrun="http://www.sap.com/adt/checkrun">
            <chkrun:checkReport chkrun:reporter="abapCheckRun"
                    chkrun:triggeringUri="/sap/bc/adt/programs/programs/z_test"
                    chkrun:status="processed" chkrun:statusText="Object Z_TEST has been checked">
                <chkrun:checkMessageList>
                    <chkrun:checkMessage
                        chkrun:uri="/sap/bc/adt/programs/programs/z_test/source/main#start=2,1;end=2,4"
                        chkrun:type="E" chkrun:shortText="Syntax error"
                        chkrun:category="Syntax" chkrun:code="SYNTAX(001)" />
                </chkrun:checkMessageList>
            </chkrun:checkReport>
        </chkrun:checkRunReports>
    "#;
    const CHECK_REPORTERS_XML: &[u8] = br#"
        <chkrun:checkReporters xmlns:chkrun="http://www.sap.com/adt/checkrun">
            <chkrun:reporter chkrun:name="abapCheckRun">
                <chkrun:supportedType>CLAS*</chkrun:supportedType>
                <chkrun:supportedType>PROG*</chkrun:supportedType>
            </chkrun:reporter>
            <chkrun:reporter chkrun:name="abapCheckRunVersion-0">
                <chkrun:supportedType>CLAS*</chkrun:supportedType>
            </chkrun:reporter>
        </chkrun:checkReporters>
    "#;

    struct UnusedTransport;

    #[test]
    fn rejects_unknown_check_report_fields_including_message_wrappers() {
        let xml = std::str::from_utf8(CHECK_REPORTS_XML).unwrap();
        for tag in [
            "chkrun:checkRunReports",
            "chkrun:checkReport",
            "chkrun:checkMessageList",
        ] {
            for (from, to) in [
                (format!("<{tag}"), format!("<{tag} unexpected=\"true\"")),
                (format!("</{tag}>"), format!("<unexpected/></{tag}>")),
            ] {
                let body = xml.replacen(&from, &to, 1);
                let error = serde_xml_rs::from_str::<CheckRunReports>(&body)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("unknown field"), "{tag}: {error}");
                assert!(error.contains("unexpected"), "{tag}: {error}");
            }
        }
    }

    #[async_trait]
    impl crate::Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn discovered_client() -> Client<Discovery> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
        )
    }

    fn serialize(objects: &CheckObjectList, resolver: &Discovery) -> String {
        objects.serialize(resolver).unwrap()
    }

    #[test]
    fn serializes_persisted_and_dirty_check_objects() {
        let client = discovered_client();
        let object = ObjectKey::<Program>::new("Z_TEST");
        let source_uri = AdtUri::parse("/sap/bc/adt/programs/programs/z_test/source/main").unwrap();
        let mut run = ObjectCheckRun::new();
        run.push_object(CheckRunObject::new(&object, WorkbenchVersion::Active))
            .push_object(
                CheckRunObject::new(&object, WorkbenchVersion::WorkingArea).artifact(
                    CheckRunArtifact::new(source_uri, "text/plain; charset=utf-8", b"hello"),
                ),
            )
            .push_reporter(CheckRunReporter::SYNTAX_CHECK_RUNNER);

        let xml = serialize(&run.objects, client.discovery());

        assert_eq!(xml.matches("<chkrun:checkObject ").count(), 2);
        assert_eq!(xml.matches("<chkrun:artifacts>").count(), 1);
        assert!(xml.contains("chkrun:version=\"active\""));
        assert!(xml.contains("chkrun:version=\"workingArea\""));
        assert!(xml.contains("chkrun:contentType=\"text/plain; charset=utf-8\""));
        assert!(xml.contains("<chkrun:content>aGVsbG8=</chkrun:content>"));
    }

    #[test]
    fn check_run_preserves_a_parentless_childs_advertised_uri() {
        let client = discovered_client();
        let object = ObjectRef::new(
            ObjectKey::<crate::FunctionModule>::from_parts(
                "Z_MODULE".to_owned(),
                "FUGR/FF".parse().unwrap(),
                None,
            ),
            AdtUri::parse("/sap/bc/adt/custom/checkable/42").unwrap(),
        );
        let mut run = ObjectCheckRun::new();
        run.push_object(CheckRunObject::from_ref(&object, WorkbenchVersion::Active));
        run.push_object(CheckRunObject::from_ref(
            &object.erase(),
            WorkbenchVersion::Active,
        ));

        let request = run.encode(client.discovery()).unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();
        assert_eq!(
            body.matches("adtcore:uri=\"/sap/bc/adt/custom/checkable/42\"")
                .count(),
            2
        );
    }

    #[test]
    fn check_run_uses_the_discovered_contract() {
        let client = discovered_client();
        let object = ObjectKey::<Program>::new("Z_TEST");
        let mut run = ObjectCheckRun::new();
        run.push_object(CheckRunObject::new(&object, WorkbenchVersion::Active))
            .push_reporter(CheckRunReporter::SYNTAX_CHECK_RUNNER);

        let request = run.encode(client.discovery()).unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.target().as_str(), "/sap/bc/adt/checkruns");
        assert_eq!(
            request.query(),
            [("reporters".to_owned(), "abapCheckRun".to_owned())]
        );
        assert_eq!(
            request.headers()[header::ACCEPT],
            CheckRunReports::MEDIA_TYPE
        );
        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            CheckObjectList::MEDIA_TYPE
        );
        assert!(
            std::str::from_utf8(request.body())
                .unwrap()
                .contains("adtcore:uri=\"/sap/bc/adt/programs/programs/z_test\"")
        );
    }

    #[test]
    fn reporters_query_uses_the_discovered_contract_and_decodes_supported_types() {
        let client = discovered_client();
        let query = CheckRunReportersQuery::new();
        let request = query.encode(client.discovery()).unwrap();

        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.target().as_str(), "/sap/bc/adt/checkruns/reporters");
        assert_eq!(
            request.headers()[header::ACCEPT],
            SupportedCheckReporterList::MEDIA_TYPE
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            SupportedCheckReporterList::MEDIA_TYPE.parse().unwrap(),
        );
        let response = AdtResponse::new(StatusCode::OK, headers, CHECK_REPORTERS_XML.to_vec());
        let reporters = query
            .decode(OperationResponse::new(
                response,
                AdtUri::parse("/sap/bc/adt/checkruns/reporters").unwrap(),
            ))
            .unwrap();

        assert_eq!(reporters.len(), 2);
        assert_eq!(reporters[0].name.as_str(), "abapCheckRun");
        assert_eq!(reporters[0].supported_types, ["CLAS*", "PROG*"]);
        assert_eq!(reporters[1].name.as_str(), "abapCheckRunVersion-0");

        let mut run = ObjectCheckRun::new();
        run.extend_reporters(&reporters);
        let request = run.encode(client.discovery()).unwrap();
        assert_eq!(
            request.query(),
            [
                ("reporters".to_owned(), "abapCheckRun".to_owned()),
                ("reporters".to_owned(), "abapCheckRunVersion-0".to_owned()),
            ]
        );
    }

    #[test]
    fn decodes_check_run_reports_and_source_fragments() {
        let run = ObjectCheckRun::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            CheckRunReports::MEDIA_TYPE.parse().unwrap(),
        );
        let response = AdtResponse::new(StatusCode::OK, headers, CHECK_REPORTS_XML.to_vec());
        let reports = run
            .decode(OperationResponse::new(
                response,
                AdtUri::parse("/sap/bc/adt/checkruns").unwrap(),
            ))
            .unwrap();

        assert_eq!(reports.reports.len(), 1);
        assert_eq!(reports.reports[0].reporter.as_str(), "abapCheckRun");
        assert_eq!(reports.reports[0].messages.messages.len(), 1);
        assert!(
            reports.reports[0].messages.messages[0]
                .uri
                .contains("#start=2,1")
        );
        assert_eq!(
            reports.reports[0].messages.messages[0].code.as_deref(),
            Some("SYNTAX(001)")
        );
    }
}
