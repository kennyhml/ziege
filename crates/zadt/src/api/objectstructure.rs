use http::{Method, StatusCode};
use serde::Deserialize;

use crate::{
    AdtUri, AdvertisedLink, AnyObject, Client, ClientState, Object, ObjectError,
    ObjectStructureRef, ObjectVersion, Operation, OperationError, OperationResponse, PropertyModel,
    Relations, ResponseError, Stateless, Structure,
    protocol::AdtRequest,
    resource::resolve_href,
    vocabulary::{media_type, query_parameter, relation},
};

/// Fetches the structure representation advertised by an object.
///
/// A more generic endpoint `/sap/bc/adt/repository/objectstructure`
/// exists but should be considered legacy and is not currently supported.
///
/// Because the endpoint is advertised by the objects relations, a loaded
/// object is required to call the method. The response is stable across
/// different object types, making the operation suitable for different
/// object types.
///
/// Program / Include handler: `CL_SEDI_ADT_RES_OBJ_STRUCTURE`
/// Class handler: `CL_SRC_ADT_RES_OBJ_STRUC`
#[derive(Clone, Debug)]
pub struct ObjectStructureQuery {
    /// The advertised resource
    pub resource: ObjectStructureRef,
    /// The version of the object (active, inactive..)
    version: Option<ObjectVersion>,
    /// Whether class parents or implemented interfaces should be expanded
    inherited_members: Option<bool>,
    /// Whether short descriptions should be included in the response.
    with_short_descriptions: Option<bool>,
}

impl ObjectStructureQuery {
    fn new(structure: ObjectStructureRef) -> Self {
        Self {
            resource: structure,
            version: None,
            inherited_members: None,
            with_short_descriptions: None,
        }
    }

    /// Selects the object version used to generate the structure.
    #[must_use]
    pub fn version(mut self, version: ObjectVersion) -> Self {
        self.version = Some(version);
        self
    }

    /// Includes the objects short descriptions, such as method documentation.
    #[must_use]
    pub fn short_descriptions(mut self, short_descriptions: bool) -> Self {
        self.with_short_descriptions = Some(short_descriptions);
        self
    }

    /// Requests members inherited from superclasses and interfaces.
    #[must_use]
    pub fn inherited_members(mut self, inherited_members: bool) -> Self {
        self.inherited_members = Some(inherited_members);
        self
    }
}

impl<S: ClientState> Operation<S> for ObjectStructureQuery {
    type Response = ObjectStructure;
    type Kind = Stateless;

    fn request(&self, _client: &Client<S>) -> Result<AdtRequest, OperationError> {
        let mut request = AdtRequest::new(Method::GET, self.resource.uri.clone());
        for (name, value) in &self.resource.query {
            if self.version.is_some() && name == query_parameter::VERSION {
                continue;
            }
            if self.inherited_members.is_some() && name == query_parameter::INHERITED_MEMBERS {
                continue;
            }
            if self.with_short_descriptions.is_some()
                && name == query_parameter::WITH_SHORT_DESCRIPTIONS
            {
                continue;
            }
            request.push_query(name, value);
        }
        if let Some(version) = self.version {
            request.push_query(query_parameter::VERSION, version.as_str());
        }
        if self.with_short_descriptions == Some(true) {
            request.push_query(query_parameter::WITH_SHORT_DESCRIPTIONS, "true");
        }
        // Only the presence of the query parameter makes it expand inherited members
        // Seems like a bug in the ADT handler, so we need a workaround
        if self.inherited_members == Some(true) {
            request.push_query(query_parameter::INHERITED_MEMBERS, "true");
        }
        request.set_accept(media_type::OBJECT_STRUCTURE_V2);
        Ok(request)
    }

    fn decode(&self, response: OperationResponse) -> Result<Self::Response, ResponseError> {
        response.require_status(StatusCode::OK)?;
        response.require_content_type(&[media_type::OBJECT_STRUCTURE_V2])?;

        let raw: RawObjectStructureElement =
            serde_xml_rs::from_reader(response.body()).map_err(ObjectError::InvalidResponse)?;
        let root = ObjectStructureElement::from_raw(raw, response.request_target())?;
        Ok(ObjectStructure {
            reference: self.resource.clone(),
            root,
        })
    }
}

