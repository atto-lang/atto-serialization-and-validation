//! Independent diagnostic envelope schema validation; no native codec claim.
use serde_json::{Value, json};
use std::{fs, path::Path};
fn root() -> &'static Path { Path::new(env!("CARGO_MANIFEST_DIR")) }
fn load(path:&str)->Value {
    serde_json::from_slice(&fs::read(root().join(path)).expect("fixture file")).expect("valid JSON")
}
fn schema()->Value { load("contracts/protobuf/authored.schema.json") }
fn validator(s: &Value) -> jsonschema::Validator {
    let mut schema=s.clone();
    schema["$ref"]=json!("#/$defs/WireMessage");
    jsonschema::validator_for(&schema).expect("authored schema must compile")
}
#[test]
fn independent_authored_schema_executes_all_fixtures() {
    let v=validator(&schema());
    for (name,expected,count) in [("valid",true,2),("invalid",false,8)] {
        let dir=root().join("contracts/protobuf/instances/WireMessage").join(name);
        let mut seen=0;
        for entry in fs::read_dir(dir).unwrap() {
            let p=entry.unwrap().path();
            assert_eq!(p.extension().and_then(|x|x.to_str()),Some("json"));
            let value:Value=serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
            assert_eq!(v.is_valid(&value),expected,"{}",p.display());
            seen+=1;
        }
        assert_eq!(seen,count,"fixture group changed");
    }
}
#[test]
fn limits_and_reserved_tags_are_envelope_only() {
    let v=validator(&schema());
    for tag in [1,19000,19999,536870911] {
        for kind in [0,1,2,5] {
            assert!(v.is_valid(&json!({"fields":[{"fieldNumber":tag,"wireType":kind,"payload":[0,255]}]})));
        }
    }
    for field in [
        json!({"fieldNumber":0,"wireType":0,"payload":[]}),
        json!({"fieldNumber":536870912u64,"wireType":0,"payload":[]}),
        json!({"fieldNumber":1,"wireType":3,"payload":[]}),
        json!({"fieldNumber":1,"wireType":0,"payload":[-1]}),
        json!({"fieldNumber":1,"wireType":0,"payload":[256]}),
        json!({"fieldNumber":1,"wireType":0,"payload":[1.5]}),
        json!({"fieldNumber":1,"wireType":0,"payload":[],"extra":true}),
    ] { assert!(!v.is_valid(&json!({"fields":[field]}))); }
    assert!(!v.is_valid(&json!({"fields":[],"extra":true})));
}
#[test]
fn fixture_detects_authored_authority_drift() {
    let mut s=schema();
    let value=load("contracts/protobuf/instances/WireMessage/invalid/field-over.json");
    assert!(!validator(&s).is_valid(&value));
    s["$defs"]["WireField"]["properties"]["fieldNumber"]["maximum"]=json!(536870912u64);
    assert!(validator(&s).is_valid(&value),"mutant should expose an actual changed acceptance decision");
}
