use serde::{Deserialize, Serialize};

use crate::{
    AdtUri, Class, ClassSourceComponent, EnhancementImplementationsRef, EntityTag,
    GlobalWorkbenchType, HtmlSourceRef, MediaVersionNegotiation, ObjectEnhancementOptionsRef,
    ObjectError, ObjectRef, ObjectStructureRef, ObjectType, ObjectVersion, Package, ResponseError,
    SourceEnhancementOptionsRef, SourceRef, SourceVersionsRef, SyntaxConfiguration,
    TextElementsRef,
    resource::{AdtLinkError, AdvertisedLink, Relations, resolve_href},
};

const CLASS_INCLUDE_TYPE: GlobalWorkbenchType = GlobalWorkbenchType::new("CLAS/I");

/// The SAP media-type version used to decode class properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClassPropertiesVersion {
    /// Class properties V2.
    V2,
    /// Class properties V3.
    V3,
    /// Class properties V4.
    V4,
}

impl MediaVersionNegotiation for ClassPropertiesVersion {
    const SUPPORTED: &'static [Self] = &[Self::V4, Self::V3, Self::V2];

    fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.oo.classes.v2+xml",
            Self::V3 => "application/vnd.sap.adt.oo.classes.v3+xml",
            Self::V4 => "application/vnd.sap.adt.oo.classes.v4+xml",
        }
    }
}

/// Class properties tagged with the media-type version returned by ADT.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "mediaVersion", content = "properties", rename_all = "lowercase")]
#[non_exhaustive]
pub enum ClassProperties {
    /// A V2 class-properties response.
    V2(Box<ClassPropertiesV2>),
    /// A V3 class-properties response.
    V3(Box<ClassPropertiesV3>),
    /// A V4 class-properties response.
    V4(Box<ClassPropertiesV4>),
}

impl ClassProperties {
    /// Returns the response media-type version.
    pub fn media_version(&self) -> ClassPropertiesVersion {
        match self {
            Self::V2(_) => ClassPropertiesVersion::V2,
            Self::V3(_) => ClassPropertiesVersion::V3,
            Self::V4(_) => ClassPropertiesVersion::V4,
        }
    }

    /// Returns the response entity tag, when present.
    pub fn etag(&self) -> Option<&EntityTag> {
        match self {
            Self::V2(class) | Self::V3(class) | Self::V4(class) => class.etag.as_ref(),
        }
    }

    pub(crate) fn parse(
        resource: &ObjectRef<Class>,
        media_version: ClassPropertiesVersion,
        body: &[u8],
        etag: Option<EntityTag>,
    ) -> Result<Self, ResponseError> {
        let raw: RawClassProperties =
            serde_xml_rs::from_reader(body).map_err(ObjectError::InvalidResponse)?;
        let properties = ClassPropertiesV4::from_raw(resource.clone(), raw, etag)?;
        Ok(match media_version {
            ClassPropertiesVersion::V2 => Self::V2(Box::new(properties)),
            ClassPropertiesVersion::V3 => Self::V3(Box::new(properties)),
            ClassPropertiesVersion::V4 => Self::V4(Box::new(properties)),
        })
    }
}

/// The V2 class-properties representation uses the shared class payload schema.
pub type ClassPropertiesV2 = ClassPropertiesV4;

/// The V3 class-properties representation uses the shared class payload schema.
pub type ClassPropertiesV3 = ClassPropertiesV4;