impl ObjectStructureRef {
    /// Creates a query for this advertised structure resource.
    pub fn query(&self) -> ObjectStructureQuery {
        ObjectStructureQuery::new(self.clone())
    }
}

impl<T: Structure> Object<T> {
    /// Creates a query for the object-structure relation advertised by this object.
    pub fn object_structure(&self) -> Result<ObjectStructureQuery, ObjectError> {
        ObjectStructureRef::from_relations(self.reference().erase(), self.properties.links())?
            .map(|reference| reference.query())
            .ok_or(ObjectError::MissingRelation {
                relation: relation::OBJECT_STRUCTURE,
            })
    }
}

impl AnyObject {
    /// Creates an object-structure query through the runtime descriptor.
    pub fn object_structure(&self) -> Result<ObjectStructureQuery, ObjectError> {
        let Some(descriptor) = self.reference().descriptor() else {
            return Err(self.reference().unsupported_capability("object structure"));
        };
        descriptor
            .object_structure(self.reference(), &self.properties)?
            .map(|reference| reference.query())
            .ok_or(ObjectError::MissingRelation {
                relation: relation::OBJECT_STRUCTURE,
            })
    }
}

/// A structural representation returned for one repository object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectStructure {
    /// The related resource that was queried.
    pub reference: ObjectStructureRef,

    /// The root element representing the repository object.
    pub root: ObjectStructureElement,
}

/// One recursively nested element in an object-structure representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectStructureElement {
    /// The element name exactly as returned by SAP.
    pub name: String,

    /// The opaque ADT element type, such as `CLAS/OM` or `PROG/PU`.
    pub object_type: String,

    /// The effective base URI used to resolve this element's links.
    pub base_uri: AdtUri,

    pub description: Option<String>,
    pub type_description: Option<String>,
    pub class_or_interface_name: Option<String>,
    pub enclosing_object_name: Option<String>,
    pub visibility: Option<String>,
    pub level: Option<String>,
    pub invisible: Option<bool>,
    pub redefinition: Option<bool>,
    pub read_only: Option<bool>,
    pub is_abstract: Option<bool>,
    pub is_final: Option<bool>,
    pub is_test_class: Option<bool>,
    pub is_constructor: Option<bool>,
    pub is_test_method: Option<bool>,
    pub is_event_handler: Option<bool>,
    pub is_constant: Option<bool>,
    pub is_external_reference: Option<bool>,

    relations: Relations,

    /// Direct child elements in backend response order.
    pub children: Vec<ObjectStructureElement>,
}

impl ObjectStructureElement {
    /// Returns this element's source and navigation relations.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }

    fn from_raw(
        raw: RawObjectStructureElement,
        inherited_base: &AdtUri,
    ) -> Result<Self, ObjectError> {
        let base_uri = match raw.base_uri.as_deref() {
            Some(href) => {
                resolve_href(inherited_base, href)
                    .map_err(|source| ObjectError::InvalidLink {
                        href: href.to_owned(),
                        source,
                    })?
                    .target
            }
            None => inherited_base.clone(),
        };
        let children = raw
            .children
            .into_iter()
            .map(|child| Self::from_raw(child, &base_uri))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            name: raw.name,
            object_type: raw.object_type,
            base_uri: base_uri.clone(),
            description: raw.description,
            type_description: raw.type_description,
            class_or_interface_name: raw.class_or_interface_name,
            enclosing_object_name: raw.enclosing_object_name,
            visibility: raw.visibility,
            level: raw.level,
            invisible: raw.invisible,
            redefinition: raw.redefinition,
            read_only: raw.read_only,
            is_abstract: raw.is_abstract,
            is_final: raw.is_final,
            is_test_class: raw.is_test_class,
            is_constructor: raw.is_constructor,
            is_test_method: raw.is_test_method,
            is_event_handler: raw.is_event_handler,
            is_constant: raw.is_constant,
            is_external_reference: raw.is_external_reference,
            relations: Relations::for_base(base_uri, raw.links),
            children,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename = "abapsource:objectStructureElement")]
