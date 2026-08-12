use serde::{Deserialize, Serialize};

use crate::{
    AdtUri, DataElement, EntityTag, GlobalWorkbenchType, MediaVersionNegotiation, ObjectError,
    ObjectRef, ObjectType, ObjectVersion, Package, PackageReference, RawObjectProperties,
    ResponseError, WritableProperties,
    resource::{AdvertisedLink, Relations},
};

const BLUE_NAMESPACE: &str = "http://www.sap.com/wbobj/dictionary/dtel";
const CORE_NAMESPACE: &str = "http://www.sap.com/adt/core";
const DATA_ELEMENT_NAMESPACE: &str = "http://www.sap.com/adt/dictionary/dataelements";
const ATOM_NAMESPACE: &str = "http://www.w3.org/2005/Atom";

/// The SAP media-type version used to decode Data Element properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataElementPropertiesVersion {
    /// Data Element properties V2.
    V2,
}

impl MediaVersionNegotiation for DataElementPropertiesVersion {
    const SUPPORTED: &'static [Self] = &[Self::V2];

    fn media_type(self) -> &'static str {
        match self {
            Self::V2 => "application/vnd.sap.adt.dataelements.v2+xml",
        }
    }
}

/// Data Element properties tagged with the media-type version returned by ADT.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DataElementProperties {
    /// A V2 Data Element properties response.
    V2(Box<DataElementPropertiesV2>),
}

impl DataElementProperties {
    /// Returns the response media-type version.
    pub fn media_version(&self) -> DataElementPropertiesVersion {
        match self {
            Self::V2(_) => DataElementPropertiesVersion::V2,
        }
    }

    /// Returns the V2 properties representation.
    pub fn properties(&self) -> &DataElementPropertiesV2 {
        match self {
            Self::V2(properties) => properties,
        }
    }

    /// Returns the mutable V2 properties representation.
    pub fn properties_mut(&mut self) -> &mut DataElementPropertiesV2 {
        match self {
            Self::V2(properties) => properties,
        }
    }

    /// Returns the response entity tag, when present.
    pub fn etag(&self) -> Option<&EntityTag> {
        self.properties().etag.as_ref()
    }
}

impl WritableProperties<DataElement> for DataElementProperties {
    fn media_version(&self) -> DataElementPropertiesVersion {
        self.media_version()
    }

    fn to_xml(&self, resource: &ObjectRef<DataElement>) -> Result<String, ObjectError> {
        match self {
            Self::V2(properties) => properties.to_xml(resource),
        }
    }
}

impl TryFrom<RawObjectProperties<DataElement>> for DataElementProperties {
    type Error = ResponseError;

    fn try_from(raw: RawObjectProperties<DataElement>) -> Result<Self, Self::Error> {
        let properties: RawDataElementProperties =
            serde_xml_rs::from_reader(raw.body.as_slice()).map_err(ObjectError::InvalidResponse)?;
        let properties = DataElementPropertiesV2::from_raw(raw.resource, properties, raw.etag)?;
        Ok(match raw.version {
            DataElementPropertiesVersion::V2 => Self::V2(Box::new(properties)),
        })
    }
}

/// The V2 Data Element properties representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataElementPropertiesV2 {
    /// The Data Element resource that was fetched.
    pub reference: ObjectRef<DataElement>,

    /// The Data Element name supplied by SAP.
    pub name: String,

    /// The repository object type, normally `DTEL/DE`.
    pub object_type: GlobalWorkbenchType,

    /// The user responsible for the Data Element, when advertised.
    pub responsible: Option<String>,

    /// The Data Element's master language, when advertised.
    pub master_language: Option<String>,

    /// The object's master system, when advertised.
    pub master_system: Option<String>,

    /// The configured ABAP language version, when advertised.
    pub abap_language_version: Option<String>,

    /// The timestamp at which the object was last changed.
    pub last_changed: Option<String>,

    /// The active, inactive, working-area, or new object version.
    pub version: Option<ObjectVersion>,

    /// The timestamp at which the object was created.
    pub created_at: Option<String>,

    /// The user who last changed the object.
    pub changed_by: Option<String>,

    /// The user who created the object.
    pub created_by: Option<String>,

    /// The Data Element description, when advertised.
    pub description: Option<String>,

    /// The language in which language-dependent values are represented.
    pub language: Option<String>,

    /// The package containing the Data Element, when advertised.
    pub package: Option<PackageReference>,

    /// The Data Element's type definition and field behavior.
    pub definition: DataElementDefinition,

    /// The response entity tag, when present.
    pub etag: Option<EntityTag>,

    relations: Relations,
}

