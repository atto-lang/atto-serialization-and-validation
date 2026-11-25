//! Offline structural receipt verification only. Independently authenticate
//! actual GitHub jobs, executed steps and artifacts before claiming guest proof.
use crate::Unique;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap,BTreeSet};
use std::fs::{self,OpenOptions};
use std::io::{Read,Write};
use std::os::unix::fs::{MetadataExt,OpenOptionsExt};
use std::path::{Component,Path,PathBuf};
use std::process::{Command,Stdio};
pub const BACKENDS:[&str;5]=["java-graalvm","llvm","wasm","cranelift-independent","dotnet-nativeaot"];
pub const MANIFEST:&[u8]=include_bytes!("../fixtures/serde/adversarial-v1.json");
const MAX_JSON:usize=512*1024;
type Result<T>=std::result::Result<T,String>;
fn require(ok:bool,why:&str)->Result<()> {if ok{Ok(())}else{Err(why.into())}}
fn io<T>(r:std::io::Result<T>)->Result<T>{r.map_err(|e|e.to_string())}
fn hex(s:&str,n:usize)->bool{s.len()==n&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn object(raw:&[u8])->Result<Value>{
    require(raw.len()<=MAX_JSON,"JSON exceeds 512 KiB")?;
    let mut d=serde_json::Deserializer::from_slice(raw);
    let v=Unique::deserialize(&mut d).map_err(|e|e.to_string())?.0;
    d.end().map_err(|e|e.to_string())?;
    require(v.is_object(),"expected JSON object")?;Ok(v)
}
fn str_field<'a>(v:&'a Value,key:&str)->Result<&'a str>{
    v.get(key).and_then(Value::as_str).filter(|s|!s.trim().is_empty()).ok_or_else(||format!("missing/invalid {key}"))
}
fn positive(v:&Value,key:&str)->Result<u64>{
    v.get(key).and_then(Value::as_u64).filter(|n|*n>0).ok_or_else(||format!("missing/nonpositive {key}"))
}
fn match_str(v:&Value,key:&str,want:&str)->Result<()>{
    require(str_field(v,key)?==want,&format!("{key} mismatch"))
}
fn digest(v:&Value,key:&str)->Result<()>{
    require(hex(str_field(v,key)?,64),&format!("invalid {key} digest"))
}
/// Linux host-only trusted system SHA-256, no shell and no network.
pub fn sha256(raw:&[u8])->Result<String>{
    sha256_bounded(raw,MAX_JSON)
}

