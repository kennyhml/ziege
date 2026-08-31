use super::run::ObjectRun;
use crate::{
    Advertised, CategoryId, EncodeError, EncodedOperation,
    error::ResponseError,
    objects::{ImmediateRun, Object, ObjectRef, Program, RunCapability},
    operation::{Operation, OperationResponse, Stateless},
};

const PROGRAM_NAME_VARIABLE: &str = "programname";
const PROGRAM_RUN_CATEGORY: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/programs",
    term: "programrun",
};
const PROGRAM_RUN_RELATION: &str = "http://www.sap.com/adt/relations/programs/programrun";

/// The plain-text console output produced by running an ABAP program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRunResult {
    /// The program that was executed.
    pub reference: ObjectRef<Program>,

    /// The rendered program output returned by SAP.
    pub content: String,
}

impl ProgramRunResult {
    pub(crate) fn new(reference: ObjectRef<Program>, content: String) -> Self {
        Self { reference, content }
    }
}

/// Runs an executable ABAP program and returns its rendered console output.
///
/// This does not currently support IF_OO_ADT_CLASSRUN inside programs. The
/// only way output is returned is when executing a list report. The backend
/// resource then exports that list into the plain text of the body.
///
/// Even if the user does not have sufficent permissions to execute the
/// program or the program could not be found, 200 OK is returned.
///
/// ADT can not handle program dumps, it simply returns a status code 500.
///
/// The profiler id usually seems to be a URL pointing to a freshly created
/// configuration posted to `runtime/traces/abaptraces/parameters`, there
/// seems to be a way to have them predefined too. Must be clarified
///
/// - Backend handler: `CL_SEDI_ADT_PROGRAMRUN`
#[derive(Debug)]
pub struct ProgramRun {
    /// The executable program to run.
    program: ObjectRef<Program>,
    run: ObjectRun,
}

impl ProgramRun {
    fn new(program: ObjectRef<Program>) -> Self {
        let run = ObjectRun::typed(&program);
        Self { program, run }
    }

    /// Runs the program with the supplied ABAP profiler trace identifier.
    #[must_use]
    pub fn profiler_id(mut self, profiler_id: impl Into<String>) -> Self {
        self.run = self.run.profiler_id(profiler_id);
        self
    }
}

impl ImmediateRun for Program {
    const RUN: RunCapability = RunCapability::new(
        PROGRAM_RUN_CATEGORY,
        PROGRAM_RUN_RELATION,
        PROGRAM_NAME_VARIABLE,
    );
}

impl Operation for ProgramRun {
    type Response = ProgramRunResult;
    type Kind = Stateless;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        self.run.encode()
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        let output = self.run.decode(response)?;
        Ok(ProgramRunResult::new(self.program.clone(), output.content))
    }
}

impl ObjectRef<Program> {
    /// Creates an operation that runs this program.
    pub fn run(&self) -> ProgramRun {
        ProgramRun::new(self.clone())
    }
}

impl Object<Program> {
    /// Creates an operation that runs this loaded program.
    pub fn run(&self) -> ProgramRun {
        self.reference().run()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use super::*;
    use crate::api::run::PROFILER_ID_QUERY;
    use crate::{
        AdtRequest, AdtResponse, AdtUri, Client, CompatibilityError, EntityTag, Include,
        IncludeProperties, MediaTyped, ObjectError, ObjectPropertiesQuery, OperationError,
        ProgramProperties, Ready, ResolveError, Revalidation, Transport,
    };

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");
    const PROGRAM_XML: &str = include_str!("../../tests/fixtures/program-z-test.xml");
    const INCLUDE_XML: &str = include_str!("../../tests/fixtures/include-ztest.xml");
    struct UnusedTransport;

    #[async_trait]
    impl crate::Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    struct RecordingTransport {
        requests: Arc<Mutex<Vec<AdtRequest>>>,
    }

    #[async_trait]
    impl Transport for RecordingTransport {
        async fn send(&self, request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            self.requests.lock().unwrap().push(request);
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(crate::protocol::TEXT_PLAIN_MEDIA_TYPE),
            );
            Ok(AdtResponse::new(StatusCode::OK, headers, Vec::new()))
        }
    }

