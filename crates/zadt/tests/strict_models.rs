use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use zadt::*;

// Exercise every populated nested struct in the wire-shaped JSON, not just roots.
fn reject_unknown_json_fields<T: DeserializeOwned + Debug>(root: &Value, pointer: &str) {
    match root.pointer(pointer).unwrap() {
        Value::Object(fields) => {
            let mut changed = root.clone();
            changed.pointer_mut(pointer).unwrap()["unexpected"] = true.into();
            let error = serde_json::from_value::<T>(changed).unwrap_err();
            assert!(
                error.to_string().contains("unknown field `unexpected`"),
                "{} at {pointer}: {error}",
                std::any::type_name::<T>()
            );
            for key in fields.keys() {
                let key = key.replace('~', "~0").replace('/', "~1");
                reject_unknown_json_fields::<T>(root, &format!("{pointer}/{key}"));
            }
        }
        Value::Array(items) => {
            for index in 0..items.len() {
                reject_unknown_json_fields::<T>(root, &format!("{pointer}/{index}"));
            }
        }
        _ => {}
    }
}

fn check_properties<T: DeserializeOwned + Serialize + Debug>(xml: &str) {
    let properties: T = serde_xml_rs::from_str(xml).unwrap();
    let json = serde_json::to_value(properties).unwrap();
    serde_json::from_value::<T>(json.clone()).unwrap();
    reject_unknown_json_fields::<T>(&json, "");
}

#[test]
fn property_families_reject_unknown_json_fields_recursively() {
    check_properties::<ClassProperties>(include_str!("fixtures/class-cl-adt-uri-mapper-v4.xml"));
    check_properties::<InterfaceProperties>(include_str!(
        "fixtures/interface-if-adt-uri-mapper-v5.xml"
    ));
    check_properties::<PackageProperties>(include_str!("fixtures/package-sadt-tools-core.xml"));
    check_properties::<ProgramProperties>(include_str!("fixtures/program-z-test.xml"));
    check_properties::<IncludeProperties>(include_str!("fixtures/include-ztest.xml"));
    check_properties::<FunctionGroupProperties>(include_str!(
        "fixtures/function-group-z-test-group-v2.xml"
    ));
    check_properties::<FunctionModuleProperties>(include_str!(
        "fixtures/function-module-zzzzfunc.xml"
    ));
    check_properties::<FunctionGroupIncludeProperties>(include_str!(
        "fixtures/function-group-include-lz-test-grouptop.xml"
    ));
    check_properties::<DomainProperties>(include_str!("fixtures/domain-xfeld.xml"));
    check_properties::<DataElementProperties>(include_str!(
        "fixtures/data-element-ztfrwtfrt-v2.xml"
    ));
    check_properties::<DataDefinitionProperties>(include_str!(
        "fixtures/data-definition-i-businesspartner.xml"
    ));
    check_properties::<AccessControlProperties>(include_str!(
        "fixtures/access-control-sdsh-cds-domain-val-dcl.xml"
    ));
    check_properties::<AnnotationDefinitionProperties>(include_str!(
        "fixtures/annotation-definition-ui.xml"
    ));
    check_properties::<MetadataExtensionProperties>(include_str!(
        "fixtures/metadata-extension-c-mdoapplicationscope.xml"
    ));
    check_properties::<ServiceDefinitionProperties>(include_str!(
        "fixtures/service-definition-managedistributions.xml"
    ));
}

#[test]
fn generated_creation_models_and_custom_sources_reject_unknown_fields() {
    let properties = ClassCreateProperties::builder()
        .description("Created class")
        .package("$TMP")
        .template(ClassTemplate::new("Z_TEMPLATE").property("key", "value"))
        .build()
        .unwrap();
    let json = serde_json::to_value(&properties).unwrap();
    serde_json::from_value::<ClassCreateProperties>(json.clone()).unwrap();
    reject_unknown_json_fields::<ClassCreateProperties>(&json, "");

    let xml = String::from_utf8(properties.to_xml().unwrap()).unwrap();
    serde_xml_rs::from_str::<ClassCreateProperties>(&xml).unwrap();
    for element in [
        "class:abapClass",
        "adtcore:packageRef",
        "class:include",
        "abapsource:template",
        "abapsource:property",
    ] {
        let marker = format!("<{element}");
        let changed = xml.replacen(&marker, &format!("{marker} unexpected=\"value\""), 1);
        assert_ne!(changed, xml, "{element}");
        let error = serde_xml_rs::from_str::<ClassCreateProperties>(&changed).unwrap_err();
        assert!(
            error.to_string().contains("unknown field `@unexpected`"),
            "{element}: {error}"
        );
    }
}
