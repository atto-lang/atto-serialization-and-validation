use atto_actor_contract_conformance::serde_receipts::{verify,sha256,BACKENDS,MANIFEST};
use serde_json::{json,Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64,Ordering};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp{
    fn new()->Self{
        let p=std::env::temp_dir().join(format!("serde-structure-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&p).unwrap();Self(p)
    }
}
impl Drop for Temp{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn synthetic()->Vec<Value>{
    let m:Value=serde_json::from_slice(MANIFEST).unwrap();
    let rows:Vec<_>=m["cases"].as_array().unwrap().iter().map(|r|{
        let text=r["bytes_hex"].as_str().unwrap();
        let bytes:Vec<_>=(0..text.len()).step_by(2).map(|i|u8::from_str_radix(&text[i..i+2],16).unwrap()).collect();
        json!({"id":r["id"],"input_sha256":sha256(&bytes).unwrap(),"result":r["expected_result"],
            "error_code":r["expected_error_code"],"decoded_value":r["expected_decoded"]})
    }).collect();
    let manifest=sha256(MANIFEST).unwrap();
    BACKENDS.iter().enumerate().map(|(i,b)|{
        let job=i+201;let run="https://github.com/synthetic/host-only/actions/runs/1";
        json!({"backend":b,"status":"PASS","source_head_sha":"a".repeat(40),"source_tree_sha":"b".repeat(40),
            "fixture_manifest_sha256":manifest,"compiler_sha256":"c".repeat(64),"guest_artifact_sha256":"d".repeat(64),
            "stdout_sha256":"e".repeat(64),"stderr_sha256":"f".repeat(64),
            "guest_artifact_kind":if *b=="wasm"{"wasm-module"}else{"native-executable"},
            "guest_exit_code":0,"compile_command":"SYNTHETIC ONLY","execute_command":"SYNTHETIC ONLY",
            "run_id":1,"job_id":job,"executed_step_count":3,"compiler_version":"SYNTHETIC ONLY",
            "target":"synthetic","runner_os":"synthetic","run_url":run,"job_url":format!("{run}/job/{job}"),
            "compiled_native_guest":true,"executed_native_guest":true,"host_only":false,
            "full_aot":*b!="wasm","independent_cranelift":*b=="cranelift-independent",
            "memory":{"allocation_limit_bytes":1048576,"peak_alloc_bytes":1024,"max_depth":128},
            "instrumentation":{"kind":"SYNTHETIC ONLY","result":"PASS","evidence_sha256":"0".repeat(64)},
            "cases":rows})
    }).collect()
}
fn write(t:&Temp,data:&[Value]){
    for (b,v) in BACKENDS.iter().zip(data){fs::write(t.0.join(format!("{b}.json")),serde_json::to_vec(v).unwrap()).unwrap();}
}
fn check(t:&Temp)->Result<(),String>{verify(&t.0,&"a".repeat(40),&"b".repeat(40))}
#[test]fn synthetic_structure_is_not_execution_proof(){
    assert_eq!(sha256(b"abc").unwrap(),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let t=Temp::new();write(&t,&synthetic());check(&t).unwrap();
}
#[test]fn tampered_fields_and_nested_cases_fail_closed(){
    let t=Temp::new();let base=synthetic();
    let edits=[
        ("/executed_step_count",json!(0)),("/executed_step_count",json!(true)),
        ("/compiled_native_guest",json!(false)),("/executed_native_guest",json!(false)),
        ("/host_only",json!(true)),("/source_head_sha",json!("0".repeat(40))),
        ("/source_tree_sha",json!("0".repeat(40))),("/fixture_manifest_sha256",json!("0".repeat(64))),
        ("/guest_artifact_sha256",Value::Null),("/stdout_sha256",Value::Null),
        ("/guest_exit_code",json!(7)),("/guest_artifact_kind",json!("mock")),("/full_aot",json!(false)),
        ("/memory/peak_alloc_bytes",json!(33554432)),("/memory/max_depth",json!(129)),
        ("/instrumentation/result",json!("NOT_RUN")),("/instrumentation/kind",json!(true)),
        ("/instrumentation/kind",json!(" ")),("/cases/0/result",json!("Ok")),
        ("/cases/0/input_sha256",json!("0".repeat(64))),("/cases/0/error_code",json!("wrong")),
        ("/cases/0/decoded_value",json!({"kind":"int","value":1})),
        ("/cases/20/decoded_value",json!({"kind":"map","integer_pairs":[[1,3]]})),
        ("/run_url",json!("https://example.invalid/synthetic")),
        ("/run_url",json!("https://github.com:444/synthetic/host-only/actions/runs/1")),
        ("/run_url",json!("https://github.com/synthetic/host-only/actions/runs/01")),
        ("/job_url",json!("https://github.com/o/r/actions/runs/1/job/5")),
        ("/job_id",json!(201)),("/memory",json!([])),("/instrumentation",json!("bad")),
        ("/cases",json!([])),
    ];
    for (pointer,value) in edits{
        let mut data=base.clone();*data[1].pointer_mut(pointer).unwrap()=value;write(&t,&data);
        assert!(check(&t).is_err(),"{pointer}");
    }
    let mut data=base.clone();data[1]["cases"].as_array_mut().unwrap().pop();write(&t,&data);assert!(check(&t).is_err());
    let mut data=base.clone();data[1]["cases"][20].as_object_mut().unwrap().remove("decoded_value");write(&t,&data);assert!(check(&t).is_err());
    let mut data=base.clone();data[1]["cases"][1]=data[1]["cases"][0].clone();write(&t,&data);assert!(check(&t).is_err());
    let mut data=base;data[3]["independent_cranelift"]=json!(false);write(&t,&data);assert!(check(&t).is_err());
}
#[test]fn unsafe_entries_duplicates_and_noninteger_syntax_fail_closed(){
    let t=Temp::new();let base=synthetic();write(&t,&base);
    let p=t.0.join("llvm.json");let raw=fs::read_to_string(&p).unwrap();
    for bad in [format!("{{\"status\":\"FAIL\",{}",&raw[1..]),
        raw.replacen("\"max_depth\":128","\"max_depth\":128,\"max_depth\":128",1),
        raw.replacen("\"guest_exit_code\":0","\"guest_exit_code\":0.0",1),
        "[]".into()," ".repeat(512*1024+1)]{
        fs::write(&p,bad).unwrap();assert!(check(&t).is_err());
    }
    write(&t,&base);let extra=t.0.join("unexpected.json");fs::write(&extra,b"{}").unwrap();assert!(check(&t).is_err());
    fs::remove_file(extra).unwrap();fs::remove_file(&p).unwrap();assert!(check(&t).is_err());
    std::os::unix::fs::symlink(t.0.join("java-graalvm.json"),&p).unwrap();assert!(check(&t).is_err());
    fs::remove_file(&p).unwrap();write(&t,&base);check(&t).unwrap();
}
