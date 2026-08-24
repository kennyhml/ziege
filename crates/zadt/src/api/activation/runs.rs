use http::{Method, StatusCode};
use serde::Deserialize;

use crate::{
    Advertised, AdvertisedLink, AdvertisedObjectReference, AnyObject, CategoryId, EncodeError,
    EncodedOperation, Object, ObjectError, ObjectRef, ObjectType, Operation, OperationResponse,
    ResponseError, Stateless, objects::ObjectReferences, operation::CollectionTarget,
};

const ACTIVATE_OBJECTS: CategoryId = CategoryId {
    scheme: "http://www.sap.com/adt/categories/activation",
    term: "activationruns",
};

#[derive(Debug, Eq, PartialEq)]
pub enum ActivationRunMode {
    /// Checks the objects without activating them.
    Check,
    /// Activates the objects.
    Activate,
}

impl ActivationRunMode {
    fn as_str(&self) -> &str {
        match self {
            Self::Activate => "activate",
            Self::Check => "check",
        }
    }
}

/// Activates or activation-checks the list of provided objects.
///
/// The object references use the same media type returned by
/// [`super::inactive::InactiveObjectsQuery`].
///
/// The endpoint aborts activation when the passed objects are assigned to
/// different CTS transport requests. This conflict must then be explicitly
/// accepted by passing `preaudit=false`.
///
/// Backend handler: `CL_SEU_ADT_RES_ACTIVATION`
#[derive(Debug)]
pub struct ActivationRun {
    /// The mode of the run. `Check` does not actually activate any objects.
    mode: ActivationRunMode,
    /// Repository objects included in the activation worklist.
    objects: ObjectReferences,
    /// Whether activation should be forced. Off by default
    forced: Option<bool>,
    /// Whether a request preaudit should be performed. On by default
    preaudit: bool,
}

impl ActivationRun {
    const MEDIA_TYPE: &'static str = "application/xml";

    /// Creates an activation or check run for `objects`.
    pub fn new(mode: ActivationRunMode, objects: ObjectReferences) -> Self {
        Self {
            mode,
            objects,
            forced: None,
            preaudit: true,
        }
    }

    /// Allow distinct transport requests. This will prevent the preaudit
    /// from aborting the activation.
    pub fn allow_distinct_transports(&mut self, allowed: bool) -> &mut Self {
        self.preaudit = !allowed;
        self
    }

    /// Adds an object to the activation list
    pub fn push_object<T>(&mut self, object: &ObjectRef<T>) -> &mut Self
    where
        for<'a> &'a ObjectRef<T>: Into<AdvertisedObjectReference>,
    {
        self.objects.objects.push(object.into());
        self
    }

    /// Requests forced Workbench activation.
    ///
    /// This seems to promote the object to the active version even if activation
    /// fails. Only works when explicitly supported for an error.
    pub fn forced(&mut self, forced: bool) -> &mut Self {
        self.forced = Some(forced);
        self
    }
}

impl Operation for ActivationRun {
    type Kind = Stateless;
    type Response = ActivationRunMessages;
    type Target = Advertised;

    fn encode(&self) -> Result<EncodedOperation<Self::Target>, EncodeError> {
        for object in &self.objects.objects {
            let Some(object_type) = object.object_type.as_ref() else {
                continue;
            };
            if crate::objects::descriptors::requires_parent(object_type)
                && object.parent_uri.is_none()
            {
                return Err(ObjectError::ParentObjectRequired {
                    object_type: object_type.clone(),
                }
                .into());
            }
        }
        let body = serde_xml_rs::SerdeXml::new()
            .namespace("adtcore", "http://www.sap.com/adt/core")
            .to_string(&self.objects)
            .map_err(ObjectError::InvalidRequest)?;
        let mut request = CollectionTarget::new(ACTIVATE_OBJECTS).operation(Method::POST);
        request.push_query("method", self.mode.as_str());
        request.push_query("preaudit", self.preaudit.to_string());
        if let Some(forced) = self.forced {
            request.push_query("forced", forced.to_string());
        }
        request.set_accept(Self::MEDIA_TYPE);
        request.set_content_type(Self::MEDIA_TYPE);
        request.set_body(body);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        if response.body().is_empty() {
            return Ok(ActivationRunMessages::default());
        }

        let messages =
            serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;

        Ok(messages)
    }
}

impl<T> ObjectRef<T>
where
    for<'a> &'a ObjectRef<T>: Into<AdvertisedObjectReference>,
{
    /// Creates an activation run for this object.
    pub fn activation(&self) -> ActivationRun {
        ActivationRun::new(
            ActivationRunMode::Activate,
            ObjectReferences {
                objects: vec![self.into()],
            },
        )
    }
}

impl<T: ObjectType> Object<T> {
    /// Creates an activation run for this loaded object.
    pub fn activation(&self) -> ActivationRun {
        self.reference().activation()
    }
}

impl AnyObject {
    /// Creates an activation run for this loaded object.
    pub fn activation(&self) -> ActivationRun {
        self.reference().activation()
    }
}

/// Messages about the result of an activation run. Mainly errors that blocked it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "chkl:messages")]
pub struct ActivationRunMessages {
    /// Execution phases reached by the backend.
    #[serde(rename = "chkl:properties")]
    pub properties: ActivationRunProperties,
    /// Diagnostics emitted by the activation run.
    #[serde(rename = "msg", default)]
    pub messages: Vec<ActivationRunMessage>,
}