struct RawObjectStructureElement {
    #[serde(rename = "@xml:base")]
    base_uri: Option<String>,
    #[serde(rename = "@adtcore:name")]
    name: String,
    #[serde(rename = "@adtcore:type")]
    object_type: String,
    #[serde(rename = "@description")]
    description: Option<String>,
    #[serde(rename = "@typeDescription")]
    type_description: Option<String>,
    #[serde(rename = "@clif_name")]
    class_or_interface_name: Option<String>,
    #[serde(rename = "@object_name")]
    enclosing_object_name: Option<String>,
    #[serde(rename = "@visibility")]
    visibility: Option<String>,
    #[serde(rename = "@level")]
    level: Option<String>,
    #[serde(rename = "@invisible")]
    invisible: Option<bool>,
    #[serde(rename = "@redefinition")]
    redefinition: Option<bool>,
    #[serde(rename = "@readOnly")]
    read_only: Option<bool>,
    #[serde(rename = "@abstract")]
    is_abstract: Option<bool>,
    #[serde(rename = "@final")]
    is_final: Option<bool>,
    #[serde(rename = "@testClass")]
    is_test_class: Option<bool>,
    #[serde(rename = "@constructor")]
    is_constructor: Option<bool>,
    #[serde(rename = "@testMethod")]
    is_test_method: Option<bool>,
    #[serde(rename = "@eventHandler")]
    is_event_handler: Option<bool>,
    #[serde(rename = "@constant")]
    is_constant: Option<bool>,
    #[serde(rename = "@isExternalRef")]
    is_external_reference: Option<bool>,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
    #[serde(rename = "abapsource:objectStructureElement", default)]
    children: Vec<RawObjectStructureElement>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use crate::{
        AdtResponse, Class, ClassProperties, ObjectRef, Program, ProgramProperties, PropertyModel,
        Transport, TransportError,
    };

    const STRUCTURE_XML: &[u8] = include_bytes!("../../tests/fixtures/object-structure-class.xml");
    const CLASS_XML: &[u8] = include_bytes!("../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const PROGRAM_XML: &[u8] = include_bytes!("../../tests/fixtures/program-z-test.xml");

    struct UnusedTransport;

    #[async_trait]
    impl Transport for UnusedTransport {
        async fn send(&self, _request: AdtRequest) -> Result<AdtResponse, TransportError> {
            unreachable!("request construction tests do not send requests")
        }
    }

    fn structure_reference() -> ObjectStructureRef {
        let object = ObjectRef::<Class>::for_test(
            "ZMYNEWCLASSV7",
            AdtUri::parse("/sap/bc/adt/oo/classes/zmynewclassv7").unwrap(),
        );
        ObjectStructureRef::new(
            object.erase(),
            AdtUri::parse("/sap/bc/adt/oo/classes/zmynewclassv7/objectstructure").unwrap(),
        )
    }

    #[test]
    fn loaded_classes_resolve_the_advertised_structure_relation() {
        let reference = ObjectRef::<Class>::for_test(
            "CL_ADT_URI_MAPPER",
            AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
        );
        let properties = ClassProperties::from_xml_for(CLASS_XML, &reference).unwrap();
        let runtime_properties = serde_json::to_value(&properties).unwrap();
        let runtime_reference = reference.erase();
        let object = Object::new(
            reference,
            "application/vnd.sap.adt.oo.classes.v4+xml",
            None,
            properties,
        );

        let query = object.object_structure().unwrap();

        assert_eq!(
            query.resource.uri.as_str(),
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/objectstructure"
        );
        assert!(query.resource.query.is_empty());

        let runtime_object = AnyObject::new(
            runtime_reference,
            "application/vnd.sap.adt.oo.classes.v4+xml",
            None,
            runtime_properties,
        );
        assert_eq!(
            runtime_object.object_structure().unwrap().resource,
            query.resource
        );
    }

    #[test]
    fn loaded_programs_resolve_the_advertised_structure_relation() {
        let reference = ObjectRef::<Program>::for_test(
            "Z_TEST",
            AdtUri::parse("/sap/bc/adt/programs/programs/z_test").unwrap(),
        );
        let properties = ProgramProperties::from_xml_for(PROGRAM_XML, &reference).unwrap();
        let object = Object::new(
            reference,
            "application/vnd.sap.adt.programs.programs.v3+xml",
            None,
            properties,
        );

        let query = object.object_structure().unwrap();

        assert_eq!(
            query.resource.uri.as_str(),
            "/sap/bc/adt/programs/programs/z_test/objectstructure"
        );
    }

    #[test]
    fn query_uses_v2_and_overrides_advertised_options() {
        let mut structure = structure_reference();
        structure
            .query
            .push((query_parameter::VERSION.to_owned(), "active".to_owned()));
        structure.query.push((
            query_parameter::INHERITED_MEMBERS.to_owned(),
            "legacy".to_owned(),
        ));
        let query = structure
            .query()
            .version(ObjectVersion::Inactive)
            .inherited_members(true);

        let request = query.request(&Client::new(UnusedTransport)).unwrap();

        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.target(), &structure.uri);
        assert_eq!(
            request.query(),
            [
                ("version".to_owned(), "inactive".to_owned()),
                ("inheritedMembers".to_owned(), "true".to_owned()),
            ]
        );
        assert_eq!(
            request.headers().get(header::ACCEPT).unwrap(),
            media_type::OBJECT_STRUCTURE_V2
        );
    }

