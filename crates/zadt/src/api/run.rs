use std::collections::HashMap;

use http::Method;
use stduritemplate::Value;

use crate::{
    client::{Client, Ready},
    error::{ObjectError, OperationError, ResponseError},
    objects::{AdtObject, GlobalWorkbenchType, ImmediateRun, ObjectRef, RunCapability},
    operation::{Operation, OperationResponse, Stateless},
    protocol::AdtRequest,
    vocabulary::{media_type, query_parameter},
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

impl Operation<Ready> for ObjectRun {
    type Response = ObjectRunResult;
    type Kind = Stateless;

    fn request(&self, client: &Client<Ready>) -> Result<AdtRequest, OperationError> {
        let template = self.run.target.template(client)?;
        if !template.has_variable(self.run.name_variable) {
            return Err(ObjectError::InvalidTemplate {
                template: template.as_str().to_owned(),
                reason: format!("missing `{}` variable", self.run.name_variable),
            }
            .into());
        }
        if self.profiler_id.is_some() && !template.has_variable(query_parameter::PROFILER_ID) {
            return Err(ObjectError::UnsupportedTemplateParameter {
                parameter: query_parameter::PROFILER_ID,
            }
            .into());
        }

        let mut variables = HashMap::from([(
            self.run.name_variable.to_owned(),
            Value::String(self.reference.name().to_ascii_lowercase()),
        )]);
        if let Some(profiler_id) = &self.profiler_id {
            variables.insert(
                query_parameter::PROFILER_ID.to_owned(),
                Value::String(profiler_id.clone()),
            );
        }
        let (target, query) = template.expand(&variables)?;
        let mut request = AdtRequest::new(Method::POST, target);
        for (name, value) in query {
            request.push_query(name, value);
        }
        request.set_accept(media_type::SOURCE);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        if !response.status().is_success() {
            return Err(ResponseError::unexpected_status(response.response()));
        }
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

impl AdtObject {
    /// Creates an immediate run operation when this object family supports it.
    pub fn run(&self) -> Result<ObjectRun, ObjectError> {
        self.reference().erase().run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use http::{HeaderMap, StatusCode};

    use crate::{AdtResponse, Class, ObjectRef, Program, Transport};

    const DISCOVERY_XML: &[u8] = include_bytes!("../../tests/fixtures/discovery.xml");

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, crate::TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn program() -> ObjectRef<Program> {
        ObjectRef::<Program>::for_test(
            "ZPROGRAM",
            crate::AdtUri::parse("/sap/bc/adt/programs/programs/zprogram").unwrap(),
        )
    }

    fn ready_client() -> Client<Ready> {
        Client::new(UnusedTransport).with_capabilities(
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
            crate::api::discovery::parse_capabilities(DISCOVERY_XML).unwrap(),
        )
    }

    #[test]
    fn type_erased_runs_dispatch_through_object_descriptors() {
        fn accepts_ready<O: Operation<Ready>>() {}
        accepts_ready::<ObjectRun>();

        let client = ready_client();
        let program = program();
        let program_run = program.erase().run().unwrap();
        let program_request = program_run.request(&client).unwrap();
        assert_eq!(
            program_request.target().as_str(),
            "/sap/bc/adt/programs/programrun/zprogram"
        );

        let program_output = program_run
            .decode(OperationResponse::new(
                AdtResponse::new(StatusCode::OK, HeaderMap::new(), b"program output".to_vec()),
                program_request.target().clone(),
            ))
            .unwrap();
        assert_eq!(program_output.reference, program.erase());
        assert_eq!(program_output.object_type.as_str(), "PROG/P");
        assert_eq!(program_output.content, "program output");

        let class = ObjectRef::<Class>::for_test(
            "ZCL_EXAMPLE",
            crate::AdtUri::parse("/sap/bc/adt/oo/classes/zcl_example").unwrap(),
        );
        let class_request = class
            .erase()
            .run()
            .unwrap()
            .profiler_id("TRACE ID")
            .request(&client)
            .unwrap();
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
