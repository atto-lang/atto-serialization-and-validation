//! Source-local native input manifest candidate. No guest execution proof.
//! Restricted line-oriented import grammar, not a compiler parser/closure proof.
use crate::{Unique,serde_receipts::sha256_bounded};
use serde::Deserialize;
use serde_json::{json,Value};
use std::collections::{BTreeMap,BTreeSet,VecDeque};
use std::fs::{self,OpenOptions};
use std::io::{Read,Write};
use std::os::unix::fs::{MetadataExt,OpenOptionsExt};
use std::path::{Component,Path,PathBuf};

type Result<T>=std::result::Result<T,String>;
const LIMIT:usize=8*1024*1024;
const TOTAL:usize=128*1024*1024;
const SCHEMA:&str="oreslang.zorde.native-guest-inputs/v1";
pub const FIXED:[&str;10]=[
    "contracts/zorde/scalars.tsp","contracts/zorde/scalars.schema.json",
    "scripts/zorde-static-contract-ir.mjs","artifacts/zorde-scalars-parity.json",
    "artifacts/zorde-scalars-contract-ir.json","artifacts/zorde-scalars-consumer-verification.json",
    "artifacts/zorde-generated/build-complete.json","artifacts/zorde-generated/registry-plan.json",
    "artifacts/zorde-generated/registry-specs.ores","artifacts/zorde-generated/registry-smoke.ores"];