/// Properties of an ABAP class.
///
/// V2 through V4 use the same observed payload shape. V4 additionally supplies
/// `abap_language_version`, which remains optional for older responses.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassPropertiesV4 {
    /// The class resource that was fetched.
    pub reference: ObjectRef<Class>,
    /// The class name supplied by SAP.
    pub name: String,
    /// The repository object type, normally `CLAS/OC`.
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which the class was last changed.
    pub last_changed: String,
    /// The active or inactive object version.
    pub version: ObjectVersion,
    /// The timestamp at which the class was created.
    pub created_at: String,
    /// The user who last changed the class.
    pub changed_by: String,
    /// The user who created the class.
    pub created_by: String,
    /// The class description.
    pub description: String,
    /// The maximum class-description length.
    pub description_text_limit: u32,
    /// The class's logon language.
    pub language: String,
    /// The user responsible for the class.
    pub responsible: String,
    /// The class's master language.
    pub master_language: String,
    /// The class's master system.
    pub master_system: String,
    /// The configured ABAP language version when supplied by the media version.
    pub abap_language_version: Option<String>,
    /// The purpose assigned to this source object by SAP.
    pub source_object_status: Option<String>,
    /// Whether fixed-point arithmetic is enabled.
    pub fix_point_arithmetic: bool,
    /// Whether the active Unicode check is enabled.
    pub unicode_check_active: bool,
    /// Whether this class is maintained through a higher-level model.
    pub modeled: bool,
    /// The semantic class category, such as `generalObjectType`.
    pub category: String,
    /// Whether the class is final.
    pub is_final: bool,
    /// Whether the class is abstract.
    pub is_abstract: bool,
    /// The class visibility.
    pub visibility: String,
    /// An optional class state supplied by SAP.
    pub state: Option<String>,
    /// Whether shared-memory support is enabled.
    pub shared_memory_enabled: bool,
    /// Whether SAP generated the constructor.
    pub constructor_generated: bool,
    /// Whether SAP explicitly marks this class as having tests.
    pub has_tests: bool,
    /// The package containing the class.
    pub package: ObjectRef<Package>,
    /// The syntax configuration advertised for the class.
    pub syntax_configuration: Option<SyntaxConfiguration>,
    /// Interfaces implemented by the class.
    pub interfaces: Vec<ClassObjectReference>,
    /// The direct superclass, when advertised.
    pub super_class: Option<ClassObjectReference>,
    /// The assigned message class, when advertised.
    pub message_class: Option<ClassObjectReference>,
    /// The root entity, when advertised.
    pub root_entity: Option<ClassObjectReference>,
    /// The required main source advertised by this class.
    pub main_source: ClassSourceProperties,
    /// Secondary source components currently advertised by this class.
    pub source_components: Vec<ClassSourceProperties>,
    /// The entity tag of these class properties, when present.
    pub etag: Option<EntityTag>,
    relations: Relations,
}

impl ClassPropertiesV4 {
    /// Returns the class's advertised links without resolving them eagerly.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }

    /// Finds an advertised secondary source component.
    pub fn source(&self, component: ClassSourceComponent) -> Option<&ClassSourceProperties> {
        self.source_components
            .iter()
            .find(|source| source.component == Some(component))
    }

    /// Returns the required main source component.
    pub fn main_source(&self) -> &ClassSourceProperties {
        &self.main_source
    }

    /// Resolves the advertised object-structure resource, when present.
    pub fn object_structure(&self) -> Result<Option<ObjectStructureRef>, AdtLinkError> {
        self.relations.get()
    }

    /// Resolves the advertised text-elements resource, when present.
    pub fn text_elements(&self) -> Result<Option<TextElementsRef>, AdtLinkError> {
        self.relations.get()
    }

    /// Resolves the advertised enhancement implementations, when present.
    pub fn enhancement_implementations(
        &self,
    ) -> Result<Option<EnhancementImplementationsRef>, AdtLinkError> {
        self.relations.get()
    }

    /// Resolves the advertised object enhancement options, when present.
    pub fn enhancement_options(&self) -> Result<Option<ObjectEnhancementOptionsRef>, AdtLinkError> {
        self.relations.get()
    }

