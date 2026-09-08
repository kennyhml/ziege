use garde::{Report, Validate};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use zaff::{
    ProjectedClassProperties, ProjectedDataElementProperties, ProjectedProgramProperties,
    ProjectionError,
};

fn class() -> Value {
    json!({
        "formatVersion": "1",
        "header": {"description": "Class", "originalLanguage": "en", "abapLanguageVersion": "standard"},
        "category": "generalObjectType", "fixPointArithmetic": true, "messageClass": "Z_MESSAGES",
        "descriptions": {
            "types": [{"name": "TYPE", "description": "Type"}],
            "attributes": [{"name": "ATTRIBUTE", "description": "Attribute"}],
            "events": [{
                "name": "EVENT", "description": "Event",
                "parameters": [{"name": "PARAMETER", "description": "Parameter"}]
            }],
            "methods": [{
                "name": "METHOD", "description": "Method",
                "parameters": [{"name": "PARAMETER", "description": "Parameter"}],
                "exceptions": [{"name": "EXCEPTION", "description": "Exception"}]
            }]
        }
    })
}

fn program() -> Value {
    json!({
        "formatVersion": "1", "header": {"description": "Program", "originalLanguage": "en"},
        "generalInformation": {
            "programType": "executableProgram", "programStatus": "unknown",
            "fixPointArithmetic": true, "editLocked": false, "startsUsingVariant": false,
            "authorizationGroup": "GROUP", "application": "A"
        },
        "logicalDatabase": {"name": "DATABASE", "selectionScreen": "100"}
    })
}

fn data_element() -> Value {
    json!({
        "formatVersion": "1",
        "header": {"description": "Element", "originalLanguage": "en", "abapLanguageVersion": "standard"},
        "dataTypeInformation": {
            "category": "predefinedType", "typeName": "Z_TYPE",
            "predefinedType": {"dataType": "CHAR", "length": 10, "decimals": 0}
        },
        "fieldLabels": {
            "short": "Short", "shortLength": 10, "medium": "Medium", "mediumLength": 20,
            "long": "Long", "longLength": 40, "heading": "Heading", "headingLength": 55
        },
        "additionalProperties": {
            "searchHelp": {"name": "Z_HELP", "parameter": "PARAMETER"},
            "bidirectionalOptions": {"basicDirection": "leftToRight", "noFiltering": false},
            "parameterId": "PID", "defaultComponentName": "COMPONENT",
            "changeDocumentRelevant": true, "noInputHistory": false
        }
    })
}

fn validate<T: DeserializeOwned + Validate<Context = ()>>(value: &Value) -> Result<(), Report> {
    serde_json::from_value::<T>(value.clone())
        .unwrap()
        .validate()
}

fn paths(report: &Report) -> Vec<String> {
    let mut paths: Vec<_> = report.iter().map(|(path, _)| path.to_string()).collect();
    paths.sort();
    paths
}

fn string_limits<T: DeserializeOwned + Validate<Context = ()>>(
    baseline: Value,
    fields: &[(&str, usize)],
) {
    validate::<T>(&baseline).unwrap();
    for &(pointer, max) in fields {
        for alphabet in ["a", "\u{e9}", "\u{1f600}", "e\u{301}"] {
            let mut text: String = alphabet.chars().cycle().take(max).collect();
            let mut document = baseline.clone();
            *document.pointer_mut(pointer).unwrap() = json!("");
            validate::<T>(&document).unwrap();
            *document.pointer_mut(pointer).unwrap() = json!(text);
            validate::<T>(&document).unwrap_or_else(|e| panic!("{pointer}, {alphabet:?}: {e}"));
            // One more codepoint, but no additional grapheme, must exceed the limit.
            text.push('\u{301}');
            *document.pointer_mut(pointer).unwrap() = json!(text);
            let report = validate::<T>(&document).expect_err(pointer);
            assert_eq!(report.iter().count(), 1, "{pointer}: {report}");
        }
    }
}

