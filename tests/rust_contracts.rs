use atto_actor_contract_conformance::{decode_and_validate, typespec_fields, wit_fields, MAX_WIRE_BYTES};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA: &str = include_str!("../contracts/v1/actor-envelope.schema.json");
const TSP: &str = include_str!("../contracts/v1/actor-envelope.tsp");
const WIT: &str = include_str!("../contracts/v1/actor-envelope.wit");

fn schema() -> Value { serde_json::from_str(SCHEMA).unwrap() }
fn base() -> Value { json!({"version":1,"message_id":"mid-1","source_actor":"src"}) }
fn message() -> Value { let mut v=base(); v["event"]=json!("message");v["target_actor"]=json!("dest");v["payload_utf8"]=json!("hi");v }
fn failure() -> Value { let mut v=base();v["event"]=json!("failure");v["error_code"]=json!("IO_ERROR");v }
fn shutdown() -> Value { let mut v=base();v["event"]=json!("shutdown");v }
fn check(v: &Value) -> Result<Value,String> { decode_and_validate(v.to_string().as_bytes(),&schema()) }
fn reject(v: Value) { assert!(check(&v).is_err(),"accepted invalid JSON: {v}"); }

#[test]
fn all_three_event_variants_pass() {
    assert!(jsonschema::draft202012::meta::is_valid(&schema()));
    for v in [message(),failure(),shutdown()] { assert_eq!(check(&v).unwrap(),v); }
}
#[test]
fn rejects_event_confusion_and_missing_properties() {
    let m=message();let f=failure();let s=shutdown();
    for (field, value) in [("event",json!("unknown")),("event",json!("failure")),("version",json!(2)),
                           ("message_id",json!("")),("source_actor",json!("")),("target_actor",json!("")),
                           ("error_code",json!("IO_ERROR"))] {
        let mut v=m.clone();v[field]=value;reject(v);
    }
    let mut v=m.clone();v.as_object_mut().unwrap().remove("payload_utf8");reject(v);
    let mut v=f.clone();v["target_actor"]=json!("x");reject(v);
    let mut v=s.clone();v["payload_utf8"]=json!("");reject(v);
    let mut v=s;v["target_actor"]=json!("x");reject(v);
}
#[test]
fn rejects_negative_boundary_fixtures() {
    let m=message();let f=failure();
    for (field, value) in [("message_id",json!("x".repeat(129))),("source_actor",json!("x".repeat(129))),
            ("target_actor",json!("x".repeat(129))),("payload_utf8",json!("x".repeat(65537)))] {
        let mut v=m.clone();v[field]=value;reject(v);
    }
    for code in ["X".repeat(65),"BAD\n".into(),"bad".into(),"A\r".into()] {
        let mut v=f.clone();v["error_code"]=json!(code);reject(v);
    }
    let mut v=m;v["extra"]=json!(true);reject(v);
}
#[test]
fn rejects_type_confusion_across_all_fields() {
    let mut n=0;
    for (base,field) in [(message(),"message_id"),(message(),"source_actor"),(message(),"target_actor"),
                         (failure(),"error_code"),(message(),"event")] {
        for wrong in [json!(null),json!(true),json!(123),json!(1.5),json!([]),json!({}),json!("")] {
            let mut v=base.clone();v[field]=wrong;reject(v);n+=1;
        }
    }
    for wrong in [json!(null),json!(true),json!(123),json!(1.5),json!([]),json!({})] {
        let mut v=message();v["payload_utf8"]=wrong;reject(v);n+=1;
    }
    assert!(n>=40);
}
#[test]
fn rejects_malformed_unicode_float_duplicate_and_oversize_json() {
    let m=message();
    let encoded=m.to_string();
    for key in m.as_object().unwrap().keys() {
        let duplicate=format!("{},\"{}\":{}}}",&encoded[..encoded.len()-1],key,m[key]);
        assert!(decode_and_validate(duplicate.as_bytes(),&schema()).is_err(),"{key}");
    }
    for raw in [r#"{"a":{"nested":1,"nested":2}}"#.as_bytes(),b"\xff",b"{",b"[]",b"null",
                br#"{"version":1.0}"#,br#"{"version":1e0}"#,br#"{"foo":NaN}"#,
                br#"{"message_id":"\ud800"}"#,br#"{"a":1}{"b":2}"#] {
        assert!(decode_and_validate(raw,&schema()).is_err(),"{raw:?}");
    }
    assert!(decode_and_validate(&vec![b' ';MAX_WIRE_BYTES+1],&schema()).is_err());
}
#[test]
fn wit_type_and_typespec_field_drift_is_detected() {
    let expected: BTreeMap<String,String>=[
        ("version","u32"),("event","event-kind"),("message-id","string"),("source-actor","string"),
        ("target-actor","option<string>"),("payload-utf8","option<string>"),("error-code","option<string>")
    ].into_iter().map(|(k,v)|(k.into(),v.into())).collect();
    assert_eq!(wit_fields(WIT,"actor-envelope"),expected);
    let projection: BTreeSet<_>=expected.keys().map(|x|x.replace('-','_')).collect();
    let schema_fields: BTreeSet<_>=schema()["properties"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(projection,schema_fields);
    assert_ne!(wit_fields(&WIT.replace("event: event-kind","event: string"),"actor-envelope"),expected);
    let fields=typespec_fields(TSP);
    for (key,types) in [
        ("version",vec!["1"]),("message_id",vec!["string"]),("source_actor",vec!["string"]),
        ("target_actor",vec!["string"]),("payload_utf8",vec!["string"]),("error_code",vec!["string"]),
        ("event",vec![r#""message""#,r#""failure""#,r#""shutdown""#])
    ] {
        let actual=fields.get(key).unwrap_or_else(|| panic!("missing field {key}"));
        assert_eq!(actual,&types.into_iter().map(str::to_string).collect::<BTreeSet<_>>());
    }
    assert_ne!(typespec_fields(&TSP.replace("message_id: string;","message_id: int32;")),fields);
}
#[test]
fn tagged_variants_and_constraints_remain_explicit() {
    let s=schema();
    let variants=s["oneOf"].as_array().unwrap();
    let events: BTreeSet<_>=variants.iter().map(|v|v["properties"]["event"]["const"].as_str().unwrap()).collect();
    assert_eq!(events,BTreeSet::from(["message","failure","shutdown"]));
    for constraint in ["@minLength(1)","@maxLength(128)","@maxLength(65536)"] {
        assert!(TSP.contains(constraint),"missing TypeSpec constraint {constraint}");
    }
    assert!(WIT.contains("option<string>"));
    assert!(s["properties"]["error_code"]["pattern"].as_str().unwrap().contains("(?![\\\\s\\\\S])")==false); 
    assert!(s["properties"]["error_code"]["pattern"].as_str().unwrap().contains("(?![\\s\\S])"));
}