    fn from_raw(
        reference: ObjectRef<Class>,
        raw: RawClassProperties,
        etag: Option<EntityTag>,
    ) -> Result<Self, ObjectError> {
        if raw.object_type != Class::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: Class::WORKBENCH_TYPE,
                actual: raw.object_type,
            });
        }
        let version = parse_object_version(&raw.version)?;
        let package = package_reference(raw.package)?;
        let sources = raw
            .sources
            .into_iter()
            .map(|source| ClassSourceProperties::from_raw(&reference, source))
            .collect::<Result<Vec<_>, _>>()?;

        let mut main_source = None;
        let mut source_components: Vec<ClassSourceProperties> =
            Vec::with_capacity(sources.len().saturating_sub(1));
        for source in sources {
            if source.include_type == "main" {
                if main_source.is_some() {
                    return Err(ObjectError::DuplicateSourceComponent {
                        component: "main".to_owned(),
                    });
                }
                main_source = Some(source);
                continue;
            }

            if let Some(component) = source.component
                && source_components
                    .iter()
                    .any(|previous| previous.component == Some(component))
            {
                return Err(ObjectError::DuplicateSourceComponent {
                    component: component.as_str().to_owned(),
                });
            }
            source_components.push(source);
        }
        let main_source = main_source.ok_or(ObjectError::MissingRelation {
            relation: "main class source",
        })?;
        let syntax_configuration = raw
            .syntax_configuration
            .and_then(|syntax| syntax.language)
            .map(|language| {
                SyntaxConfiguration::new(
                    reference.erase(),
                    language.version,
                    language.description,
                    language.links,
                )
            });
        let interfaces = raw
            .interfaces
            .into_iter()
            .filter(|related| !related.is_empty())
            .map(|related| ClassObjectReference::from_raw(reference.uri(), related))
            .collect::<Result<Vec<_>, _>>()?;
        let super_class = related_object(reference.uri(), raw.super_class)?;
        let message_class = related_object(reference.uri(), raw.message_class)?;
        let root_entity = related_object(reference.uri(), raw.root_entity)?;
        let relations = Relations::new(reference.erase(), raw.links);

        Ok(Self {
            reference,
            name: raw.name,
            object_type: raw.object_type,
            last_changed: raw.last_changed,
            version,
            created_at: raw.created_at,
            changed_by: raw.changed_by,
            created_by: raw.created_by,
            description: raw.description,
            description_text_limit: raw.description_text_limit,
            language: raw.language,
            responsible: raw.responsible,
            master_language: raw.master_language,
            master_system: raw.master_system,
            abap_language_version: raw.abap_language_version,
            source_object_status: raw.source_object_status,
            fix_point_arithmetic: raw.fix_point_arithmetic,
            unicode_check_active: raw.unicode_check_active,
            modeled: raw.modeled,
            category: raw.category,
            is_final: raw.is_final,
            is_abstract: raw.is_abstract,
            visibility: raw.visibility,
            state: raw.state,
            shared_memory_enabled: raw.shared_memory_enabled,
            constructor_generated: raw.constructor_generated,
            has_tests: raw.has_tests,
            package,
            syntax_configuration,
            interfaces,
            super_class,
            message_class,
            root_entity,
            main_source,
            source_components,
            etag,
            relations,
        })
    }
}

/// Metadata and relations for one source component in a class manifest.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassSourceProperties {
    /// The recognized secondary component, or `None` for the main or an unknown source type.
    pub component: Option<ClassSourceComponent>,
    /// The exact include type supplied by SAP.
    pub include_type: String,
    /// The include object type, normally `CLAS/I`.
    pub object_type: GlobalWorkbenchType,
    /// The timestamp at which this source was last changed.
    pub last_changed: String,
    /// The active or inactive source version.
    pub version: ObjectVersion,
    /// The timestamp at which this source was created.
    pub created_at: String,
    /// The user who last changed this source.
    pub changed_by: String,
    /// The user who created this source.
    pub created_by: String,
    /// The advertised plain-text source representation.
    pub source: SourceRef,
    relations: Relations,
}

impl ClassSourceProperties {
    /// Returns this source component's advertised links.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }

    /// Resolves the rendered HTML source, when present.
    pub fn html_source(&self) -> Result<Option<HtmlSourceRef>, AdtLinkError> {
        self.relations.get()
    }

    /// Resolves the source version-history resource, when present.
    pub fn versions(&self) -> Result<Option<SourceVersionsRef>, AdtLinkError> {
        self.relations.get()
    }