#[test]
fn class_string_bounds_and_every_description_dive() {
    let fields = [
        ("/header/description", 60),
        ("/messageClass", 20),
        ("/descriptions/types/0/name", 30),
        ("/descriptions/types/0/description", 60),
        ("/descriptions/attributes/0/name", 30),
        ("/descriptions/attributes/0/description", 60),
        ("/descriptions/events/0/name", 30),
        ("/descriptions/events/0/description", 60),
        ("/descriptions/events/0/parameters/0/name", 30),
        ("/descriptions/events/0/parameters/0/description", 60),
        ("/descriptions/methods/0/name", 30),
        ("/descriptions/methods/0/description", 60),
        ("/descriptions/methods/0/parameters/0/name", 30),
        ("/descriptions/methods/0/parameters/0/description", 60),
        ("/descriptions/methods/0/exceptions/0/name", 30),
        ("/descriptions/methods/0/exceptions/0/description", 60),
    ];
    string_limits::<ProjectedClassProperties>(class(), &fields);
}

#[test]
fn program_string_bounds() {
    let fields = [
        ("/header/description", 70),
        ("/generalInformation/authorizationGroup", 8),
        ("/generalInformation/application", 1),
        ("/logicalDatabase/name", 20),
        ("/logicalDatabase/selectionScreen", 3),
    ];
    string_limits::<ProjectedProgramProperties>(program(), &fields);
}

#[test]
fn data_element_string_bounds() {
    let fields = [
        ("/header/description", 60),
        ("/dataTypeInformation/typeName", 30),
        ("/fieldLabels/short", 10),
        ("/fieldLabels/medium", 20),
        ("/fieldLabels/long", 40),
        ("/fieldLabels/heading", 55),
        ("/additionalProperties/parameterId", 20),
        ("/additionalProperties/defaultComponentName", 30),
        ("/additionalProperties/searchHelp/name", 30),
        ("/additionalProperties/searchHelp/parameter", 30),
    ];
    string_limits::<ProjectedDataElementProperties>(data_element(), &fields);
}

#[test]
fn numeric_maxima_are_inclusive() {
    for (pointer, max) in [
        ("/dataTypeInformation/predefinedType/length", 999_999),
        ("/dataTypeInformation/predefinedType/decimals", 999_999),
        ("/fieldLabels/shortLength", 10),
        ("/fieldLabels/mediumLength", 20),
        ("/fieldLabels/longLength", 40),
        ("/fieldLabels/headingLength", 55),
    ] {
        let mut document = data_element();
        for value in [0, max] {
            *document.pointer_mut(pointer).unwrap() = json!(value);
            validate::<ProjectedDataElementProperties>(&document).expect(pointer);
        }
        *document.pointer_mut(pointer).unwrap() = json!(max + 1);
        let report = validate::<ProjectedDataElementProperties>(&document).expect_err(pointer);
        assert_eq!(report.iter().count(), 1, "{pointer}: {report}");
    }
}

#[test]
fn predefined_datatype_whitelist_is_exact() {
    let allowed = "ACCP CHAR CLNT CUKY CURR DF16_DEC DF16_RAW DF16_SCL DECFLOAT16 DF34_DEC \
        DF34_RAW DF34_SCL DECFLOAT34 DATS DATN DEC FLTP GEOM_EWKB INT1 INT2 INT4 INT8 LANG \
        LCHR LRAW NUMC PREC QUAN RAW RAWSTRING SSTRING STRING TIMS TIMN UNIT UTCLONG VARC";
    let disallowed = ["", "char", "Char", " CHAR", "CHAR ", "CHARACTER", "UNKNOWN"];
    for (value, valid) in allowed
        .split_whitespace()
        .map(|v| (v, true))
        .chain(disallowed.into_iter().map(|v| (v, false)))
    {
        let mut document = data_element();
        document["dataTypeInformation"]["predefinedType"]["dataType"] = json!(value);
        let result = validate::<ProjectedDataElementProperties>(&document);
        assert_eq!(result.is_ok(), valid, "{value:?}: {result:?}");
    }
}

#[test]
fn all_families_check_format_version_but_not_merge_language_restrictions() {
    let cases = [
        (
            class(),
            validate::<ProjectedClassProperties> as fn(&Value) -> Result<(), Report>,
        ),
        (program(), validate::<ProjectedProgramProperties>),
        (data_element(), validate::<ProjectedDataElementProperties>),
    ];
    for (mut document, check) in cases {
        // Language conversion is a merge concern, not a schema constraint.
        document["header"]["originalLanguage"] = json!("not-supported");
        check(&document).unwrap();
        for version in ["", "0", "2", "01", "1.0", " 1"] {
            document["formatVersion"] = json!(version);
            assert_eq!(paths(&check(&document).unwrap_err()), ["format_version"]);
        }
    }
}

