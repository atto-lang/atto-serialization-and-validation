//! Generated guest receipt candidate. Trusted launcher/runner authorization
//! and actual compiler semantics remain external. Never authenticate a peer
//! using this API or treat a synthetic executable as a native compiler.
use crate::{Unique,native_input_manifest as inputs,serde_receipts::sha256_bounded};
use serde::Deserialize;
use serde_json::{json,Value};
use std::path::Path;
#[allow(dead_code)]
#[path="../scripts/zorde-isolated.rs"] mod isolated;
#[allow(dead_code)]
#[path="../scripts/verify-zorde-checkout.rs"] mod checkout;
type Result<T>=std::result::Result<T,String>;
const LIMIT:usize=8*1024*1024;
const LOG_LIMIT:usize=16*1024*1024;
const IR:&str="artifacts/zorde-scalars-contract-ir.json";
const PARITY:&str="artifacts/zorde-scalars-parity.json";
const VERIFY:&str="artifacts/zorde-scalars-consumer-verification.json";
const PLAN:&str="artifacts/zorde-generated/registry-plan.json";
const SPECS:&str="artifacts/zorde-generated/registry-specs.ores";
const GUEST:&str="artifacts/zorde-generated/registry-smoke.ores";
const BUILT:&str="artifacts/zorde-generated/build-complete.json";
const MANIFEST:&str="zorde-generated-inputs-before.json";
const RECEIPT:&str="zorde-generated-smoke-native-receipt.json";
const MARKER:&[u8]=b"zorde generated registry guest smoke passed";
pub struct Approved<'a>{
    pub head:&'a str,pub tree:&'a str,pub compiler_sha:&'a str,pub node_sha:&'a str,
    pub ir_sha:&'a str,pub parity_sha:&'a str,pub verification_sha:&'a str,
}
fn need(ok:bool,why:&str)->Result<()> {if ok{Ok(())}else{Err(why.into())}}
fn hex(s:&str,n:usize)->bool{s.len()==n&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn digest(m:&Value,name:&str)->Result<String>{
    let s=m["files"].get(name).and_then(Value::as_str).ok_or("missing manifest digest")?;
    need(hex(s,64),"invalid manifest digest")?;Ok(s.into())
}
fn object(root:&Path,m:&Value,name:&str)->Result<Value>{
    let raw=inputs::read(&root.join(name),LIMIT)?;
    need(sha256_bounded(&raw,LIMIT)?==digest(m,name)?,"input changed since manifest")?;
    let mut d=serde_json::Deserializer::from_slice(&raw);
    let v=Unique::deserialize(&mut d).map_err(|e|e.to_string())?.0;
    d.end().map_err(|e|e.to_string())?;
    need(v.is_object(),"expected JSON object")?;Ok(v)
}
fn eq(v:&Value,key:&str,want:&Value)->Result<()>{
    need(v.get(key)==Some(want),&format!("invalid {key}"))
}
fn static_authority(root:&Path,m:&Value,a:&Approved<'_>)->Result<String>{
    for (path,want) in [(IR,a.ir_sha),(PARITY,a.parity_sha),(VERIFY,a.verification_sha)]{
        need(digest(m,path)?==want,"independent authority digest mismatch")?;
    }
    let v=object(root,m,VERIFY)?;let ir=object(root,m,IR)?;
    let parity=object(root,m,PARITY)?;let plan=object(root,m,PLAN)?;let built=object(root,m,BUILT)?;
    let id=ir["irId"].as_str().filter(|s|!s.is_empty()&&s.len()<=1024).ok_or("missing IR identity")?;
    let ids=json!(["Zorde.Validation.Port","Zorde.Validation.ShortLabel"]);
    eq(&v,"schema",&json!("ores.typespec-json-schema-validator.consumer-verification-receipt/v1"))?;
    for o in [&v,&ir]{eq(o,"status",&json!("passed"))?;eq(o,"admissible",&json!(true))?;}
    for key in ["expectedIrId","computedIrId","suppliedIrId"]{eq(&v,key,&json!(id))?;}
    eq(&v,"declarationIds",&ids)?;
    eq(&plan,"irId",&json!(id))?;
    let entries=plan["entries"].as_array().ok_or("missing plan entries")?;
    let actual:Vec<_>=entries.iter().map(|e|e["schemaId"].clone()).collect();
    need(json!(actual)==ids,"wrong plan declarations")?;
    for key in ["releaseReady","runtimeImplementationVerified"]{eq(&plan,key,&json!(false))?;}
    eq(&built,"schema",&json!("oreslang.zorde.static-registry-build-complete/v1"))?;
    eq(&built,"irId",&json!(id))?;
    let run=plan["tjsvRunId"].as_str().filter(|s|!s.is_empty()).ok_or("missing TJSV run ID")?;
    eq(&built,"tjsvRunId",&json!(run))?;
    for key in ["guestSmokeCompiled","guestSmokeExecuted","nativeGuestExecuted","releaseReady"]{eq(&built,key,&json!(false))?;}
    for (key,path) in [("planSha256",PLAN),("guestSha256",SPECS),("smokeSha256",GUEST)]{eq(&built,key,&json!(digest(m,path)?))?;}
    let corpus=parity["configuration"]["differential"]["instanceCorpusDigest"].as_str().ok_or("missing corpus digest")?;
    need(hex(corpus,64),"malformed corpus digest")?;
    eq(&built,"reviewedCorpusDigest",&json!(corpus))?;
    eq(&built,"reviewedCorpusInstances",&json!(9))?;
    let sum=&parity["differential"]["summary"];
    for (k,n) in [("corpusInstances",9),("declarationsWithReviewedPositiveAndNegative",2),("reviewedUnassertedInstances",0)]{eq(sum,k,&json!(n))?;}
    Ok(id.into())
}
fn exact_marker(log:&[u8])->bool{
    !log.contains(&0)&&std::str::from_utf8(log).is_ok()&&
        log.split(|b|*b==b'\n').filter(|line|*line==MARKER).count()==1
}
fn executables(compiler:&Path,node:&Path,a:&Approved<'_>)->Result<()>{
    need(isolated::executable_digest(compiler)?==a.compiler_sha,"compiler digest mismatch")?;
    need(isolated::executable_digest(node)?==a.node_sha,"Node digest mismatch")
}
/// Captures execution itself; no API accepts a claimed exit status or a
/// caller-supplied success log. Requires independently approved binaries and
/// authority digests and an exclusive source-local runner. This candidate
/// is not wired into the production harness or its private approval gates.
pub fn execute_and_issue(root:&Path,evidence:&Path,compiler:&Path,node:&Path,a:&Approved<'_>)->Result<()>{
    need(hex(a.head,40)&&hex(a.tree,40),"invalid source identity")?;
    for s in [a.compiler_sha,a.node_sha,a.ir_sha,a.parity_sha,a.verification_sha]{need(hex(s,64),"invalid approved digest")?;}
    need(!evidence.starts_with(root),"evidence inside checkout")?;
    need(!evidence.join(RECEIPT).exists(),"receipt already exists")?;
    checkout::verify_tracked(root,a.head,a.tree)?;
    executables(compiler,node,a)?;
    let file=evidence.join(MANIFEST);
    let hash=inputs::capture(root,&file)?;
    let m=inputs::verified_manifest(root,&file,&hash)?;
    let id=static_authority(root,&m,a)?;
    let log=isolated::capture(evidence,compiler,&root.join(GUEST),180)?;
    need(exact_marker(&log),"missing or ambiguous exact success marker")?;
    executables(compiler,node,a)?;
    checkout::verify_tracked(root,a.head,a.tree)?;
    let after=inputs::verified_manifest(root,&file,&hash)?;
    need(static_authority(root,&after,a)?==id,"authority changed")?;
    let log_hash=sha256_bounded(&log,LOG_LIMIT)?;
    let log_again=inputs::read(&evidence.join("zorde-generated-smoke-native.log"),LOG_LIMIT)?;
    need(log_again==log,"log changed before receipt")?;
    // Candidate schema is deliberately distinct. It is a process observation
    // from a trusted supplied binary, not production guest certification.
    let value=json!({
        "schema":"oreslang.zorde.source-local.generated-guest-candidate/v1",
        "source_head_sha":a.head,"source_root_tree_sha":a.tree,
        "compiler_executable_sha256":a.compiler_sha,"node_executable_sha256":a.node_sha,
        "plan_ir_id":id,"plan_sha256":digest(&after,PLAN)?,
        "specs_sha256":digest(&after,SPECS)?,"smoke_guest_sha256":digest(&after,GUEST)?,
        "native_log_sha256":log_hash,"pre_execution_input_manifest_sha256":hash,
        "source_bound_guest_inputs_unchanged":true,"static_guest_import_closure_sealed":true,
        "static_guest_import_module_count":after["staticGuestImportClosure"].as_array().ok_or("invalid closure")?.len(),
        "approved_process_executed":true,"native_guest_executed":false,"success_marker_observed":true,"process_exit_code":0,
        "compiler_dependency_closure_attested":false,"actor_shareability_certified":false,
        "cross_backend_parity_certified":false,"release_ready":false
    });
    let mut raw=serde_json::to_vec_pretty(&value).map_err(|e|e.to_string())?;raw.push(b'\n');
    isolated::private_output(evidence,RECEIPT,&raw)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn exact_marker_refuses_spoofed_lines(){
        assert!(exact_marker(b"diagnostics\nzorde generated registry guest smoke passed\n"));
        for b in [b"prefix zorde generated registry guest smoke passed\n".as_slice(),
            b"zorde generated registry guest smoke passed\r\n",b"zorde generated registry guest smoke passed\nzorde generated registry guest smoke passed\n",
            b"\x00\nzorde generated registry guest smoke passed\n",b"\xff\nzorde generated registry guest smoke passed\n",b""]{assert!(!exact_marker(b));}
    }
}
