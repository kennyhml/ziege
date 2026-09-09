//! JSON schema shapes for the newly supported metadata families.

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use zaff::{
    dcls::ProjectedAccessControlProperties, ddla::ProjectedAnnotationDefinitionProperties,
    ddls::ProjectedDataDefinitionProperties, ddlx::ProjectedMetadataExtensionProperties,
    devc::ProjectedPackageProperties, doma::ProjectedDomainProperties,
    intf::ProjectedInterfaceProperties, srvd::ProjectedServiceDefinitionProperties,
};

fn accepts<T: DeserializeOwned>(value: Value) -> bool {
    serde_json::from_value::<T>(value).is_ok()
}

type ShapeCase = (fn(Value) -> bool, Value);

#[test]
fn headers_require_objects_and_enum_fields_require_strings() {
    let cases: &[ShapeCase] = &[
        (
            accepts::<ProjectedInterfaceProperties>,
            json!({"category":"general"}),
        ),
        (accepts::<ProjectedAccessControlProperties>, json!({})),
        (accepts::<ProjectedMetadataExtensionProperties>, json!({})),
        (
            accepts::<ProjectedAnnotationDefinitionProperties>,
            json!({}),
        ),
        (
            accepts::<ProjectedDataDefinitionProperties>,
            json!({"sourceOrigin":"abapDevelopmentTools", "sourceType":"unknown"}),
        ),
        (
            accepts::<ProjectedServiceDefinitionProperties>,
            json!({"generalInformation":{"sourceOrigin":"abapDevelopmentTools", "sourceType":"definition"}}),
        ),
        (
            accepts::<ProjectedDomainProperties>,
            json!({"format":{"dataType":"CHAR", "length":1}}),
        ),
        (
            accepts::<ProjectedPackageProperties>,
            json!({"generalInformation":{"type":"development"}}),
        ),
    ];
    for (accepts, fields) in cases {
        let mut valid = fields.clone();
        valid["formatVersion"] = json!("1");
        valid["header"] = json!({"description":"Example", "originalLanguage":"en"});
        assert!(accepts(valid.clone()));
        for header in [
            json!(["Example", "en"]),
            json!(["Example", "en", "standard"]),
            Value::Null,
        ] {
            let mut invalid = valid.clone();
            invalid["header"] = header;
            assert!(!accepts(invalid));
        }
        for path in [
            "/category",
            "/sourceOrigin",
            "/sourceType",
            "/generalInformation/type",
            "/generalInformation/sourceOrigin",
            "/generalInformation/sourceType",
        ] {
            let mut invalid = valid.clone();
            if let Some(value) = invalid.pointer_mut(path) {
                *value = json!({value.as_str().unwrap(): null});
                assert!(!accepts(invalid));
            }
        }
    }
}

#[test]
fn dictionary_collections_reject_positional_entries_and_unknown_members() {
    let domain = json!({"formatVersion":"1", "header":{"description":"Domain", "originalLanguage":"en"}, "format":{"dataType":"CHAR", "length":1}});
    for (field, value) in [
        ("fixedValues", json!([["X", "Yes"]])),
        ("fixedValueIntervals", json!([["A", "Z", "Letters"]])),
        ("valueTable", json!(["Z_TABLE"])),
        ("fixedValueAppends", json!([["Z_APPEND"]])),
        ("outputCharacteristics", json!({"style":{"normal":null}})),
    ] {
        let mut invalid = domain.clone();
        invalid[field] = value;
        assert!(!accepts::<ProjectedDomainProperties>(invalid));
    }
    let package =
        json!({"formatVersion":"1", "header":{"description":"Package", "originalLanguage":"en"}});
    for value in [
        json!([["Z_INTERFACE", "none"]]),
        json!([{"packageInterface":"Z_INTERFACE", "severity":{"none":null}}]),
        json!([{"unexpected":true}]),
    ] {
        let mut invalid = package.clone();
        invalid["useAccesses"] = value;
        assert!(!accepts::<ProjectedPackageProperties>(invalid));
    }
}