#[test]
fn optional_fields_accept_none_and_empty_nested_blocks() {
    let mut document = json!({
        "formatVersion": "1", "header": {"description": "", "originalLanguage": "en"}
    });
    validate::<ProjectedClassProperties>(&document).unwrap();
    validate::<ProjectedProgramProperties>(&document).unwrap();
    document["dataTypeInformation"] = json!({"category": "predefinedType"});
    validate::<ProjectedDataElementProperties>(&document).unwrap();
    document["dataTypeInformation"]["predefinedType"] = json!({"dataType": "CHAR", "length": 0});
    document["fieldLabels"] = json!({});
    document["additionalProperties"] = json!({});
    validate::<ProjectedDataElementProperties>(&document).unwrap();
}

#[test]
fn all_description_arrays_require_unique_whole_entries_not_unique_names() {
    for (pointer, path) in [
        ("/descriptions/types", "descriptions.types"),
        ("/descriptions/attributes", "descriptions.attributes"),
        ("/descriptions/events", "descriptions.events"),
        ("/descriptions/methods", "descriptions.methods"),
        (
            "/descriptions/events/0/parameters",
            "descriptions.events[0].parameters",
        ),
        (
            "/descriptions/methods/0/parameters",
            "descriptions.methods[0].parameters",
        ),
        (
            "/descriptions/methods/0/exceptions",
            "descriptions.methods[0].exceptions",
        ),
    ] {
        let mut document = class();
        let entry = document.pointer(pointer).unwrap()[0].clone();
        let mut same_name = entry.clone();
        same_name["description"] = json!("Different description");
        *document.pointer_mut(pointer).unwrap() = json!([entry, same_name]);
        validate::<ProjectedClassProperties>(&document).expect(pointer);
        *document.pointer_mut(pointer).unwrap() = json!([entry, same_name, entry]);
        assert_eq!(
            paths(&validate::<ProjectedClassProperties>(&document).unwrap_err()),
            [path]
        );
        *document.pointer_mut(pointer).unwrap() = json!([]);
        validate::<ProjectedClassProperties>(&document).expect(pointer);
    }
}

#[test]
fn deserialization_does_not_validate_and_projection_error_retains_indexed_report() {
    let mut document = class();
    document["formatVersion"] = json!("2");
    document["messageClass"] = json!("x".repeat(21));
    document["header"]["description"] = json!("x".repeat(61));
    document["descriptions"]["methods"][0]["parameters"][0]["description"] = json!("x".repeat(61));
    document["descriptions"]["methods"][0]["exceptions"][0]["name"] = json!("x".repeat(31));
    document["descriptions"]["events"][0]["parameters"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "x".repeat(31), "description": "Second parameter"}));
    let decoded: ProjectedClassProperties = serde_json::from_str(&document.to_string()).unwrap();
    assert_eq!(decoded.format_version, "2");
    let report = decoded.validate().unwrap_err();
    assert_eq!(
        paths(&report),
        [
            "descriptions.events[0].parameters[1].name",
            "descriptions.methods[0].exceptions[0].name",
            "descriptions.methods[0].parameters[0].description",
            "format_version",
            "header.description",
            "message_class",
        ]
    );
    let message = report.to_string();
    let error = ProjectionError::from(report);
    let ProjectionError::Validation(report) = &error else {
        panic!("{error:?}")
    };
    let source = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<Report>()
        .unwrap();
    assert!(std::ptr::eq(source, report));
    assert_eq!(source.to_string(), message);
}

#[test]
fn nested_program_and_data_element_paths_use_rust_names() {
    let mut document = program();
    document["generalInformation"]["authorizationGroup"] = json!("x".repeat(9));
    document["logicalDatabase"]["selectionScreen"] = json!("1234");
    assert_eq!(
        paths(&validate::<ProjectedProgramProperties>(&document).unwrap_err()),
        [
            "general_information.authorization_group",
            "logical_database.selection_screen",
        ]
    );
    let mut document = data_element();
    document["dataTypeInformation"]["predefinedType"]["dataType"] = json!("INVALID");
    document["fieldLabels"]["shortLength"] = json!(11);
    document["additionalProperties"]["searchHelp"]["name"] = json!("x".repeat(31));
    assert_eq!(
        paths(&validate::<ProjectedDataElementProperties>(&document).unwrap_err()),
        [
            "additional_properties.search_help.name",
            "data_type_information.predefined_type.data_type",
            "field_labels.short_length",
        ]
    );
}
