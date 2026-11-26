//! Linux x86_64 host tooling. Does not attest compiler identity or guest proof.
//! Launch this binary from a trusted clean parent: LD_PRELOAD can execute
//! before main. Exclusive source/evidence directories remain required.
#[cfg(not(all(target_os="linux",target_arch="x86_64")))]
compile_error!("reviewed Linux x86_64 syscall ABI required");
use std::ffi::{CString,OsStr,OsString};
use std::fs::{self,File};
use std::io::{Read,Write};
use std::os::fd::{AsRawFd,FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command,Stdio};
type Result<T>=std::result::Result<T,String>;
const DIRECTORY:i32=0x10000;
const NOFOLLOW:i32=0x20000;
const CLOEXEC:i32=0x80000;
const NONBLOCK:i32=0x800;
const CREATE:i32=0x40;
const EXCLUSIVE:i32=0x80;
const WRONLY:i32=1;
const LOG_LIMIT:u64=16*1024*1024;
const LOG_NAME:&str="zorde-generated-smoke-native.log";
const CONTRACT:[&str;6]=["EXPECTED_SOURCE_SHA","EXPECTED_SOURCE_TREE","EXPECTED_NODE_SHA256","NODE_BIN","TJSV_ROOT","ZORDE_EVIDENCE_DIR"];
const GUEST:[&str;10]=["EXPECTED_SOURCE_SHA","EXPECTED_SOURCE_TREE","EXPECTED_COMPILER_SHA256","EXPECTED_CONTRACT_IR_SHA256",
"EXPECTED_PARITY_REPORT_SHA256","EXPECTED_VERIFICATION_SHA256","ORESLANG_BIN","NODE_BIN","EXPECTED_NODE_SHA256","ZORDE_EVIDENCE_DIR"];
#[repr(C)]
struct Rlimit{cur:u64,max:u64}
extern "C" {
    fn openat(fd:i32,path:*const std::os::raw::c_char,flags:i32,...)->i32;
    fn geteuid()->u32;
    fn setrlimit(resource:i32,limit:*const Rlimit)->i32;
}
fn io<T>(r:std::io::Result<T>)->Result<T>{r.map_err(|e|e.to_string())}
fn need(ok:bool,why:&str)->Result<()> {if ok{Ok(())}else{Err(why.into())}}
fn uid()->u32{unsafe{geteuid()}}
fn open_child(fd:i32,name:&OsStr,flags:i32,mode:u32)->Result<File>{
    let c=CString::new(name.as_bytes()).map_err(|_|"NUL path")?;
    let raw=unsafe{openat(fd,c.as_ptr(),flags|CLOEXEC,mode)};
    if raw<0{return Err(std::io::Error::last_os_error().to_string());}
    // SAFETY: successful openat returns a new owned descriptor.
    Ok(unsafe{File::from_raw_fd(raw)})
}
fn parts(p:&Path)->Result<Vec<&OsStr>>{
    let bytes=p.as_os_str().as_bytes();
    need(bytes.starts_with(b"/")&&bytes.len()<=4096,"absolute bounded path required")?;
    if bytes==b"/"{return Ok(vec![]);}
    let mut result=Vec::new();
    for b in bytes[1..].split(|b|*b==b'/'){
        need(!b.is_empty()&&b!=b"."&&b!=b"..","ambiguous path component")?;
        result.push(OsStr::from_bytes(b));
    }
    Ok(result)
}
fn dir(p:&Path)->Result<File>{
    let mut d=open_child(-100,OsStr::new("/"),DIRECTORY|NOFOLLOW,0)?;
    for name in parts(p)? {d=open_child(d.as_raw_fd(),name,DIRECTORY|NOFOLLOW,0)?;}
    Ok(d)
}
fn leaf(p:&Path)->Result<File>{
    parts(p)?;
    let parent=p.parent().ok_or("missing parent")?;
    let name=p.file_name().ok_or("missing leaf")?;
    let d=dir(parent)?;
    let f=open_child(d.as_raw_fd(),name,NOFOLLOW|NONBLOCK,0)?;
    let m=io(f.metadata())?;
    need(m.is_file()&&m.nlink()==1,"unique regular file required")?;
    Ok(f)
}
fn stamp(m:&fs::Metadata)->(u64,u64,u64,u64,u32,u32,i64,i64,i64,i64){
    (m.dev(),m.ino(),m.len(),m.nlink(),m.uid(),m.mode(),m.mtime(),m.mtime_nsec(),m.ctime(),m.ctime_nsec())
}
fn private(m:&fs::Metadata)->bool{m.uid()==uid()&&m.mode()&0o077==0}
fn clean(program:&str)->Command{
    let mut c=Command::new(program);
    c.env_clear().env("PATH","/usr/bin:/bin").env("LANG","C.UTF-8").env("LC_ALL","C.UTF-8");c
}
fn hex(s:&str,n:usize)->bool{s.len()==n&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn digest(raw:&[u8])->Result<String>{
    let mut c=io(clean("/usr/bin/sha256sum").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn())?;
    let r=c.stdin.take().ok_or("missing digest input").and_then(|mut p|p.write_all(raw).map_err(|_|"digest write failed"));
    if let Err(e)=r{let _=c.kill();let _=c.wait();return Err(e.into());}
    let out=io(c.wait_with_output())?;need(out.status.success(),"SHA256 utility failed")?;
    let s=std::str::from_utf8(&out.stdout).map_err(|_|"invalid digest output")?;
    let h=s.strip_suffix("  -\n").ok_or("invalid digest output")?;
    need(hex(h,64),"invalid SHA256 output")?;Ok(h.into())
}
fn gate_bytes(p:&Path)->Result<Vec<u8>>{
    let mut f=leaf(p)?;let before=io(f.metadata())?;
    need(before.len()>0&&before.len()<=1024*1024,"gate size outside bounds")?;
    let mut raw=Vec::new();io((&mut f).take(1024*1024+1).read_to_end(&mut raw))?;
    need(raw.len() as u64==before.len()&&stamp(&io(f.metadata())?)==stamp(&before)
        &&stamp(&io(leaf(p)?.metadata())?)==stamp(&before),"gate changed during read")?;
    Ok(raw)
}
fn bootstrap(mode:&str,root:&Path,approved:&str)->Result<()>{
    let (gate,required)=match mode{
        "contract"=>("test-zorde-contract-source-local.sh",CONTRACT.as_slice()),
        "guest"=>("test-zorde-generated-guest-source-local.sh",GUEST.as_slice()),
        _=>return Err("unknown bootstrap mode".into())
    };
    need(hex(approved,64),"invalid approved gate digest")?;
    dir(root)?;
    let script=root.join("scripts").join(gate);
    let mut command=clean("/usr/bin/bash");
    for &key in required{
        let v=std::env::var(key).map_err(|_|format!("missing approved input: {key}"))?;
        need(!v.is_empty()&&v.len()<=4096&&!v.chars().any(char::is_control),&format!("invalid approved input: {key}"))?;
        if key.starts_with("EXPECTED_"){
            let n=if key=="EXPECTED_SOURCE_SHA"||key=="EXPECTED_SOURCE_TREE"{40}else{64};
            need(hex(&v,n),&format!("invalid approved digest: {key}"))?;
        }
        command.env(key,v);
    }
    // The launcher binary and digest must be provisioned independently.
    // Source identity and other approval gates are still enforced by Bash.
    need(digest(&gate_bytes(&script)?)?==approved,"gate digest mismatch")?;
    let status=io(command.current_dir(root).args(["--noprofile","--norc"]).arg(&script).status())?;
    need(status.success(),"source-local gate refused or failed")
}
fn capture(directory:&Path,compiler:&Path,guest:&Path,seconds:u32)->Result<()>{
    need((1..=180).contains(&seconds),"invalid timeout")?;
    let d=dir(directory)?;let before_dir=io(d.metadata())?;
    need(private(&before_dir),"evidence directory must be owned and private")?;
    let c=leaf(compiler)?;let g=leaf(guest)?;
    let cm=io(c.metadata())?;let gm=io(g.metadata())?;
    need(cm.mode()&0o111!=0,"compiler not executable")?;
    let out=open_child(d.as_raw_fd(),OsStr::new(LOG_NAME),WRONLY|CREATE|EXCLUSIVE|NOFOLLOW|NONBLOCK,0o600)?;
    let before=io(out.metadata())?;
    need(before.is_file()&&before.nlink()==1&&private(&before),"unsafe created log")?;
    let mut command=clean("/usr/bin/timeout");
    command.args(["--signal=TERM","--kill-after=5s"]).arg(format!("{seconds}s"))
        .arg(compiler).arg(guest).stdin(Stdio::null())
        .stdout(Stdio::from(io(out.try_clone())?)).stderr(Stdio::from(io(out.try_clone())?));
    // SAFETY: pre_exec invokes only the async-signal-safe Linux setrlimit
    // syscall wrapper, with stack-local C layout and no environment changes.
    unsafe{command.pre_exec(||{
        let limit=Rlimit{cur:LOG_LIMIT,max:LOG_LIMIT};
        if setrlimit(1,&limit)!=0{return Err(std::io::Error::last_os_error());}
        Ok(())
    });}
    let status=io(command.status())?;
    let after=io(out.metadata())?;
    let current_dir=io(dir(directory)?.metadata())?;
    let current=io(open_child(d.as_raw_fd(),OsStr::new(LOG_NAME),NOFOLLOW|NONBLOCK,0)?.metadata())?;
    need(status.success(),"timed compiler/guest failed")?;
    // Exact saturation is refused, even if a child caught SIGXFSZ or ignored
    // a write error and returned zero. No receipt is issued by this utility.
    need(after.is_file()&&private(&after)&&after.nlink()==1&&after.len()>0&&after.len()<LOG_LIMIT
        &&before.dev()==after.dev()&&before.ino()==after.ino()
        &&stamp(&after)==stamp(&current),"native log changed, empty or saturated")?;
    need(current_dir.dev()==before_dir.dev()&&current_dir.ino()==before_dir.ino()&&private(&current_dir),"evidence directory changed")?;
    need(stamp(&io(leaf(compiler)?.metadata())?)==stamp(&cm)&&stamp(&io(leaf(guest)?.metadata())?)==stamp(&gm),
        "compiler or guest changed during execution")?;
    io(out.sync_all())?;
    Ok(())
}
fn main(){
    let a:Vec<OsString>=std::env::args_os().collect();
    let r=match a.as_slice(){
        [_,cmd,mode,root,hash] if cmd=="bootstrap"=>{
            match (mode.to_str(),hash.to_str()){(Some(m),Some(h))=>bootstrap(m,Path::new(root),h),_=>Err("invalid bootstrap arguments".into())}
        }
        [_,cmd,directory,compiler,guest,seconds] if cmd=="capture"=>{
            match seconds.to_str().and_then(|s|s.parse::<u32>().ok()){
                Some(n)=>capture(Path::new(directory),Path::new(compiler),Path::new(guest),n),
                None=>Err("invalid timeout".into())
            }
        }
        _=>Err("usage: zorde-isolated bootstrap contract|guest ROOT GATE_SHA256 | capture DIR COMPILER GUEST SECONDS".into())
    };
    if let Err(e)=r{eprintln!("ZORDE_ISOLATED_REFUSED: {e}");std::process::exit(2);}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn absolute_paths_no_normalization(){
        for s in ["relative","/a/../b","/a/./b","/a//b","/a/"]{assert!(parts(Path::new(s)).is_err(),"{s}");}
        assert_eq!(parts(Path::new("/a/b")).unwrap(),vec![OsStr::new("a"),OsStr::new("b")]);
    }
    #[test]fn golden_digest(){
        assert_eq!(digest(b"abc").unwrap(),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