impl DataElementPropertiesV2 {
    /// Returns the links advertised with the properties response.
    pub fn relations(&self) -> &Relations {
        &self.relations
    }

    fn from_raw(
        reference: ObjectRef<DataElement>,
        raw: RawDataElementProperties,
        etag: Option<EntityTag>,
    ) -> Result<Self, ObjectError> {
        if raw.object_type != DataElement::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: DataElement::WORKBENCH_TYPE,
                actual: raw.object_type,
            });
        }
        if !raw.name.eq_ignore_ascii_case(reference.name()) {
            return Err(ObjectError::UnexpectedObjectName {
                expected: reference.name().to_owned(),
                actual: raw.name,
            });
        }
        let version = raw
            .version
            .as_deref()
            .map(|version| {
                ObjectVersion::parse(version).ok_or_else(|| ObjectError::UnsupportedObjectVersion {
                    version: version.to_owned(),
                })
            })
            .transpose()?;
        let package = raw
            .package
            .map(RawPackageReference::into_reference)
            .transpose()?;
        let relations = Relations::new(reference.erase(), raw.links);

        Ok(Self {
            reference,
            name: raw.name,
            object_type: raw.object_type,
            responsible: raw.responsible,
            master_language: raw.master_language,
            master_system: raw.master_system,
            abap_language_version: raw.abap_language_version,
            last_changed: raw.last_changed,
            version,
            created_at: raw.created_at,
            changed_by: raw.changed_by,
            created_by: raw.created_by,
            description: raw.description,
            language: raw.language,
            package,
            definition: raw.definition.try_into()?,
            etag,
            relations,
        })
    }

    fn validate(&self, resource: &ObjectRef<DataElement>) -> Result<(), ObjectError> {
        if self.reference != *resource {
            return Err(ObjectError::ObjectPropertiesMismatch {
                expected: resource.to_string(),
                actual: self.reference.to_string(),
            });
        }
        if !self.reference.name().eq_ignore_ascii_case(resource.name()) {
            return Err(ObjectError::UnexpectedObjectName {
                expected: resource.name().to_owned(),
                actual: self.reference.name().to_owned(),
            });
        }
        if !self.name.eq_ignore_ascii_case(resource.name()) {
            return Err(ObjectError::UnexpectedObjectName {
                expected: resource.name().to_owned(),
                actual: self.name.clone(),
            });
        }
        if self.object_type != DataElement::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: DataElement::WORKBENCH_TYPE,
                actual: self.object_type.clone(),
            });
        }
        Ok(())
    }

    fn to_xml(&self, resource: &ObjectRef<DataElement>) -> Result<String, ObjectError> {
        self.validate(resource)?;
        serde_xml_rs::SerdeXml::new()
            .namespace("blue", BLUE_NAMESPACE)
            .namespace("adtcore", CORE_NAMESPACE)
            .namespace("dtel", DATA_ELEMENT_NAMESPACE)
            .namespace("atom", ATOM_NAMESPACE)
            .to_string(&WritableDataElement::new(self))
            .map_err(ObjectError::InvalidRequest)
    }
}

/// The Data Element type-definition kind used by ADT.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataElementTypeKind {
    /// The Data Element references a Dictionary domain.
    Domain,

    /// The Data Element directly uses a predefined ABAP type.
    PredefinedAbapType,

    /// The Data Element is a reference to a predefined ABAP type.
    ReferenceToPredefinedAbapType,

    /// The Data Element is a reference to another Dictionary type.
    ReferenceToDictionaryType,

    /// The Data Element is a reference to a class or interface type.
    ReferenceToClassOrInterfaceType,
}

impl DataElementTypeKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "domain" => Some(Self::Domain),
            "predefinedAbapType" => Some(Self::PredefinedAbapType),
            "refToPredefinedAbapType" => Some(Self::ReferenceToPredefinedAbapType),
            "refToDictionaryType" => Some(Self::ReferenceToDictionaryType),
            "refToClifType" => Some(Self::ReferenceToClassOrInterfaceType),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::PredefinedAbapType => "predefinedAbapType",
            Self::ReferenceToPredefinedAbapType => "refToPredefinedAbapType",
            Self::ReferenceToDictionaryType => "refToDictionaryType",
            Self::ReferenceToClassOrInterfaceType => "refToClifType",
        }
    }
}

/// One field label and the lengths advertised for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataElementFieldLabel {
    /// The language-dependent field label.
    pub text: Option<String>,

    /// The current output length of the label.
    pub length: Option<u32>,

    /// The maximum supported output length of the label.
    pub max_length: Option<u32>,
}

/// Documentation status assigned to a Data Element.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataElementDocumentationStatus {
    /// Documentation is required.
    Required,

    /// The Data Element is not used in screens.
    NotUsedInScreens,

    /// The short text sufficiently explains the Data Element.
    ExplainedByShortText,

    /// Documentation has been postponed.
    Postponed,
}