const ROOTS:[&str;2]=[FIXED[8],FIXED[9]];
const CORPUS:&str="contracts/zorde/scalar-instances";
extern "C"{fn geteuid()->u32;}
fn uid()->u32{unsafe{geteuid()}}
fn need(ok:bool,why:&str)->Result<()> {if ok{Ok(())}else{Err(why.into())}}
fn io<T>(r:std::io::Result<T>)->Result<T>{r.map_err(|e|e.to_string())}
fn dirs(p:&Path)->Result<()>{
    need(p.is_absolute(),"absolute directory required")?;
    let mut q=PathBuf::new();
    for c in p.components(){
        need(!matches!(c,Component::ParentDir),"directory traversal")?;
        q.push(c.as_os_str());need(io(fs::symlink_metadata(&q))?.is_dir(),"unsafe directory ancestor")?;
    }Ok(())
}
fn stamp(m:&fs::Metadata)->(u64,u64,u64,u64,u32,u32,i64,i64,i64,i64){
    (m.dev(),m.ino(),m.len(),m.nlink(),m.uid(),m.mode(),m.mtime(),m.mtime_nsec(),m.ctime(),m.ctime_nsec())
}
fn read(p:&Path,limit:usize)->Result<Vec<u8>>{
    dirs(p.parent().ok_or("missing parent")?)?;
    let before=io(fs::symlink_metadata(p))?;
    need(before.is_file()&&before.nlink()==1&&before.uid()==uid()&&before.len()>0&&before.len()<=limit as u64,"unsafe or oversized input")?;
    let mut f=io(OpenOptions::new().read(true).custom_flags(0x20000|0x800).open(p))?;
    let opened=io(f.metadata())?;need(stamp(&before)==stamp(&opened),"input changed during open")?;
    let mut b=Vec::new();io((&mut f).take(limit as u64+1).read_to_end(&mut b))?;
    need(b.len()<=limit&&b.len() as u64==opened.len()&&stamp(&io(f.metadata())?)==stamp(&opened)
        &&stamp(&io(fs::symlink_metadata(p))?)==stamp(&opened),"input changed during read")?;
    dirs(p.parent().ok_or("missing parent")?)?;Ok(b)
}
fn names(p:&Path,cap:usize)->Result<BTreeSet<String>>{
    dirs(p)?;let mut out=BTreeSet::new();
    for e in io(fs::read_dir(p))?{
        let n=io(e)?.file_name().into_string().map_err(|_|"non-UTF8 directory name")?;
        need(out.len()<cap&&out.insert(n),"directory entry budget")?;
    }Ok(out)
}
fn corpus(root:&Path)->Result<Vec<String>>{
    need(names(&root.join(CORPUS),2)?==BTreeSet::from(["Port".into(),"ShortLabel".into()]),"unexpected corpus declarations")?;
    let mut files=Vec::new();
    for decl in ["Port","ShortLabel"]{
        let base=format!("{CORPUS}/{decl}");
        need(names(&root.join(&base),2)?==BTreeSet::from(["invalid".into(),"valid".into()]),"unexpected corpus lanes")?;
        for lane in ["valid","invalid"]{
            let lane=format!("{base}/{lane}");
            let found=names(&root.join(&lane),64)?;
            need(!found.is_empty(),"empty corpus lane")?;
            for name in found{
                need(name.ends_with(".json")&&name.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
                    &&name.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.')),"invalid corpus name")?;
                files.push(format!("{lane}/{name}"));
            }
        }
    }
    need(files.len()==9,"expected nine corpus fixtures")?;Ok(files)
}
fn relative_target(root:&Path,source:&str,raw:&str)->Result<String>{
    need(raw.starts_with("./")||raw.starts_with("../"),"unsupported import target")?;
    need(raw.len()<=1024&&raw.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'/'|b'.'|b'_'|b'-')),"unsafe import spelling")?;
    let mut path=PathBuf::from(source);path.pop();
    let parts:Vec<_>=raw.split('/').collect();
    for (i,part) in parts.iter().enumerate(){
        need(!part.is_empty(),"empty import path component")?;
        if *part=="."{need(i+1<parts.len(),"invalid import leaf")?;}
        else if *part==".."{
            need(i+1<parts.len()&&path.pop(),"import escapes checkout")?;
        }else if i+1==parts.len(){
            path.push(format!("{part}.ores"));
        }else{
            path.push(part);dirs(&root.join(&path))?;
        }
    }
    let name=path.to_str().ok_or("non-UTF8 import path")?.to_owned();
    need(name.len()<=4096&&name.ends_with(".ores"),"invalid import path")?;Ok(name)
}
fn imports(root:&Path,source:&str,bytes:&[u8])->Result<Vec<String>>{
    let text=std::str::from_utf8(bytes).map_err(|_|"guest source is not UTF-8")?;
    need(!text.contains('\0'),"NUL in guest source")?;
    let mut out=Vec::new();
    for line in text.lines(){
        let t=line.trim();
        // Conservative: refuse any import token outside the supported full-line
        // grammar (even in a comment/string). Actual compiler grammar remains
        // a separate verification requirement.
        let has=t.split(|c:char|!(c.is_alphanumeric()||c=='_')).any(|s|s=="import");
        if !has{continue;}
        let rest=t.strip_prefix("import ").ok_or("unsupported import placement")?;
        let (subject,tail)=rest.rsplit_once(" from ").ok_or("unsupported static import syntax")?;
        need(!subject.trim().is_empty()&&subject.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b' '|b'\t'|b'_'|b','|b'{'|b'}')),"unsupported import subject")?;
        let quoted=tail.strip_suffix(';').ok_or("missing import terminator")?;
        let quote=quoted.as_bytes().first().copied().ok_or("empty import")?;
        need(matches!(quote,b'\''|b'"')&&quoted.len()>=3&&quoted.as_bytes().last()==Some(&quote),"invalid import quotes")?;
        out.push(relative_target(root,source,&quoted[1..quoted.len()-1])?);
        need(out.len()<=256,"per-module import budget exceeded")?;
    }Ok(out)
}
fn load(root:&Path,name:&str,files:&mut BTreeMap<String,String>,total:&mut usize)->Result<Vec<u8>>{
    need(!files.contains_key(name),"duplicate manifest input")?;
    let b=read(&root.join(name),LIMIT)?;
    *total=total.checked_add(b.len()).ok_or("aggregate input overflow")?;
    need(*total<=TOTAL,"aggregate input byte budget exceeded")?;
    files.insert(name.into(),sha256_bounded(&b,LIMIT)?);Ok(b)
}
/// Builds a manifest from actual bytes; each guest source is hashed and
/// scanned from the SAME bounded read. Breadth-first discovery bounds the
/// shortest import depth; repeated edges/cycles do not re-read nodes.
pub fn build(root:&Path)->Result<Value>{
    dirs(root)?;
    let corpus=corpus(root)?;
    let mut files=BTreeMap::new();let mut closure=BTreeSet::new();let mut total=0;
    for name in FIXED.into_iter().filter(|n|!ROOTS.contains(n)).chain(corpus.iter().map(String::as_str)){
        load(root,name,&mut files,&mut total)?;
    }
    let mut q:VecDeque<_>=ROOTS.into_iter().map(|s|(s.to_owned(),0usize)).collect();
    let mut scheduled:BTreeSet<_>=ROOTS.into_iter().map(str::to_owned).collect();
    while let Some((name,depth))=q.pop_front(){
        need(depth<=32,"guest import depth exceeded")?;
        let bytes=load(root,&name,&mut files,&mut total)?;
        for target in imports(root,&name,&bytes)?{
            if scheduled.insert(target.clone()){
                need(depth<32&&closure.len()<64,"guest import closure budget exceeded")?;
                need(!files.contains_key(&target),"guest dependency overlaps fixed input")?;
                closure.insert(target.clone());q.push_back((target,depth+1));
            }
        }
    }
    need((8..=64).contains(&closure.len())&&files.len()==19+closure.len(),"incomplete guest closure")?;
    Ok(json!({"schema":SCHEMA,"staticGuestImportClosure":closure,"files":files}))
}
fn evidence_path(root:&Path,p:&Path)->Result<()>{
    need(p.is_absolute()&&!p.starts_with(root),"manifest must be outside source checkout")?;
    need(p.components().all(|c|!matches!(c,Component::ParentDir)),"manifest traversal")?;
    let parent=p.parent().ok_or("missing output parent")?;
    dirs(parent)?;let m=io(fs::symlink_metadata(parent))?;
    need(m.uid()==uid()&&m.mode()&0o077==0,"evidence directory must be private and owned")?;
    Ok(())
}
pub fn capture(root:&Path,out:&Path)->Result<String>{
    evidence_path(root,out)?;
    let manifest=build(root)?;
    let mut bytes=serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?;bytes.push(b'\n');
    let digest=sha256_bounded(&bytes,1024*1024)?;
    let mut f=io(OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(0x20000|0x800).open(out))?;
    io(f.write_all(&bytes))?;io(f.sync_all())?;Ok(digest)
}
/// Compare the externally pinned original manifest AND re-derived closure,
/// corpus membership and hashes. No caller-supplied path is dereferenced
/// from JSON. Duplicate keys and trailing JSON are rejected.
pub fn recheck(root:&Path,out:&Path,digest:&str)->Result<()>{
    evidence_path(root,out)?;
    let raw=read(out,1024*1024)?;
    need(sha256_bounded(&raw,1024*1024)?==digest,"manifest digest mismatch")?;
    let mut d=serde_json::Deserializer::from_slice(&raw);
    let expected=Unique::deserialize(&mut d).map_err(|e|e.to_string())?.0;
    d.end().map_err(|e|e.to_string())?;
    need(build(root)?==expected,"guest input or import closure changed")
}