/// Execution phases reached by an activation run.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename = "chkl:properties")]
pub struct ActivationRunProperties {
    /// Whether the object check phase ran.
    #[serde(rename = "@checkExecuted")]
    pub check_executed: bool,
    /// Whether the activation phase ran.
    #[serde(rename = "@activationExecuted")]
    pub activation_executed: bool,
    /// Whether the generation phase ran.
    #[serde(rename = "@generationExecuted")]
    pub generation_executed: bool,
}

/// One diagnostic emitted by an activation run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename = "msg")]
pub struct ActivationRunMessage {
    /// Backend-formatted description of the affected object or component.
    #[serde(rename = "@objDescr")]
    pub description: String,
    /// SAP message severity, such as `E` for an error.
    #[serde(rename = "@type")]
    pub message_type: String,
    /// Ordinal position of the diagnostic in the response.
    #[serde(rename = "@line")]
    pub line: i32,
    /// Raw source link, including any source-range fragment.
    #[serde(rename = "@href")]
    pub href: String,
    /// Raw backend value indicating whether forced activation is supported.
    #[serde(rename = "@forceSupported")]
    pub force_supported: String,
    /// Short diagnostic text lines.
    #[serde(rename = "shortText")]
    pub short_text: ActivationRunMessageText,
    /// Extended diagnostic text lines, when supplied.
    #[serde(rename = "longText", default)]
    pub long_text: Option<ActivationRunMessageText>,
    /// Advertised links, including quick-fix relations.
    #[serde(rename = "atom:link", default)]
    pub links: Vec<AdvertisedLink>,
}

/// A string table used for short and extended checklist text.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct ActivationRunMessageText {
    /// Human-readable text lines in backend order.
    #[serde(rename = "txt", default)]
    pub lines: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdtUri, Class, FunctionGroup, FunctionModule};
    use http::header;

    const ACTIVATION_MESSAGES_XML: &[u8] =
        include_bytes!("../../../tests/fixtures/activation-run-messages.xml");
    #[test]
    fn activation_run_posts_flags_and_namespaced_object_references() {
        let object = ObjectRef::<Class>::for_test(
            "Z_SYNTAX_TEST",
            AdtUri::parse("/sap/bc/adt/oo/classes/z_syntax_test").unwrap(),
        );
        let mut run = object.activation();
        run.allow_distinct_transports(true).forced(true);

        let request = run.encode().unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.query(),
            [
                ("method".to_owned(), "activate".to_owned()),
                ("preaudit".to_owned(), "false".to_owned()),
                ("forced".to_owned(), "true".to_owned()),
            ]
        );
        assert_eq!(request.headers()[header::ACCEPT], ActivationRun::MEDIA_TYPE);
        assert_eq!(
            request.headers()[header::CONTENT_TYPE],
            ActivationRun::MEDIA_TYPE
        );
        let body = std::str::from_utf8(request.body()).unwrap();
        assert!(body.contains("xmlns:adtcore=\"http://www.sap.com/adt/core\""));
        assert!(body.contains("adtcore:type=\"CLAS/OC\""));
        assert!(body.contains("adtcore:name=\"Z_SYNTAX_TEST\""));
    }

    #[test]
    fn child_activation_includes_its_parent_uri() {
        let group = ObjectRef::<FunctionGroup>::new(
            "Z_TEST_GROUP".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group").unwrap(),
        );
        let module = ObjectRef::<FunctionModule>::new(
            "ZZZZFUNC".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc").unwrap(),
        )
        .with_parent(&group);

        let request = module.activation().encode().unwrap();
        let body = std::str::from_utf8(request.body()).unwrap();

        assert!(body.contains("adtcore:parentUri=\"/sap/bc/adt/functions/groups/z_test_group\""));
    }

    #[test]
    fn child_activation_requires_parent_identity() {
        let module = ObjectRef::<FunctionModule>::new(
            "ZZZZFUNC".to_owned(),
            AdtUri::parse("/sap/bc/adt/functions/groups/z_test_group/fmodules/zzzzfunc").unwrap(),
        );

        assert!(matches!(
            module.activation().encode(),
            Err(EncodeError::Object(ObjectError::ParentObjectRequired { object_type }))
                if object_type == FunctionModule::WORKBENCH_TYPE
        ));
    }

    #[test]
    fn parses_activation_run_messages() {
        let result: ActivationRunMessages =
            serde_xml_rs::from_reader(ACTIVATION_MESSAGES_XML).unwrap();

        assert_eq!(result.properties, ActivationRunProperties::default());
        assert_eq!(result.messages.len(), 18);

        let first = &result.messages[0];
        assert_eq!(first.description, "Class Z_SYNTAX_TEST, Public Section");
        assert_eq!(first.message_type, "E");
        assert_eq!(first.line, 1);
        assert_eq!(
            first.href,
            "/sap/bc/adt/oo/classes/z_syntax_test/source/main#start=207,19"
        );
        assert_eq!(first.force_supported, "true");
        assert_eq!(
            first.short_text.lines,
            ["Implementation missing for method \"CLS_METHODS_MULTIPLE1\"."]
        );
        assert!(first.long_text.is_none());
        assert_eq!(first.links.len(), 1);
        assert_eq!(first.links[0].href, "art.syntax:G(2");
        assert_eq!(
            first.links[0].relation.as_deref(),
            Some("http://www.sap.com/adt/categories/quickfixes")
        );

        let last = result.messages.last().unwrap();
        assert_eq!(last.line, 18);
        assert_eq!(
            last.short_text.lines,
            ["Implementation missing for method \"SINGLE_METHOD_USING_ESCAPE\"."]
        );
    }
}
