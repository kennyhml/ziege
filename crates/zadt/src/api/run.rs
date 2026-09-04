use std::collections::HashMap;

use http::{Method, StatusCode};
use stduritemplate::Value;

use crate::{
    Discovery, EncodeError, EncodedOperation, RequiresDiscovery, ResolveError,
    error::{ObjectError, ResponseError},
    objects::{GlobalWorkbenchType, ImmediateRun, ObjectRef, ObjectSnapshot, RunCapability},
    operation::{Operation, OperationResponse, Stateless},
    protocol::TEXT_PLAIN_MEDIA_TYPE,
    resource::AdtUriTemplate,
};

/// Plain-text output produced by running a type-erased repository object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectRunResult {
    /// The type-erased object that was executed.
    pub reference: ObjectRef,

    /// The exact Workbench type of the executed object.
    pub object_type: GlobalWorkbenchType,

    /// The rendered output returned by SAP.
    pub content: String,
}

impl ObjectRunResult {
    pub(crate) fn new(
        reference: ObjectRef,
        object_type: GlobalWorkbenchType,
        content: String,
    ) -> Self {
        Self {
            reference,
            object_type,
            content,
        }
    }
}

/// Runs a type-erased repository object through its descriptor capability.
#[derive(Debug)]
pub struct ObjectRun {
    reference: ObjectRef,
    run: RunCapability,
    profiler_id: Option<String>,
}

impl ObjectRun {
    pub(super) const PROFILER_ID_QUERY: &str = "profilerId";

    pub(crate) fn new(reference: ObjectRef, run: RunCapability) -> Self {
        Self {
            reference,
            run,
            profiler_id: None,
        }
    }

    pub(crate) fn typed<T: ImmediateRun>(reference: &ObjectRef<T>) -> Self {
        Self::new(reference.erase(), T::RUN)
    }

    /// Runs the object with the supplied ABAP profiler trace identifier.
    #[must_use]
    pub fn profiler_id(mut self, profiler_id: impl Into<String>) -> Self {
        self.profiler_id = Some(profiler_id.into());
        self
    }
}

impl Operation for ObjectRun {
    type Response = ObjectRunResult;
    type Kind = Stateless;
    type ResolutionRequirement = RequiresDiscovery;

    fn encode(&self, resolver: &Discovery) -> Result<EncodedOperation, EncodeError> {
        let link = resolver.require_template(self.run.category, self.run.relation)?;
        let template = AdtUriTemplate::new(link.template());
        if !template.has_variable(self.run.name_variable) {
            return Err(
                ResolveError::from(ObjectError::UnsupportedTemplateParameter {
                    parameter: self.run.name_variable,
                })
                .into(),
            );
        }
        let mut variables = HashMap::from([(
            self.run.name_variable.to_owned(),
            Value::String(self.reference.name().to_ascii_lowercase()),
        )]);
        if let Some(profiler_id) = &self.profiler_id {
            if !template.has_variable(Self::PROFILER_ID_QUERY) {
                return Err(
                    ResolveError::from(ObjectError::UnsupportedTemplateParameter {
                        parameter: Self::PROFILER_ID_QUERY,
                    })
                    .into(),
                );
            }
            variables.insert(
                Self::PROFILER_ID_QUERY.to_owned(),
                Value::String(profiler_id.clone()),
            );
        }
        let (target, query) = template.expand(&variables).map_err(ResolveError::from)?;
        let mut request = EncodedOperation::new(Method::POST, target);
        for (name, value) in query {
            request.push_query(name, value);
        }
        request.set_accept(TEXT_PLAIN_MEDIA_TYPE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[TEXT_PLAIN_MEDIA_TYPE])?;
        let content = String::from_utf8(response.into_body())
            .map_err(ObjectError::InvalidResponseEncoding)?;
        Ok(ObjectRunResult::new(
            self.reference.clone(),
            self.reference.object_type().clone(),
            content,
        ))
    }
}

impl ObjectRef<()> {
    /// Creates an immediate run operation when this object family supports it.
    pub fn run(&self) -> Result<ObjectRun, ObjectError> {
        let run = self
            .descriptor()
            .and_then(|descriptor| descriptor.run())
            .ok_or_else(|| ObjectError::UnsupportedCapability {
                object_type: self.object_type().clone(),
                capability: "immediate run",
            })?;
        Ok(ObjectRun::new(self.clone(), run))
    }
}

impl ObjectSnapshot<()> {
    /// Creates an immediate run operation when this object family supports it.
    pub fn run(&self) -> Result<ObjectRun, ObjectError> {
        self.reference().run()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use crate::{AdtRequest, AdtResponse, Class, Client, Discovery, ObjectRef, Program, Transport};

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");

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
                HeaderValue::from_static(TEXT_PLAIN_MEDIA_TYPE),
            );
            Ok(AdtResponse::new(
                StatusCode::OK,
                headers,
                b"program output".to_vec(),
            ))
        }
    }

    fn program() -> ObjectRef<Program> {
        ObjectRef::<Program>::new("ZPROGRAM")
    }

    fn discovered_client() -> (Client<Discovery>, Arc<Mutex<Vec<AdtRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = Client::new(RecordingTransport {
            requests: Arc::clone(&requests),
        })
        .with_capabilities(
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
        );
        (client, requests)
    }

    #[tokio::test]
    async fn type_erased_runs_dispatch_through_object_descriptors() {
        fn requires_discovery<O: Operation<ResolutionRequirement = RequiresDiscovery>>() {}
        requires_discovery::<ObjectRun>();

        let (client, requests) = discovered_client();
        let program = program();
        let program_run = program.erase().run().unwrap();
        let program_output = program_run.execute(&client).await.unwrap();
        {
            let requests_guard = requests.lock().unwrap();
            let program_request = &requests_guard[0];
            assert_eq!(
                program_request.target().as_str(),
                "/sap/bc/adt/programs/programrun/zprogram"
            );
        }

        assert_eq!(program_output.reference, program.erase());
        assert_eq!(program_output.object_type.as_str(), "PROG/P");
        assert_eq!(program_output.content, "program output");

        let class = ObjectRef::<Class>::new("ZCL_EXAMPLE");
        class
            .erase()
            .run()
            .unwrap()
            .profiler_id("TRACE ID")
            .execute(&client)
            .await
            .unwrap();
        let requests_guard = requests.lock().unwrap();
        let class_request = &requests_guard[1];
        assert_eq!(
            class_request.target().as_str(),
            "/sap/bc/adt/oo/classrun/zcl_example"
        );
        assert_eq!(
            class_request.query(),
            [("profilerId".to_owned(), "TRACE ID".to_owned())]
        );
    }
}