impl DataElementDocumentationStatus {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "required" => Some(Self::Required),
            "notUsedInScreens" => Some(Self::NotUsedInScreens),
            "explainedByShortText" => Some(Self::ExplainedByShortText),
            "postponed" => Some(Self::Postponed),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::NotUsedInScreens => "notUsedInScreens",
            Self::ExplainedByShortText => "explainedByShortText",
            Self::Postponed => "postponed",
        }
    }
}

/// The nested Data Element definition preserved between reads and updates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataElementDefinition {
    /// How the Data Element obtains its type.
    pub type_kind: DataElementTypeKind,

    /// The referenced domain or type name, when used by this type kind.
    pub type_name: Option<String>,

    /// The resolved Dictionary datatype, when advertised.
    pub data_type: Option<String>,

    /// The resolved datatype length, when advertised.
    pub data_type_length: Option<u32>,

    /// Whether the datatype length can be edited for this type kind.
    pub data_type_length_enabled: Option<bool>,

    /// The resolved number of decimal places, when advertised.
    pub data_type_decimals: Option<u32>,

    /// Whether decimal places can be edited for this type kind.
    pub data_type_decimals_enabled: Option<bool>,

    /// The short field label.
    pub short_field_label: DataElementFieldLabel,

    /// The medium field label.
    pub medium_field_label: DataElementFieldLabel,

    /// The long field label.
    pub long_field_label: DataElementFieldLabel,

    /// The heading field label.
    pub heading_field_label: DataElementFieldLabel,

    /// The assigned search help, including an explicitly empty value.
    pub search_help: Option<String>,

    /// The assigned search-help parameter, including an explicitly empty value.
    pub search_help_parameter: Option<String>,

    /// The assigned SET/GET parameter, including an explicitly empty value.
    pub set_get_parameter: Option<String>,

    /// The default component name, including an explicitly empty value.
    pub default_component_name: Option<String>,

    /// Whether input history is disabled, when advertised.
    pub deactivate_input_history: Option<bool>,

    /// Whether changes are recorded in change documents, when advertised.
    pub change_document: Option<bool>,

    /// Whether bidirectional text is forced left-to-right, when advertised.
    pub left_to_right_direction: Option<bool>,

    /// Whether bidirectional filtering is disabled, when advertised.
    pub deactivate_bidi_filtering: Option<bool>,

    /// The documentation status returned with the Data Element definition.
    pub documentation_status: Option<DataElementDocumentationStatus>,
}