pub(crate) fn sha256_bounded(raw:&[u8],limit:usize)->Result<String>{
    require(raw.len()<=limit,"digest input exceeds bound")?;
    let mut c=io(Command::new("/usr/bin/sha256sum").env_clear().env("LC_ALL","C")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn())?;
    let r=c.stdin.take().ok_or("missing digest stdin")
        .and_then(|mut p|p.write_all(raw).map_err(|_|"digest write failed"));
    if let Err(e)=r{let _=c.kill();let _=c.wait();return Err(e.into());}
    let out=io(c.wait_with_output())?;require(out.status.success(),"digest command failed")?;
    let text=std::str::from_utf8(&out.stdout).map_err(|e|e.to_string())?;
    let h=text.strip_suffix("  -\n").ok_or("malformed digest output")?;
    require(hex(h,64),"malformed digest output")?;Ok(h.into())
}
fn unhex(s:&str)->Result<Vec<u8>>{
    require(s.len()%2==0&&s.len()<=MAX_JSON,"invalid fixture bytes")?;
    fn digit(b:u8)->Option<u8>{match b{b'0'..=b'9'=>Some(b-b'0'),b'a'..=b'f'=>Some(b-b'a'+10),b'A'..=b'F'=>Some(b-b'A'+10),_=>None}}
    s.as_bytes().chunks_exact(2).map(|p|Ok((digit(p[0]).ok_or("invalid hex")?<<4)|digit(p[1]).ok_or("invalid hex")?)).collect()
}
struct Expected{digest:String,result:String,error:Value,decoded:Value}
fn expected()->Result<(String,BTreeMap<String,Expected>)>{
    let m=object(MANIFEST)?;
    match_str(&m,"schema","attolang.serde.adversarial-v1")?;
    let rows=m["cases"].as_array().ok_or("manifest cases absent")?;
    require(!rows.is_empty()&&rows.len()<=1024,"invalid fixture count")?;
    let mut map=BTreeMap::new();
    for row in rows{
        let id=str_field(row,"id")?.to_owned();
        let result=str_field(row,"expected_result")?.to_owned();
        require(matches!(result.as_str(),"Ok"|"Err"),"invalid expected result")?;
        if result=="Err"{str_field(row,"expected_error_code")?;}
        else{require(!row["expected_decoded"].is_null(),"positive fixture missing decoded value")?;}
        let item=Expected{digest:sha256(&unhex(str_field(row,"bytes_hex")?)?)?,result,
            error:row["expected_error_code"].clone(),decoded:row["expected_decoded"].clone()};
        require(map.insert(id,item).is_none(),"duplicate manifest fixture ID")?;
    }
    Ok((sha256(MANIFEST)?,map))
}
fn url_token(s:&str)->bool{
    !s.is_empty()&&s!="."&&s!=".."&&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'.'|b'-'))
}
fn urls(v:&Value,run:u64,job:u64)->Result<()>{
    let url=str_field(v,"run_url")?;
    let rest=url.strip_prefix("https://github.com/").ok_or("noncanonical GitHub run URL")?;
    let p:Vec<_>=rest.split('/').collect();
    require(p.len()==5&&url_token(p[0])&&url_token(p[1])&&p[2]=="actions"&&p[3]=="runs"
        &&p[4]==run.to_string(),"run URL/ID mismatch")?;
    match_str(v,"job_url",&format!("{url}/job/{job}"))
}
fn receipt(v:&Value,backend:&str,head:&str,tree:&str,manifest:&str,expected:&BTreeMap<String,Expected>,jobs:&mut BTreeSet<u64>)->Result<()>{
    for (k,w) in [("backend",backend),("status","PASS"),("source_head_sha",head),("source_tree_sha",tree),
        ("fixture_manifest_sha256",manifest),("guest_artifact_kind",if backend=="wasm"{"wasm-module"}else{"native-executable"})]{
        match_str(v,k,w)?;
    }
    for k in ["compiler_sha256","guest_artifact_sha256","stdout_sha256","stderr_sha256"]{digest(v,k)?;}
    let run=positive(v,"run_id")?;let job=positive(v,"job_id")?;positive(v,"executed_step_count")?;
    require(jobs.insert(job),"job ID reused by another backend")?;
    for k in ["compiler_version","target","runner_os","compile_command","execute_command"]{str_field(v,k)?;}
    urls(v,run,job)?;
    require(v["guest_exit_code"].as_u64()==Some(0),"missing/nonzero guest exit")?;
    require(v["compiled_native_guest"].as_bool()==Some(true)&&v["executed_native_guest"].as_bool()==Some(true)
        &&v["host_only"].as_bool()==Some(false),"missing native execution or host-only")?;
    if backend=="cranelift-independent"{require(v["independent_cranelift"].as_bool()==Some(true),"independent Cranelift absent")?;}
    if backend!="wasm"{require(v["full_aot"].as_bool()==Some(true),"native AOT evidence absent")?;}
    let mem=&v["memory"];require(mem.is_object(),"missing memory object")?;
    let cap=positive(mem,"allocation_limit_bytes")?;
    require(mem["peak_alloc_bytes"].as_u64().is_some_and(|n|n<=cap),"peak allocation outside limit")?;
    require(mem["max_depth"].as_u64().is_some_and(|n|n<=128),"invalid depth")?;
    let inst=&v["instrumentation"];require(inst.is_object(),"missing instrumentation object")?;
    match_str(inst,"result","PASS")?;str_field(inst,"kind")?;digest(inst,"evidence_sha256")?;
    let rows=v["cases"].as_array().ok_or("missing case list")?;
    require(rows.len()==expected.len(),"incomplete cases")?;
    let mut seen=BTreeSet::new();
    for row in rows{
        require(row.is_object(),"invalid case object")?;
        let id=str_field(row,"id")?;
        let e=expected.get(id).ok_or("unknown fixture ID")?;
        require(seen.insert(id),"duplicate fixture ID")?;
        match_str(row,"input_sha256",&e.digest)?;match_str(row,"result",&e.result)?;
        if e.result=="Err"{
            require(row["error_code"]==e.error,"error classification mismatch")?;
            require(row["decoded_value"].is_null(),"Err must not publish decoded value")?;
        }else{
            require(row["error_code"].is_null()||row["error_code"].as_str()==Some(""),"Ok contains error")?;
            require(row.get("decoded_value")==Some(&e.decoded),"positive decoded value mismatch")?;
        }
    }
    Ok(())
}
fn directory(path:&Path)->Result<PathBuf>{
    let full=if path.is_absolute(){path.to_path_buf()}else{io(std::env::current_dir())?.join(path)};
    let mut prefix=PathBuf::new();
    for c in full.components(){
        require(!matches!(c,Component::ParentDir),"receipt directory traversal refused")?;
        prefix.push(c.as_os_str());
        require(io(fs::symlink_metadata(&prefix))?.is_dir(),"receipt directory missing or symlink")?;
    }
    Ok(full)
}
fn stamp(m:&fs::Metadata)->(u64,u64,u64,i64,i64,i64,i64){
    (m.dev(),m.ino(),m.len(),m.mtime(),m.mtime_nsec(),m.ctime(),m.ctime_nsec())
}
fn read_receipt(path:&Path)->Result<Vec<u8>>{
    let before=io(fs::symlink_metadata(path))?;
    require(before.is_file()&&before.len()<=MAX_JSON as u64,"unsafe or oversized receipt")?;
    // Linux O_NOFOLLOW|O_NONBLOCK, and metadata rechecks. Trusted isolated
    // directory remains required; this is not a hostile same-UID snapshot.
    let mut f=io(OpenOptions::new().read(true).custom_flags(0x20000|0x800).open(path))?;
    let opened=io(f.metadata())?;
    require(opened.is_file()&&stamp(&before)==stamp(&opened),"receipt changed during open")?;
    let mut raw=Vec::new();
    io((&mut f).take(MAX_JSON as u64+1).read_to_end(&mut raw))?;
    require(raw.len()<=MAX_JSON&&raw.len() as u64==opened.len()
        &&stamp(&io(f.metadata())?)==stamp(&opened)
        &&stamp(&io(fs::symlink_metadata(path))?)==stamp(&opened),"receipt changed or exceeded bound")?;
    Ok(raw)
}
/// Accept structure only. Synthetic data can pass. Never use this as evidence
/// of execution, authentication, provenance or release approval.
pub fn verify(dir:&Path,head:&str,tree:&str)->Result<()>{
    require(hex(head,40)&&hex(tree,40),"invalid expected source identity")?;
    let dir=directory(dir)?;
    let names:BTreeSet<_>=BACKENDS.iter().map(|b|format!("{b}.json")).collect();
    let mut actual=BTreeSet::new();
    for item in io(fs::read_dir(&dir))?{
        let item=io(item)?;
        let name=item.file_name().into_string().map_err(|_|"non-UTF-8 receipt name")?;
        require(names.contains(&name)&&actual.insert(name),"unexpected receipt directory entry")?;
        require(io(item.file_type())?.is_file(),"unsafe receipt entry")?;
    }
    require(actual==names,"missing backend receipt")?;
    let (manifest,expected)=expected()?;let mut jobs=BTreeSet::new();
    for backend in BACKENDS{
        let v=object(&read_receipt(&dir.join(format!("{backend}.json")))?)?;
        receipt(&v,backend,head,tree,&manifest,&expected,&mut jobs).map_err(|e|format!("{backend}: {e}"))?;
    }
    Ok(())
}
