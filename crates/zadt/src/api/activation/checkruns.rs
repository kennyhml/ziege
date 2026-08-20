use std::borrow::Cow;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use http::{Method, StatusCode};
use serde::{Deserialize, Serialize, Serializer};

use crate::{
    AdtRequest, AdtUri, AdvertisedLink, CategoryId, Client, ObjectError, ObjectRef, ObjectVersion,
    Operation, OperationError, OperationResponse, Ready, ResponseError, Stateless,
    target::CollectionTarget,
};

const CHECK_RUN_CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/check",
    term: "checkruns",
};
const CHECK_OBJECTS_MEDIA_TYPE: &str = "application/vnd.sap.adt.checkobjects+xml";
const CHECK_MESSAGES_MEDIA_TYPE: &str = "application/vnd.sap.adt.checkmessages+xml";
const CHECK_RUN_NAMESPACE: &str = "http://www.sap.com/adt/checkrun";
const ADT_CORE_NAMESPACE: &str = "http://www.sap.com/adt/core";

/// Executes checks on a set of objects with the given reporters.
///
/// Multiple reporters can be specified. The avaiable reporters can be
/// queried using [`CheckRunReportersQuery`]. Common reporters are
/// specified as constants in [`CheckRunReporter`] - it can, however not
/// account for kernel release based check runners or custom enhancements.
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
    const TARGET: CollectionTarget = CollectionTarget::new(CHECK_RUN_CATEGORY);

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
}

impl Operation<Ready> for ObjectCheckRun {
    type Kind = Stateless;
    type Response = CheckRunReports;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let body = self.objects.serialize()?;
        let mut request = Self::TARGET.request(client, Method::POST)?;
        for reporter in &self.reporters {
            request.push_query("reporters", reporter.as_str());
        }
        request.set_accept(CHECK_MESSAGES_MEDIA_TYPE);
        request.set_content_type(CHECK_OBJECTS_MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[CHECK_MESSAGES_MEDIA_TYPE])?;
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

#[derive(Debug, Default, Serialize)]
#[serde(rename = "chkrun:checkObjectList")]
struct CheckObjectList {
    #[serde(rename = "chkrun:checkObject")]
    objects: Vec<CheckRunObject>,
}

impl CheckObjectList {
    fn serialize(&self) -> Result<String, ObjectError> {
        serde_xml_rs::SerdeXml::new()
            .namespace("chkrun", CHECK_RUN_NAMESPACE)
            .namespace("adtcore", ADT_CORE_NAMESPACE)
            .to_string(self)
            .map_err(ObjectError::InvalidRequest)
    }
}

/// One repository object included in a check run.
#[derive(Debug, Serialize)]
pub struct CheckRunObject {
    #[serde(rename = "@adtcore:uri")]
    uri: AdtUri,

    #[serde(rename = "@chkrun:version")]
    version: ObjectVersion,

    #[serde(
        rename = "chkrun:artifacts",
        skip_serializing_if = "CheckRunArtifacts::is_empty"
    )]
    artifacts: CheckRunArtifacts,
}

impl CheckRunObject {
    pub fn new<T>(object: &ObjectRef<T>, version: ObjectVersion) -> Self {
        Self {
            uri: object.uri().clone(),
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
#[serde(rename = "chkrun:checkRunReports")]
pub struct CheckRunReports {
    #[serde(rename = "chkrun:checkReport", default)]
    pub reports: Vec<CheckRunReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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
pub struct CheckRunMessageList {
    #[serde(rename = "chkrun:checkMessage", default)]
    pub messages: Vec<CheckRunMessage>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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
    use crate::{AdtResponse, Program, Transport, TransportError};
    use http::{HeaderMap, header};

    const CHECK_DISCOVERY_XML: &[u8] = br#"
        <app:service xmlns:app="http://www.w3.org/2007/app"
                xmlns:atom="http://www.w3.org/2005/Atom">
            <app:workspace>
                <atom:title>Activation</atom:title>
                <app:collection href="/sap/bc/adt/checkruns">
                    <atom:category term="checkruns"
                        scheme="http://www.sap.com/adt/categories/check" />
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

    struct UnusedTransport;

    #[async_trait::async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn ready_client() -> Client<Ready> {
        Client::new(UnusedTransport).with_capabilities(
            super::super::super::discovery::parse_capabilities(CHECK_DISCOVERY_XML).unwrap(),
            super::super::super::discovery::parse_capabilities(CHECK_DISCOVERY_XML).unwrap(),
        )
    }

    fn serialize(objects: &CheckObjectList) -> String {
        objects.serialize().unwrap()
    }

    #[test]
    fn serializes_persisted_and_dirty_check_objects() {
        let object = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
        );
        let source_uri = AdtUri::parse("/sap/bc/adt/programs/programs/z_test/source/main").unwrap();
        let mut run = ObjectCheckRun::new();
        run.push_object(CheckRunObject::new(&object, ObjectVersion::Active))
            .push_object(
                CheckRunObject::new(&object, ObjectVersion::WorkingArea).artifact(
                    CheckRunArtifact::new(source_uri, "text/plain; charset=utf-8", b"hello"),
                ),
            )
            .push_reporter(CheckRunReporter::SYNTAX_CHECK_RUNNER);

        let xml = serialize(&run.objects);

        assert_eq!(xml.matches("<chkrun:checkObject ").count(), 2);
        assert_eq!(xml.matches("<chkrun:artifacts>").count(), 1);
        assert!(xml.contains("chkrun:version=\"active\""));
        assert!(xml.contains("chkrun:version=\"workingArea\""));
        assert!(xml.contains("chkrun:contentType=\"text/plain; charset=utf-8\""));
        assert!(xml.contains("<chkrun:content>aGVsbG8=</chkrun:content>"));
    }

    #[test]
    fn check_run_uses_the_discovered_contract() {
        let object = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
        );
        let mut run = ObjectCheckRun::new();
        run.push_object(CheckRunObject::new(&object, ObjectVersion::Active))
            .push_reporter(CheckRunReporter::SYNTAX_CHECK_RUNNER);

        let request = run.request(&ready_client()).unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.target().as_str(), "/sap/bc/adt/checkruns");
        assert_eq!(
            request.query(),
            [("reporters".to_owned(), "abapCheckRun".to_owned())]
        );
        assert_eq!(request.headers()[header::ACCEPT], CHECK_MESSAGES_MEDIA_TYPE);
        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            CHECK_OBJECTS_MEDIA_TYPE
        );
    }

    #[test]
    fn decodes_check_run_reports_and_source_fragments() {
        let run = ObjectCheckRun::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            CHECK_MESSAGES_MEDIA_TYPE.parse().unwrap(),
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