#[derive(Deserialize)]
#[serde(rename = "blue:wbobj")]
#[serde(deny_unknown_fields)]
struct RawDataElementProperties {
    #[serde(rename = "@adtcore:responsible")]
    responsible: Option<String>,
    #[serde(rename = "@adtcore:masterLanguage")]
    master_language: Option<String>,
    #[serde(rename = "@adtcore:masterSystem")]
    master_system: Option<String>,
    #[serde(rename = "@adtcore:abapLanguageVersion")]
    abap_language_version: Option<String>,
    #[serde(rename = "@adtcore:name")]
    name: String,
    #[serde(rename = "@adtcore:type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:changedAt")]
    last_changed: Option<String>,
    #[serde(rename = "@adtcore:version")]
    version: Option<String>,
    #[serde(rename = "@adtcore:createdAt")]
    created_at: Option<String>,
    #[serde(rename = "@adtcore:changedBy")]
    changed_by: Option<String>,
    #[serde(rename = "@adtcore:createdBy")]
    created_by: Option<String>,
    #[serde(rename = "@adtcore:description")]
    description: Option<String>,
    #[serde(rename = "@adtcore:language")]
    language: Option<String>,
    #[serde(rename = "atom:link", default)]
    links: Vec<AdvertisedLink>,
    #[serde(rename = "adtcore:packageRef")]
    package: Option<RawPackageReference>,
    #[serde(rename = "dtel:dataElement")]
    definition: RawDataElementDefinition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackageReference {
    #[serde(rename = "@adtcore:name")]
    name: String,
    #[serde(rename = "@adtcore:uri")]
    uri: String,
    #[serde(rename = "@adtcore:type")]
    object_type: GlobalWorkbenchType,
    #[serde(rename = "@adtcore:description")]
    description: Option<String>,
}

impl RawPackageReference {
    fn into_reference(self) -> Result<PackageReference, ObjectError> {
        if self.object_type != Package::WORKBENCH_TYPE {
            return Err(ObjectError::UnexpectedObjectType {
                expected: Package::WORKBENCH_TYPE,
                actual: self.object_type,
            });
        }
        let uri = AdtUri::parse(&self.uri).map_err(|source| ObjectError::InvalidLink {
            href: self.uri.clone(),
            source,
        })?;
        Ok(PackageReference {
            reference: ObjectRef::from_parts(self.name, uri),
            description: self.description,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDataElementDefinition {
    #[serde(rename = "dtel:typeKind")]
    type_kind: String,
    #[serde(rename = "dtel:typeName")]
    type_name: Option<String>,
    #[serde(rename = "dtel:dataType")]
    data_type: Option<String>,
    #[serde(rename = "dtel:dataTypeLength")]
    data_type_length: Option<u32>,
    #[serde(rename = "dtel:dataTypeLengthEnabled")]
    data_type_length_enabled: Option<bool>,
    #[serde(rename = "dtel:dataTypeDecimals")]
    data_type_decimals: Option<u32>,
    #[serde(rename = "dtel:dataTypeDecimalsEnabled")]
    data_type_decimals_enabled: Option<bool>,
    #[serde(rename = "dtel:shortFieldLabel")]
    short_field_label: Option<String>,
    #[serde(rename = "dtel:shortFieldLength")]
    short_field_length: Option<u32>,
    #[serde(rename = "dtel:shortFieldMaxLength")]
    short_field_max_length: Option<u32>,
    #[serde(rename = "dtel:mediumFieldLabel")]
    medium_field_label: Option<String>,
    #[serde(rename = "dtel:mediumFieldLength")]
    medium_field_length: Option<u32>,
    #[serde(rename = "dtel:mediumFieldMaxLength")]
    medium_field_max_length: Option<u32>,
    #[serde(rename = "dtel:longFieldLabel")]
    long_field_label: Option<String>,
    #[serde(rename = "dtel:longFieldLength")]
    long_field_length: Option<u32>,
    #[serde(rename = "dtel:longFieldMaxLength")]
    long_field_max_length: Option<u32>,
    #[serde(rename = "dtel:headingFieldLabel")]
    heading_field_label: Option<String>,
    #[serde(rename = "dtel:headingFieldLength")]
    heading_field_length: Option<u32>,
    #[serde(rename = "dtel:headingFieldMaxLength")]
    heading_field_max_length: Option<u32>,
    #[serde(rename = "dtel:searchHelp")]
    search_help: Option<String>,
    #[serde(rename = "dtel:searchHelpParameter")]
    search_help_parameter: Option<String>,
    #[serde(rename = "dtel:setGetParameter")]
    set_get_parameter: Option<String>,
    #[serde(rename = "dtel:defaultComponentName")]
    default_component_name: Option<String>,
    #[serde(rename = "dtel:deactivateInputHistory")]
    deactivate_input_history: Option<bool>,
    #[serde(rename = "dtel:changeDocument")]
    change_document: Option<bool>,
    #[serde(rename = "dtel:leftToRightDirection")]
    left_to_right_direction: Option<bool>,
    #[serde(rename = "dtel:deactivateBIDIFiltering")]
    deactivate_bidi_filtering: Option<bool>,
    #[serde(rename = "dtel:documentationStatus")]
    documentation_status: Option<String>,
}

impl TryFrom<RawDataElementDefinition> for DataElementDefinition {
    type Error = ObjectError;

    fn try_from(raw: RawDataElementDefinition) -> Result<Self, Self::Error> {
        let type_kind = DataElementTypeKind::parse(&raw.type_kind).ok_or_else(|| {
            ObjectError::UnsupportedDataElementTypeKind {
                kind: raw.type_kind.clone(),
            }
        })?;
        let documentation_status = raw
            .documentation_status
            .as_deref()
            .map(|status| {
                DataElementDocumentationStatus::parse(status).ok_or_else(|| {
                    ObjectError::UnsupportedDataElementDocumentationStatus {
                        status: status.to_owned(),
                    }
                })
            })
            .transpose()?;
        Ok(Self {
            type_kind,
            type_name: raw.type_name,
            data_type: raw.data_type,
            data_type_length: raw.data_type_length,
            data_type_length_enabled: raw.data_type_length_enabled,
            data_type_decimals: raw.data_type_decimals,
            data_type_decimals_enabled: raw.data_type_decimals_enabled,
            short_field_label: DataElementFieldLabel {
                text: raw.short_field_label,
                length: raw.short_field_length,
                max_length: raw.short_field_max_length,
            },
            medium_field_label: DataElementFieldLabel {
                text: raw.medium_field_label,
                length: raw.medium_field_length,
                max_length: raw.medium_field_max_length,
            },
            long_field_label: DataElementFieldLabel {
                text: raw.long_field_label,
                length: raw.long_field_length,
                max_length: raw.long_field_max_length,
            },
            heading_field_label: DataElementFieldLabel {
                text: raw.heading_field_label,
                length: raw.heading_field_length,
                max_length: raw.heading_field_max_length,
            },
            search_help: raw.search_help,
            search_help_parameter: raw.search_help_parameter,
            set_get_parameter: raw.set_get_parameter,
            default_component_name: raw.default_component_name,
            deactivate_input_history: raw.deactivate_input_history,
            change_document: raw.change_document,
            left_to_right_direction: raw.left_to_right_direction,
            deactivate_bidi_filtering: raw.deactivate_bidi_filtering,
            documentation_status,
        })
    }
}

#[derive(Serialize)]
#[serde(rename = "blue:wbobj")]
struct WritableDataElement<'a> {
    #[serde(
        rename = "@adtcore:responsible",
        skip_serializing_if = "Option::is_none"
    )]
    responsible: Option<&'a str>,
    #[serde(
        rename = "@adtcore:masterLanguage",
        skip_serializing_if = "Option::is_none"
    )]
    master_language: Option<&'a str>,
    #[serde(
        rename = "@adtcore:masterSystem",
        skip_serializing_if = "Option::is_none"
    )]
    master_system: Option<&'a str>,
    #[serde(
        rename = "@adtcore:abapLanguageVersion",
        skip_serializing_if = "Option::is_none"
    )]
    abap_language_version: Option<&'a str>,
    #[serde(rename = "@adtcore:name")]
    name: &'a str,
    #[serde(rename = "@adtcore:type")]
    object_type: &'a str,
    #[serde(rename = "@adtcore:changedAt", skip_serializing_if = "Option::is_none")]
    last_changed: Option<&'a str>,
    #[serde(rename = "@adtcore:version", skip_serializing_if = "Option::is_none")]
    version: Option<&'static str>,
    #[serde(rename = "@adtcore:createdAt", skip_serializing_if = "Option::is_none")]
    created_at: Option<&'a str>,
    #[serde(rename = "@adtcore:changedBy", skip_serializing_if = "Option::is_none")]
    changed_by: Option<&'a str>,
    #[serde(rename = "@adtcore:createdBy", skip_serializing_if = "Option::is_none")]
    created_by: Option<&'a str>,
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<&'a str>,
    #[serde(rename = "@adtcore:language", skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
    #[serde(rename = "atom:link")]
    links: &'a [AdvertisedLink],
    #[serde(rename = "adtcore:packageRef", skip_serializing_if = "Option::is_none")]
    package: Option<WritablePackageReference<'a>>,
    #[serde(rename = "dtel:dataElement")]
    definition: WritableDataElementDefinition<'a>,
}