    /// Resolves the source enhancement options, when present.
    pub fn enhancement_options(&self) -> Result<Option<SourceEnhancementOptionsRef>, AdtLinkError> {
        self.relations.get()
    }

    fn from_raw(reference: &ObjectRef<Class>, raw: RawClassSource) -> Result<Self, ObjectError> {
        if raw.object_type != CLASS_INCLUDE_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: CLASS_INCLUDE_TYPE,
                actual: raw.object_type,
            });
        }
        let version = parse_object_version(&raw.version)?;
        let relations = Relations::new(reference.erase(), raw.links);
        let source: SourceRef = relations.get()?.ok_or(ObjectError::MissingRelation {
            relation: "plain-text class source",
        })?;
        let declared_source = resolve_href(reference.uri(), &raw.source_uri).map_err(|source| {
            ObjectError::InvalidLink {
                href: raw.source_uri.clone(),
                source,
            }
        })?;
        if declared_source.target != source.uri {
            return Err(ObjectError::RelationMismatch {
                relation: "class source",
                declared: declared_source.target.to_string(),
                advertised: source.uri.to_string(),
            });
        }

        let is_main = raw.include_type == "main";
        let component = (!is_main)
            .then(|| ClassSourceComponent::from_name(&raw.include_type))
            .flatten();
        if is_main {
            let expected = reference.source();
            if expected.uri != source.uri {
                return Err(ObjectError::RelationMismatch {
                    relation: "main class source",
                    declared: expected.uri.to_string(),
                    advertised: source.uri.to_string(),
                });
            }
        } else if let Some(component) = component {
            let expected = reference.component_source(component);
            if expected.uri != source.uri {
                return Err(ObjectError::RelationMismatch {
                    relation: "class source component",
                    declared: expected.uri.to_string(),
                    advertised: source.uri.to_string(),
                });
            }
        } else {
            let owner_prefix = format!("{}/", reference.uri());
            if !source.uri.as_str().starts_with(&owner_prefix) {
                return Err(ObjectError::RelationMismatch {
                    relation: "class source owner",
                    declared: owner_prefix,
                    advertised: source.uri.to_string(),
                });
            }
        }

        Ok(Self {
            component,
            include_type: raw.include_type,
            object_type: raw.object_type,
            last_changed: raw.last_changed,
            version,
            created_at: raw.created_at,
            changed_by: raw.changed_by,
            created_by: raw.created_by,
            source,
            relations,
        })
    }
}

/// An object referenced by a class-properties representation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassObjectReference {
    pub uri: Option<AdtUri>,
    pub object_type: Option<String>,
    pub name: Option<String>,
    pub package_name: Option<String>,
    pub description: Option<String>,
}

impl ClassObjectReference {
    fn from_raw(base: &AdtUri, raw: RawObjectReference) -> Result<Self, ObjectError> {
        let uri = raw
            .uri
            .map(|href| {
                resolve_href(base, &href)
                    .map(|resolved| resolved.target)
                    .map_err(|source| ObjectError::InvalidLink { href, source })
            })
            .transpose()?;
        Ok(Self {
            uri,
            object_type: raw.object_type,
            name: raw.name,
            package_name: raw.package_name,
            description: raw.description,
        })
    }
}

fn related_object(
    base: &AdtUri,
    raw: Option<RawObjectReference>,
) -> Result<Option<ClassObjectReference>, ObjectError> {
    raw.filter(|related| !related.is_empty())
        .map(|related| ClassObjectReference::from_raw(base, related))
        .transpose()
}

fn parse_object_version(version: &str) -> Result<ObjectVersion, ObjectError> {
    ObjectVersion::parse(version).ok_or_else(|| ObjectError::UnsupportedObjectVersion {
        version: version.to_owned(),
    })
}