    fn ready_client(xml: &[u8]) -> Client<Ready> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(xml).unwrap(),
            crate::api::discovery::parse_capabilities(xml).unwrap(),
        )
    }

    fn recording_client(xml: &[u8]) -> (Client<Ready>, Arc<Mutex<Vec<AdtRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = Client::new(RecordingTransport {
            requests: Arc::clone(&requests),
        })
        .with_capabilities(
            crate::api::discovery::parse_capabilities(xml).unwrap(),
            crate::api::discovery::parse_capabilities(xml).unwrap(),
        );
        (client, requests)
    }

    fn program_properties_query() -> ObjectPropertiesQuery<Program> {
        ObjectRef::<Program>::for_test(
            "Z_TEST",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/Z_TEST").unwrap(),
        )
        .query()
    }

    fn program_properties_response(media_type: &'static str) -> OperationResponse {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&format!("{media_type}; charset=utf-8")).unwrap(),
        );
        headers.insert(header::ETAG, HeaderValue::from_static("program-etag"));
        operation_response(AdtResponse::new(
            StatusCode::OK,
            headers,
            PROGRAM_XML.as_bytes().to_vec(),
        ))
    }

    fn include_properties_query() -> ObjectPropertiesQuery<Include> {
        ObjectRef::<Include>::for_test(
            "ZTEST",
            crate::AdtUri::parse("/sap/bc/adt/programs/includes/ZTEST").unwrap(),
        )
        .query()
    }

    fn program_run() -> ProgramRun {
        ObjectRef::<Program>::for_test(
            "Z_TEST",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/Z_TEST").unwrap(),
        )
        .run()
    }

    fn request_target() -> AdtUri {
        AdtUri::parse("/sap/bc/adt/test").unwrap()
    }

    fn operation_response(response: AdtResponse) -> OperationResponse {
        OperationResponse::new(response, request_target())
    }

    #[test]
    fn include_properties_query_defaults_to_v2() {
        let request = include_properties_query().encode().unwrap();

        assert_eq!(
            request.headers()[header::ACCEPT],
            IncludeProperties::MEDIA_TYPES[0]
        );
    }

    #[test]
    fn program_properties_query_accepts_every_supported_version() {
        let request = program_properties_query().encode().unwrap();

        assert_eq!(
            request.headers()[header::ACCEPT],
            "application/vnd.sap.adt.programs.programs.v3+xml, application/vnd.sap.adt.programs.programs.v2+xml"
        );
    }

    #[tokio::test]
    async fn expands_namespaced_program_run_variables() {
        let (client, requests) = recording_client(DISCOVERY_XML);
        ObjectRef::<Program>::for_test(
            "/DMO/PROGRAM",
            AdtUri::parse("/sap/bc/adt/programs/programs/%2Fdmo%2Fprogram").unwrap(),
        )
        .run()
        .profiler_id("TRACE ID")
        .execute(&client)
        .await
        .unwrap();
        let requests = requests.lock().unwrap();
        let request = &requests[0];

        assert_eq!(
            request.target().as_str(),
            "/sap/bc/adt/programs/programrun/%2Fdmo%2Fprogram"
        );
        assert_eq!(
            request.query(),
            [("profilerId".to_owned(), "TRACE ID".to_owned())]
        );
    }

    #[tokio::test]
    async fn omits_an_unset_program_run_profiler() {
        let (client, requests) = recording_client(DISCOVERY_XML);
        program_run().execute(&client).await.unwrap();
        let requests = requests.lock().unwrap();
        let request = &requests[0];

        assert!(request.query().is_empty());
    }

    #[tokio::test]
    async fn rejects_profiling_when_the_template_does_not_advertise_it() {
        let client = ready_client(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom"
                    xmlns:adtcomp="http://www.sap.com/adt/compatibility">
                    <app:workspace>
                        <atom:title>Programs</atom:title>
                        <app:collection href="/sap/bc/adt/programs/programrun">
                            <atom:category term="programrun"
                                scheme="http://www.sap.com/adt/categories/programs" />
                            <adtcomp:templateLinks>
                                <adtcomp:templateLink
                                    rel="http://www.sap.com/adt/relations/programs/programrun"
                                    template="/sap/bc/adt/programs/programrun/{programname}" />
                            </adtcomp:templateLinks>
                        </app:collection>
                    </app:workspace>
                </app:service>"#,
        );
        let error = program_run()
            .profiler_id("TRACE-ID")
            .execute(&client)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OperationError::Resolve(ResolveError::Object(
                ObjectError::UnsupportedTemplateParameter { parameter }
            )) if parameter == PROFILER_ID_QUERY
        ));
    }

    #[tokio::test]
    async fn selects_the_relation_template_with_the_required_variables() {
        let (client, requests) = recording_client(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom"
                    xmlns:adtcomp="http://www.sap.com/adt/compatibility">
                    <app:workspace>
                        <atom:title>Programs</atom:title>
                        <app:collection href="/sap/bc/adt/programs/programrun">
                            <atom:category term="programrun"
                                scheme="http://www.sap.com/adt/categories/programs" />
                            <adtcomp:templateLinks>
                                <adtcomp:templateLink
                                    rel="http://www.sap.com/adt/relations/programs/programrun"
                                    template="/sap/bc/adt/programs/programrun/wrong/{other}" />
                                <adtcomp:templateLink
                                    rel="http://www.sap.com/adt/relations/programs/programrun"
                                    template="/sap/bc/adt/programs/programrun/{programname}" />
                            </adtcomp:templateLinks>
                        </app:collection>
                    </app:workspace>
                </app:service>"#,
        );

        program_run().execute(&client).await.unwrap();

        assert_eq!(
            requests.lock().unwrap()[0].target().as_str(),
            "/sap/bc/adt/programs/programrun/z_test"
        );
    }

    #[test]
    fn rejects_non_utf8_program_run_output() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(crate::protocol::TEXT_PLAIN_MEDIA_TYPE),
        );
        let response = AdtResponse::new(StatusCode::OK, headers, vec![0xff]);
        let error = program_run()
            .decode(operation_response(response))
            .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::InvalidResponseEncoding(_))
        ));
    }

    #[tokio::test]
    async fn program_run_request_requires_the_discovery_collection() {
        let client = ready_client(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom">
                    <app:workspace><atom:title>Programs</atom:title></app:workspace>
                </app:service>"#,
        );
        let error = program_run().execute(&client).await.unwrap_err();

        assert!(matches!(
            error,
            OperationError::Resolve(ResolveError::Compatibility(
                CompatibilityError::MissingCollection(category)
            )) if category == PROGRAM_RUN_CATEGORY
        ));
    }

    #[tokio::test]
    async fn program_run_request_requires_the_relation_template() {
        let client = ready_client(
            br#"<app:service xmlns:app="http://www.w3.org/2007/app"
                    xmlns:atom="http://www.w3.org/2005/Atom">
                    <app:workspace>
                        <atom:title>Programs</atom:title>
                        <app:collection href="/sap/bc/adt/programs/programrun">
                            <atom:category term="programrun"
                                scheme="http://www.sap.com/adt/categories/programs" />
                        </app:collection>
                    </app:workspace>
                </app:service>"#,
        );
        let error = program_run().execute(&client).await.unwrap_err();

        assert!(matches!(
            error,
            OperationError::Resolve(ResolveError::Object(ObjectError::MissingTemplate {
                relation: PROGRAM_RUN_RELATION,
            }))
        ));
    }

    #[test]
    fn tags_a_v2_program_properties_representation() {
        let representation = program_properties_query()
            .decode(program_properties_response(
                ProgramProperties::MEDIA_TYPES[1],
            ))
            .unwrap();
        assert_eq!(
            representation.media_type(),
            ProgramProperties::MEDIA_TYPES[1]
        );
        assert_eq!(representation.properties().name, "Z_TEST");
    }

    #[test]
    fn tags_a_v3_program_properties_representation() {
        let representation = program_properties_query()
            .decode(program_properties_response(
                ProgramProperties::MEDIA_TYPES[0],
            ))
            .unwrap();
        assert_eq!(
            representation.media_type(),
            ProgramProperties::MEDIA_TYPES[0]
        );
        assert_eq!(representation.properties().name, "Z_TEST");
    }

    #[test]
    fn properties_query_requires_a_response_content_type() {
        let response = AdtResponse::new(
            StatusCode::OK,
            HeaderMap::new(),
            PROGRAM_XML.as_bytes().to_vec(),
        );

        let error = program_properties_query()
            .decode(operation_response(response))
            .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::MissingContentType { target } if target == request_target()
        ));
    }

    #[test]
    fn properties_query_reports_an_unsupported_response_content_type() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let response = AdtResponse::new(StatusCode::OK, headers, PROGRAM_XML.as_bytes().to_vec());

        let error = program_properties_query()
            .decode(operation_response(response))
            .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::UnsupportedContentType {
                target,
                content_type,
                supported,
            } if target == request_target()
                && content_type == "application/json"
                && supported == [
                    ProgramProperties::MEDIA_TYPES[0],
                    ProgramProperties::MEDIA_TYPES[1],
                ]
        ));
    }

    #[test]
    fn wraps_a_modified_conditional_program_properties_query() {
        let response = program_properties_query()
            .if_none_match(EntityTag::from_static("old-etag"))
            .decode(program_properties_response(
                ProgramProperties::MEDIA_TYPES[0],
            ))
            .unwrap();

        assert!(matches!(response, Revalidation::Modified(_)));
    }

    #[test]
    fn loaded_program_revalidates_with_its_entity_tag() {
        let program = program_properties_query()
            .decode(program_properties_response(
                ProgramProperties::MEDIA_TYPES[0],
            ))
            .unwrap();
        let request = program.revalidate().unwrap().encode().unwrap();

        assert_eq!(request.headers()[header::IF_NONE_MATCH], "program-etag");
    }

    #[test]
    fn rejects_not_modified_for_an_unconditional_program_properties_query() {
        let response = AdtResponse::new(StatusCode::NOT_MODIFIED, HeaderMap::new(), Vec::new());
        let error = program_properties_query()
            .decode(operation_response(response))
            .unwrap_err();

        assert!(matches!(error, ResponseError::UnexpectedNotModified));
    }

    #[test]
    fn decodes_a_v2_include_properties_representation() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.sap.adt.programs.includes.v2+xml"),
        );
        headers.insert(header::ETAG, HeaderValue::from_static("include-etag"));
        let response = AdtResponse::new(StatusCode::OK, headers, INCLUDE_XML.as_bytes().to_vec());

        let representation = include_properties_query()
            .decode(operation_response(response))
            .unwrap();
        assert_eq!(representation.properties().name, "ZTEST");
        assert_eq!(
            representation.etag().map(EntityTag::as_str),
            Some("include-etag")
        );
    }

    #[test]
    fn returns_not_modified_for_a_current_include_etag() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ETAG, HeaderValue::from_static("include-etag"));
        let response = AdtResponse::new(StatusCode::NOT_MODIFIED, headers, Vec::new());

        let response = include_properties_query()
            .if_none_match(EntityTag::from_static("include-etag"))
            .decode(operation_response(response))
            .unwrap();
        assert!(matches!(&response, Revalidation::NotModified { .. }));
        assert_eq!(response.not_modified_etag(), Some("include-etag"));
        assert!(response.as_modified().is_none());
    }

    #[test]
    fn rejects_not_modified_for_an_unconditional_include_properties_query() {
        let response = AdtResponse::new(StatusCode::NOT_MODIFIED, HeaderMap::new(), Vec::new());
        let error = include_properties_query()
            .decode(operation_response(response))
            .unwrap_err();

        assert!(matches!(error, ResponseError::UnexpectedNotModified));
    }
}