impl<'a> WritableDataElement<'a> {
    fn new(properties: &'a DataElementPropertiesV2) -> Self {
        Self {
            responsible: properties.responsible.as_deref(),
            master_language: properties.master_language.as_deref(),
            master_system: properties.master_system.as_deref(),
            abap_language_version: properties.abap_language_version.as_deref(),
            name: &properties.name,
            object_type: properties.object_type.as_str(),
            last_changed: properties.last_changed.as_deref(),
            version: properties.version.map(ObjectVersion::as_str),
            created_at: properties.created_at.as_deref(),
            changed_by: properties.changed_by.as_deref(),
            created_by: properties.created_by.as_deref(),
            description: properties.description.as_deref(),
            language: properties.language.as_deref(),
            links: properties.relations.advertised(),
            package: properties
                .package
                .as_ref()
                .map(WritablePackageReference::new),
            definition: WritableDataElementDefinition::new(&properties.definition),
        }
    }
}

#[derive(Serialize)]
struct WritablePackageReference<'a> {
    #[serde(rename = "@adtcore:name")]
    name: &'a str,
    #[serde(rename = "@adtcore:uri")]
    uri: &'a str,
    #[serde(rename = "@adtcore:type")]
    object_type: String,
    #[serde(
        rename = "@adtcore:description",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<&'a str>,
}