    #[test]
    fn false_inherited_members_omits_the_presence_based_parameter() {
        let mut structure = structure_reference();
        structure.query.push((
            query_parameter::INHERITED_MEMBERS.to_owned(),
            "true".to_owned(),
        ));

        let request = structure
            .query()
            .inherited_members(false)
            .request(&Client::new(UnusedTransport))
            .unwrap();

        assert!(request.query().is_empty());
    }

    #[test]
    fn short_descriptions_uses_the_explicit_true_value() {
        let request = structure_reference()
            .query()
            .short_descriptions(true)
            .request(&Client::new(UnusedTransport))
            .unwrap();

        assert_eq!(
            request.query(),
            [("withShortDescriptions".to_owned(), "true".to_owned())]
        );
    }

    #[test]
    fn decodes_recursive_elements_and_resolves_xml_base() {
        let structure = structure_reference();
        let query = structure.query();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(media_type::OBJECT_STRUCTURE_V2),
        );
        let response = OperationResponse::new(
            AdtResponse::new(StatusCode::OK, headers, STRUCTURE_XML.to_vec()),
            structure.uri.clone(),
        );

        let result =
            <ObjectStructureQuery as Operation<crate::Initial>>::decode(&query, response).unwrap();

        assert_eq!(result.reference, structure);
        assert_eq!(result.root.name, "ZMYNEWCLASSV7");
        assert_eq!(result.root.object_type, "CLAS/OC");
        assert_eq!(result.root.visibility.as_deref(), Some("public"));
        assert_eq!(result.root.is_final, Some(true));
        assert_eq!(result.root.children.len(), 4);

        let constant = &result.root.children[1];
        assert_eq!(constant.name, "GC_COMPANY_ID");
        assert_eq!(constant.is_constant, Some(true));
        assert_eq!(constant.read_only, Some(true));
        assert_eq!(constant.level.as_deref(), Some("static"));

        let method = &result.root.children[2];
        assert_eq!(
            method.class_or_interface_name.as_deref(),
            Some("ZMYNEWCLASSV7")
        );
        let implementation = method.relations().iter().next().unwrap().unwrap();
        assert_eq!(
            implementation.target.as_str(),
            "/sap/bc/adt/oo/classes/zmynewclassv7/source/main"
        );
        assert_eq!(
            implementation.fragment.as_deref(),
            Some("start=23,9;end=23,57")
        );
        assert_eq!(implementation.media_type.as_deref(), Some("CLAS/OM"));

        let external = &result.root.children[3];
        assert_eq!(external.is_external_reference, Some(true));
        assert_eq!(external.description.as_deref(), Some("Text Elements"));
    }
}