fn package_reference(raw: RawPackageReference) -> Result<ObjectRef<Package>, ObjectError> {
    if raw.object_type != Package::WORKBENCH_TYPE {
        return Err(ObjectError::UnexpectedObjectType {
            expected: Package::WORKBENCH_TYPE,
            actual: raw.object_type,
        });
    }
    let uri = AdtUri::parse(&raw.uri).map_err(|source| ObjectError::InvalidLink {
        href: raw.uri.clone(),
        source,
    })?;
    Ok(ObjectRef::from_parts(raw.name, uri))
}

#[derive(Deserialize)]
#[serde(rename = "class:abapClass")]
struct RawClassProperties {
    #[serde(rename = "@class:final")]
    is_final: bool,
    #[serde(rename = "@class:abstract")]
    is_abstract: bool,
    #[serde(rename = "@class:visibility")]
    visibility: String,
    #[serde(rename = "@class:category")]
    category: String,
    #[serde(rename = "@class:state")]
    state: Option<String>,
    #[serde(rename = "@class:sharedMemoryEnabled")]
    shared_memory_enabled: bool,
    #[serde(rename = "@class:constructorGenerated", default)]
    constructor_generated: bool,
    #[serde(rename = "@class:hasTests", default)]
    has_tests: bool,
    #[serde(rename = "@abapoo:modeled")]
    modeled: bool,
    #[serde(rename = "@abapsource:sourceObjectStatus")]
    source_object_status: Option<String>,
    #[serde(rename = "@abapsource:fixPointArithmetic")]
    fix_point_arithmetic: bool,
    #[serde(rename = "@abapsource:activeUnicodeCheck")]
    unicode_check_active: bool,
    #[serde(rename = "@adtcore:responsible")]
    responsible: String,
    #[serde(rename = "@adtcore:masterLanguage")]
    master_language: String,
    #[serde(rename = "@adtcore:masterSystem")]
    master_system: String,
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    abap_language_version: Option<String>,
    #[serde(rename = "@adtcore:name")]
    name: String,
    #[serde(rename = "@adtcore:type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    last_changed: String,
    #[serde(rename = "@adtcore:version")]
    version: String,
    #[serde(rename = "@adtcore:createdAt")]
    created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    changed_by: String,
    #[serde(rename = "@adtcore:createdBy")]
    created_by: String,
    #[serde(rename = "@adtcore:description")]
    description: String,
    #[serde(rename = "@adtcore:descriptionTextLimit")]
    description_text_limit: u32,
    #[serde(rename = "@adtcore:language")]
    language: String,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
    #[serde(rename = "adtcore:packageRef")]
    package: RawPackageReference,
    #[serde(rename = "abapsource:syntaxConfiguration")]
    syntax_configuration: Option<RawSyntaxConfiguration>,
    #[serde(rename = "abapoo:interfaceRef", default)]
    interfaces: Vec<RawObjectReference>,
    #[serde(rename = "class:include", default)]
    sources: Vec<RawClassSource>,
    #[serde(rename = "class:superClassRef")]
    super_class: Option<RawObjectReference>,
    #[serde(rename = "class:messageClassRef")]
    message_class: Option<RawObjectReference>,
    #[serde(rename = "class:rootEntityRef")]
    root_entity: Option<RawObjectReference>,
}

#[derive(Deserialize)]
struct RawClassSource {
    #[serde(rename = "@class:includeType")]
    include_type: String,
    #[serde(rename = "@abapsource:sourceUri")]
    source_uri: String,
    #[serde(rename = "@adtcore:type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    last_changed: String,
    #[serde(rename = "@adtcore:version")]
    version: String,
    #[serde(rename = "@adtcore:createdAt")]
    created_at: String,
    #[serde(rename = "@adtcore:changedBy")]
    changed_by: String,
    #[serde(rename = "@adtcore:createdBy")]
    created_by: String,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}

#[derive(Deserialize)]
struct RawPackageReference {
    #[serde(rename = "@adtcore:name")]
    name: String,
    #[serde(rename = "@adtcore:uri")]
    uri: String,
    #[serde(rename = "@adtcore:type")]
    object_type: GlobalWorkbenchType,
}

#[derive(Deserialize)]
struct RawSyntaxConfiguration {
    #[serde(rename = "abapsource:language")]
    language: Option<RawSyntaxLanguage>,
}