impl<'a> WritablePackageReference<'a> {
    fn new(package: &'a PackageReference) -> Self {
        Self {
            name: package.reference.name(),
            uri: package.reference.uri().as_str(),
            object_type: Package::WORKBENCH_TYPE.to_string(),
            description: package.description.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct WritableDataElementDefinition<'a> {
    #[serde(rename = "dtel:typeKind")]
    type_kind: &'static str,
    #[serde(rename = "dtel:typeName", skip_serializing_if = "Option::is_none")]
    type_name: Option<&'a str>,
    #[serde(rename = "dtel:dataType", skip_serializing_if = "Option::is_none")]
    data_type: Option<&'a str>,
    #[serde(
        rename = "dtel:dataTypeLength",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_length: Option<u32>,
    #[serde(
        rename = "dtel:dataTypeLengthEnabled",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_length_enabled: Option<bool>,
    #[serde(
        rename = "dtel:dataTypeDecimals",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_decimals: Option<u32>,
    #[serde(
        rename = "dtel:dataTypeDecimalsEnabled",
        skip_serializing_if = "Option::is_none"
    )]
    data_type_decimals_enabled: Option<bool>,
    #[serde(
        rename = "dtel:shortFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    short_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:shortFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    short_field_length: Option<u32>,
    #[serde(
        rename = "dtel:shortFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    short_field_max_length: Option<u32>,
    #[serde(
        rename = "dtel:mediumFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    medium_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:mediumFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    medium_field_length: Option<u32>,
    #[serde(
        rename = "dtel:mediumFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    medium_field_max_length: Option<u32>,
    #[serde(
        rename = "dtel:longFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    long_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:longFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    long_field_length: Option<u32>,
    #[serde(
        rename = "dtel:longFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    long_field_max_length: Option<u32>,
    #[serde(
        rename = "dtel:headingFieldLabel",
        skip_serializing_if = "Option::is_none"
    )]
    heading_field_label: Option<&'a str>,
    #[serde(
        rename = "dtel:headingFieldLength",
        skip_serializing_if = "Option::is_none"
    )]
    heading_field_length: Option<u32>,
    #[serde(
        rename = "dtel:headingFieldMaxLength",
        skip_serializing_if = "Option::is_none"
    )]
    heading_field_max_length: Option<u32>,
    #[serde(rename = "dtel:searchHelp", skip_serializing_if = "Option::is_none")]
    search_help: Option<&'a str>,
    #[serde(
        rename = "dtel:searchHelpParameter",
        skip_serializing_if = "Option::is_none"
    )]
    search_help_parameter: Option<&'a str>,
    #[serde(
        rename = "dtel:setGetParameter",
        skip_serializing_if = "Option::is_none"
    )]
    set_get_parameter: Option<&'a str>,
    #[serde(
        rename = "dtel:defaultComponentName",
        skip_serializing_if = "Option::is_none"
    )]
    default_component_name: Option<&'a str>,
    #[serde(
        rename = "dtel:deactivateInputHistory",
        skip_serializing_if = "Option::is_none"
    )]
    deactivate_input_history: Option<bool>,
    #[serde(
        rename = "dtel:changeDocument",
        skip_serializing_if = "Option::is_none"
    )]
    change_document: Option<bool>,
    #[serde(
        rename = "dtel:leftToRightDirection",
        skip_serializing_if = "Option::is_none"
    )]
    left_to_right_direction: Option<bool>,
    #[serde(
        rename = "dtel:deactivateBIDIFiltering",
        skip_serializing_if = "Option::is_none"
    )]
    deactivate_bidi_filtering: Option<bool>,
    #[serde(
        rename = "dtel:documentationStatus",
        skip_serializing_if = "Option::is_none"
    )]
    documentation_status: Option<&'static str>,
}

impl<'a> WritableDataElementDefinition<'a> {
    fn new(definition: &'a DataElementDefinition) -> Self {
        Self {
            type_kind: definition.type_kind.as_str(),
            type_name: definition.type_name.as_deref(),
            data_type: definition.data_type.as_deref(),
            data_type_length: definition.data_type_length,
            data_type_length_enabled: definition.data_type_length_enabled,
            data_type_decimals: definition.data_type_decimals,
            data_type_decimals_enabled: definition.data_type_decimals_enabled,
            short_field_label: definition.short_field_label.text.as_deref(),
            short_field_length: definition.short_field_label.length,
            short_field_max_length: definition.short_field_label.max_length,
            medium_field_label: definition.medium_field_label.text.as_deref(),
            medium_field_length: definition.medium_field_label.length,
            medium_field_max_length: definition.medium_field_label.max_length,
            long_field_label: definition.long_field_label.text.as_deref(),
            long_field_length: definition.long_field_label.length,
            long_field_max_length: definition.long_field_label.max_length,
            heading_field_label: definition.heading_field_label.text.as_deref(),
            heading_field_length: definition.heading_field_label.length,
            heading_field_max_length: definition.heading_field_label.max_length,
            search_help: definition.search_help.as_deref(),
            search_help_parameter: definition.search_help_parameter.as_deref(),
            set_get_parameter: definition.set_get_parameter.as_deref(),
            default_component_name: definition.default_component_name.as_deref(),
            deactivate_input_history: definition.deactivate_input_history,
            change_document: definition.change_document,
            left_to_right_direction: definition.left_to_right_direction,
            deactivate_bidi_filtering: definition.deactivate_bidi_filtering,
            documentation_status: definition
                .documentation_status
                .map(DataElementDocumentationStatus::as_str),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA_ELEMENT_XML: &[u8] =
        include_bytes!("../../tests/fixtures/data-element-ztfrwtfrt-v2.xml");

    fn reference() -> ObjectRef<DataElement> {
        ObjectRef::<DataElement>::for_test(
            "ZTFRWTFRT",
            AdtUri::parse("/sap/bc/adt/ddic/dataelements/ztfrwtfrt").unwrap(),
        )
    }

    fn parse(
        resource: &ObjectRef<DataElement>,
        version: DataElementPropertiesVersion,
        body: &[u8],
        etag: Option<EntityTag>,
    ) -> Result<DataElementProperties, ResponseError> {
        DataElementProperties::try_from(RawObjectProperties {
            resource: resource.clone(),
            version,
            body: body.to_vec(),
            etag,
        })
    }

    fn properties() -> DataElementPropertiesV2 {
        let properties = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            DATA_ELEMENT_XML,
            Some(EntityTag::from_static("data-element-etag")),
        )
        .unwrap();
        let DataElementProperties::V2(properties) = properties;
        *properties
    }

    fn sparse_properties_xml(type_kind: &str, extra: &str) -> String {
        let mut xml = String::from_utf8(DATA_ELEMENT_XML.to_vec()).unwrap();
        let start = xml.find("<dtel:dataElement").unwrap();
        let end_tag = "</dtel:dataElement>";
        let end = xml[start..].find(end_tag).unwrap() + start + end_tag.len();
        xml.replace_range(
            start..end,
            &format!(
                "<dtel:dataElement xmlns:dtel=\"{DATA_ELEMENT_NAMESPACE}\"><dtel:typeKind>{type_kind}</dtel:typeKind>{extra}</dtel:dataElement>"
            ),
        );
        xml
    }

    fn without_root_attributes(names: &[&str]) -> String {
        let mut xml = String::from_utf8(DATA_ELEMENT_XML.to_vec()).unwrap();
        for name in names {
            let prefix = format!(" adtcore:{name}=\"");
            let start = xml.find(&prefix).unwrap();
            let value_start = start + prefix.len();
            let value_end = xml[value_start..].find('"').unwrap() + value_start;
            xml.replace_range(start..=value_end, "");
        }
        xml
    }

    #[test]
    fn parses_live_data_element_properties_as_one_representation() {
        let properties = properties();

        assert_eq!(properties.reference, reference());
        assert_eq!(properties.version, Some(ObjectVersion::New));
        assert_eq!(properties.master_system.as_deref(), Some("A4H"));
        assert_eq!(
            properties.package.as_ref().unwrap().reference.name(),
            "$TMP"
        );
        assert_eq!(properties.etag.as_deref(), Some("data-element-etag"));
        assert_eq!(properties.relations().len(), 4);
        assert_eq!(properties.description.as_deref(), Some("tfarFAR"));
        assert_eq!(properties.definition.type_kind, DataElementTypeKind::Domain);
        assert_eq!(properties.definition.type_name.as_deref(), Some("CHAR0008"));
        assert_eq!(properties.definition.data_type_length, Some(8));
        assert_eq!(
            properties.definition.short_field_label.text.as_deref(),
            Some("123")
        );
        assert_eq!(properties.definition.search_help.as_deref(), Some(""));
    }

    #[test]
    fn update_xml_reuses_the_complete_properties_representation() {
        let properties = properties();
        let xml = properties.to_xml(&reference()).unwrap();

        assert!(xml.contains("<blue:wbobj"));
        assert!(xml.contains("adtcore:name=\"ZTFRWTFRT\""));
        assert!(xml.contains("adtcore:type=\"DTEL/DE\""));
        assert!(xml.contains("<dtel:dataTypeLength>8</dtel:dataTypeLength>"));
        assert!(xml.contains("<dtel:searchHelp"));
        assert!(xml.contains("<dtel:defaultComponentName"));
        assert!(xml.contains("adtcore:changedAt="));
        assert!(xml.contains("<adtcore:packageRef"));
        assert!(xml.contains("<atom:link"));
        assert!(!xml.contains("data-element-etag"));

        let mut expected = properties.clone();
        expected.etag = None;
        let decoded = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap();
        assert_eq!(decoded, DataElementProperties::V2(Box::new(expected)));

        let raw: RawDataElementProperties = serde_xml_rs::from_str(&xml).unwrap();
        assert_eq!(raw.object_type, DataElement::WORKBENCH_TYPE);
        assert_eq!(raw.links.len(), 4);
        assert_eq!(
            raw.package.unwrap().description.as_deref(),
            Some("Temporary Objects (never transported!)")
        );
        assert_eq!(raw.definition.data_type_length, Some(8));
        assert_eq!(raw.definition.search_help.as_deref(), Some(""));
    }

    #[test]
    fn preserves_variant_specific_omissions_for_every_type_kind() {
        for (wire_value, expected) in [
            ("domain", DataElementTypeKind::Domain),
            (
                "predefinedAbapType",
                DataElementTypeKind::PredefinedAbapType,
            ),
            (
                "refToPredefinedAbapType",
                DataElementTypeKind::ReferenceToPredefinedAbapType,
            ),
            (
                "refToDictionaryType",
                DataElementTypeKind::ReferenceToDictionaryType,
            ),
            (
                "refToClifType",
                DataElementTypeKind::ReferenceToClassOrInterfaceType,
            ),
        ] {
            let properties = parse(
                &reference(),
                DataElementPropertiesVersion::V2,
                sparse_properties_xml(wire_value, "").as_bytes(),
                None,
            )
            .unwrap();
            let definition = &properties.properties().definition;

            assert_eq!(definition.type_kind, expected);
            assert!(definition.type_name.is_none());
            assert!(definition.data_type.is_none());
            assert!(definition.short_field_label.text.is_none());

            let xml = properties.to_xml(&reference()).unwrap();
            assert!(xml.contains(&format!("<dtel:typeKind>{wire_value}</dtel:typeKind>")));
            assert!(!xml.contains("dtel:dataType"));
            assert!(!xml.contains("dtel:shortFieldLabel"));
        }
    }

    #[test]
    fn preserves_optional_root_attribute_omissions() {
        let xml =
            without_root_attributes(&["responsible", "masterLanguage", "description", "language"]);
        let properties = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap();
        let properties_v2 = properties.properties();

        assert!(properties_v2.responsible.is_none());
        assert!(properties_v2.master_language.is_none());
        assert!(properties_v2.description.is_none());
        assert!(properties_v2.language.is_none());

        let update = properties.to_xml(&reference()).unwrap();
        let raw: RawDataElementProperties = serde_xml_rs::from_str(&update).unwrap();
        assert!(raw.responsible.is_none());
        assert!(raw.master_language.is_none());
        assert!(raw.description.is_none());
        assert!(raw.language.is_none());
    }

    #[test]
    fn documentation_status_round_trips_with_the_properties() {
        let xml = sparse_properties_xml(
            "domain",
            "<dtel:documentationStatus>required</dtel:documentationStatus>",
        );
        let properties = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap();

        assert_eq!(
            properties.properties().definition.documentation_status,
            Some(DataElementDocumentationStatus::Required)
        );
        assert!(
            properties
                .to_xml(&reference())
                .unwrap()
                .contains("<dtel:documentationStatus>required</dtel:documentationStatus>")
        );
    }

    #[test]
    fn rejects_unmodeled_definition_fields_before_they_can_be_dropped() {
        let xml = sparse_properties_xml("domain", "<dtel:futureField>value</dtel:futureField>");
        let error = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::InvalidResponse(_))
        ));
    }

    #[test]
    fn rejects_properties_fetched_for_another_resource() {
        let properties = properties();
        let other = ObjectRef::<DataElement>::for_test(
            "ZOTHER",
            AdtUri::parse("/sap/bc/adt/ddic/dataelements/zother").unwrap(),
        );

        assert!(matches!(
            properties.to_xml(&other),
            Err(ObjectError::ObjectPropertiesMismatch { .. })
        ));
    }

    #[test]
    fn rejects_an_unexpected_object_type() {
        let xml = String::from_utf8(DATA_ELEMENT_XML.to_vec())
            .unwrap()
            .replacen("adtcore:type=\"DTEL/DE\"", "adtcore:type=\"DOMA/DD\"", 1);
        let error = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::UnexpectedObjectType { expected, actual })
                if expected == DataElement::WORKBENCH_TYPE && actual.as_str() == "DOMA/DD"
        ));
    }

    #[test]
    fn rejects_an_unexpected_object_name() {
        let xml = String::from_utf8(DATA_ELEMENT_XML.to_vec())
            .unwrap()
            .replacen("adtcore:name=\"ZTFRWTFRT\"", "adtcore:name=\"ZOTHER\"", 1);
        let error = parse(
            &reference(),
            DataElementPropertiesVersion::V2,
            xml.as_bytes(),
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ResponseError::Object(ObjectError::UnexpectedObjectName { expected, actual })
                if expected == "ZTFRWTFRT" && actual == "ZOTHER"
        ));
    }
}