#[derive(Deserialize)]
struct RawSyntaxLanguage {
    #[serde(rename = "abapsource:version")]
    version: String,
    #[serde(rename = "abapsource:description")]
    description: String,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
}

#[derive(Deserialize)]
struct RawObjectReference {
    #[serde(rename = "@adtcore:uri")]
    uri: Option<String>,
    #[serde(rename = "@adtcore:type")]
    object_type: Option<String>,
    #[serde(rename = "@adtcore:name")]
    name: Option<String>,
    #[serde(rename = "@adtcore:packageName")]
    package_name: Option<String>,
    #[serde(rename = "@adtcore:description")]
    description: Option<String>,
}

impl RawObjectReference {
    fn is_empty(&self) -> bool {
        self.uri.is_none()
            && self.object_type.is_none()
            && self.name.is_none()
            && self.package_name.is_none()
            && self.description.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASS_XML: &str = include_str!("../../tests/fixtures/class-cl-adt-uri-mapper-v4.xml");
    const LOCAL_TYPES_XML: &str = include_str!("../../tests/fixtures/class-cx-root-v4.xml");

    fn parse(body: &str) -> Result<ClassPropertiesV4, ResponseError> {
        let properties = ClassProperties::parse(
            &ObjectRef::<Class>::for_test(
                "CL_ADT_URI_MAPPER",
                AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            ),
            ClassPropertiesVersion::V4,
            body.as_bytes(),
            Some(EntityTag::from_static("class-etag")),
        )?;
        Ok(match properties {
            ClassProperties::V2(properties)
            | ClassProperties::V3(properties)
            | ClassProperties::V4(properties) => *properties,
        })
    }

    #[test]
    fn parses_the_live_v4_class_manifest() {
        let class = parse(CLASS_XML).unwrap();

        assert_eq!(class.name, "CL_ADT_URI_MAPPER");
        assert_eq!(class.object_type, Class::WORKBENCH_TYPE);
        assert_eq!(class.version, ObjectVersion::Active);
        assert_eq!(class.package.name(), "SADT_TOOLS_CORE");
        assert_eq!(class.abap_language_version.as_deref(), Some("X"));
        assert_eq!(class.source_components.len(), 4);
        assert_eq!(class.relations().len(), 7);
        assert_eq!(class.etag.as_deref(), Some("class-etag"));
        assert_eq!(
            class.main_source().source.uri.as_str(),
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/source/main"
        );
        assert_eq!(
            class
                .source(ClassSourceComponent::Definitions)
                .unwrap()
                .source
                .etag
                .as_deref(),
            Some("201701161841300011")
        );
        assert_eq!(
            class
                .main_source()
                .versions()
                .unwrap()
                .unwrap()
                .uri
                .as_str(),
            "/sap/bc/adt/oo/classes/cl_adt_uri_mapper/includes/main/versions"
        );
    }

    #[test]
    fn parses_the_live_local_types_manifest() {
        let properties = ClassProperties::parse(
            &ObjectRef::<Class>::for_test(
                "CX_ROOT",
                AdtUri::parse("/sap/bc/adt/oo/classes/cx_root").unwrap(),
            ),
            ClassPropertiesVersion::V4,
            LOCAL_TYPES_XML.as_bytes(),
            None,
        )
        .unwrap();
        let class = match properties {
            ClassProperties::V4(properties) => *properties,
            _ => panic!("unexpected class-properties version"),
        };

        assert!(class.is_abstract);
        assert!(class.constructor_generated);
        assert_eq!(class.source_components.len(), 1);
        assert_eq!(
            class
                .source(ClassSourceComponent::LocalTypes)
                .unwrap()
                .source
                .uri
                .as_str(),
            "/sap/bc/adt/oo/classes/cx_root/includes/localtypes"
        );
    }

    #[test]
    fn accepts_v2_without_the_v4_language_version() {
        let body = CLASS_XML.replace(" adtcore:abapLanguageVersion=\"X\"", "");
        let properties = ClassProperties::parse(
            &ObjectRef::<Class>::for_test(
                "CL_ADT_URI_MAPPER",
                AdtUri::parse("/sap/bc/adt/oo/classes/cl_adt_uri_mapper").unwrap(),
            ),
            ClassPropertiesVersion::V2,
            body.as_bytes(),
            None,
        )
        .unwrap();
        let class = match properties {
            ClassProperties::V2(properties) => *properties,
            _ => panic!("unexpected class-properties version"),
        };

        assert_eq!(class.abap_language_version, None);
    }

    #[test]
    fn preserves_unknown_source_types() {
        let body = CLASS_XML.replacen(
            "class:includeType=\"definitions\"",
            "class:includeType=\"future-source\"",
            1,
        );
        let class = parse(&body).unwrap();

        assert_eq!(class.source_components[0].component, None);
        assert_eq!(class.source_components[0].include_type, "future-source");
    }

    #[test]
    fn rejects_an_unknown_source_owned_by_another_object() {
        let body = CLASS_XML
            .replacen(
                "class:includeType=\"definitions\"",
                "class:includeType=\"future-source\"",
                1,
            )
            .replace(
                "includes/definitions",
                "/sap/bc/adt/oo/classes/another_class/includes/future-source",
            );

        assert!(matches!(
            parse(&body),
            Err(ResponseError::Object(ObjectError::RelationMismatch {
                relation: "class source owner",
                ..
            }))
        ));
    }

    #[test]
    fn rejects_a_mismatched_source_relation() {
        let body = CLASS_XML.replacen(
            "abapsource:sourceUri=\"includes/definitions\"",
            "abapsource:sourceUri=\"includes/other\"",
            1,
        );

        assert!(matches!(
            parse(&body),
            Err(ResponseError::Object(ObjectError::RelationMismatch {
                relation: "class source",
                ..
            }))
        ));
    }

    #[test]
    fn rejects_a_recognized_component_at_another_source_path() {
        let body = CLASS_XML.replace("includes/definitions", "includes/other");

        assert!(matches!(
            parse(&body),
            Err(ResponseError::Object(ObjectError::RelationMismatch {
                relation: "class source component",
                ..
            }))
        ));
    }

    #[test]
    fn rejects_the_main_source_at_a_secondary_path() {
        let body = CLASS_XML.replace("source/main", "includes/main");

        assert!(matches!(
            parse(&body),
            Err(ResponseError::Object(ObjectError::RelationMismatch {
                relation: "main class source",
                ..
            }))
        ));
    }

    #[test]
    fn rejects_duplicate_recognized_source_components() {
        let start = CLASS_XML
            .find("<class:include class:includeType=\"main\"")
            .unwrap();
        let end =
            start + CLASS_XML[start..].find("</class:include>").unwrap() + "</class:include>".len();
        let duplicate = &CLASS_XML[start..end];
        let body = CLASS_XML.replace(
            "</class:abapClass>",
            &format!("{duplicate}</class:abapClass>"),
        );

        assert!(matches!(
            parse(&body),
            Err(ResponseError::Object(
                ObjectError::DuplicateSourceComponent { component }
            )) if component == "main"
        ));
    }

    #[test]
    fn ignores_empty_optional_object_references() {
        let body = CLASS_XML.replacen(
            "<class:include",
            "<abapoo:interfaceRef/><class:superClassRef/><class:messageClassRef/><class:rootEntityRef/><class:include",
            1,
        );
        let class = parse(&body).unwrap();

        assert!(class.interfaces.is_empty());
        assert!(class.super_class.is_none());
        assert!(class.message_class.is_none());
        assert!(class.root_entity.is_none());
    }

    #[test]
    fn requires_the_main_source() {
        let body = CLASS_XML.replace("class:includeType=\"main\"", "class:includeType=\"other\"");

        assert!(matches!(
            parse(&body),
            Err(ResponseError::Object(ObjectError::MissingRelation {
                relation: "main class source"
            }))
        ));
    }
}
