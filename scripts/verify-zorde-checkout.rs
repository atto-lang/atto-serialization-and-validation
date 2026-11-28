//! Linux host source integrity candidate; not native guest evidence.
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
const MAX_FILES:usize=10_000;
const MAX_BYTES:u64=512*1024*1024;
const MAX_PATH:usize=4096;
// Linux O_NOFOLLOW | O_NONBLOCK: refuse leaf symlink and FIFO substitution.
const OPEN_FLAGS:i32=0x20000|0x800;
type Result<T>=std::result::Result<T,String>;
fn require(ok:bool,why:&str)->Result<()> {if ok{Ok(())}else{Err(why.into())}}
fn io<T>(r:std::io::Result<T>)->Result<T>{r.map_err(|e|e.to_string())}
fn sha(s:&[u8])->bool{s.len()==40&&s.iter().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(b))}
fn git(root:&Path)->Command{
    let mut c=Command::new("/usr/bin/git");
    c.current_dir(root).env_clear().env("PATH","/usr/bin:/bin").env("LC_ALL","C")
        .env("GIT_CONFIG_NOSYSTEM","1").env("GIT_CONFIG_GLOBAL","/dev/null")
        .env("GIT_NO_REPLACE_OBJECTS","1").stderr(Stdio::null());c
}
fn git_bytes(root:&Path,args:&[&str],cap:u64)->Result<Vec<u8>>{
    let mut c=io(git(root).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).spawn())?;
    let mut out=Vec::new();
    let r=c.stdout.take().ok_or("missing Git stdout")
        .and_then(|p|p.take(cap+1).read_to_end(&mut out).map_err(|_|"Git read failure"));
    if r.is_err()||out.len() as u64>cap{let _=c.kill();let _=c.wait();return Err("Git output budget or read failure".into());}
    require(io(c.wait())?.success(),"Git command failed")?;Ok(out)
}
fn identity(root:&Path,commit:&str,tree:&str)->Result<()>{
    require(sha(commit.as_bytes())&&sha(tree.as_bytes()),"invalid SHA-1 identity")?;
    let mut path=root.as_os_str().as_bytes().to_vec();path.push(b'\n');
    require(git_bytes(root,&["rev-parse","--show-toplevel"],4097)?==path,"checkout root mismatch")?;
    require(git_bytes(root,&["rev-parse","--show-object-format"],32)?==b"sha1\n","unsupported object format")?;
    require(git_bytes(root,&["rev-parse","HEAD"],64)?==format!("{commit}\n").as_bytes(),"HEAD does not equal reviewed commit")?;
    require(git_bytes(root,&["rev-parse","HEAD^{tree}"],64)?==format!("{tree}\n").as_bytes(),"root tree does not equal reviewed tree")
}
fn valid_path(raw:&[u8])->bool{
    !raw.is_empty()&&raw.len()<=MAX_PATH&&!raw.contains(&0)&&raw.split(|b|*b==b'/')
        .all(|p|!p.is_empty()&&p!=b"."&&p!=b".."&&p!=b".git")
}
struct Entry{path:PathBuf,executable:bool,oid:Vec<u8>}
fn entries(raw:&[u8])->Result<Vec<Entry>>{
    require(raw.ends_with(b"\0"),"malformed or empty Git tree")?;
    let mut result=Vec::new();let mut names=BTreeSet::new();
    for entry in raw[..raw.len()-1].split(|b|*b==0){
        require(result.len()<MAX_FILES,"file-count budget exceeded")?;
        let tab=entry.iter().position(|b|*b==b'\t').ok_or("malformed tree entry")?;
        let parts:Vec<_>=entry[..tab].split(|b|*b==b' ').collect();let name=&entry[tab+1..];
        require(parts.len()==3&&parts[1]==b"blob"&&sha(parts[2]),"unattested Git object or submodule")?;
        require(parts[0]==b"100644"||parts[0]==b"100755","unsupported tracked mode; symlink refused")?;
        require(valid_path(name),"invalid tree path")?;
        let path=PathBuf::from(OsStr::from_bytes(name));
        require(names.insert(path.clone()),"duplicate tree path")?;
        result.push(Entry{path,executable:parts[0]==b"100755",oid:parts[2].to_vec()});
    }
    require(!result.is_empty(),"empty checkout")?;Ok(result)
}
fn parents(root:&Path,rel:&Path)->Result<()>{
    for p in rel.ancestors().skip(1){
        if p.as_os_str().is_empty(){break;}
        require(io(fs::symlink_metadata(root.join(p)))?.is_dir(),"tracked parent directory is not a real directory")?;
    }Ok(())
}
fn physical(root:&Path,files:&BTreeSet<PathBuf>,dirs:&BTreeSet<PathBuf>)->Result<()>{
    let mut stack=vec![PathBuf::new()];let mut seen=BTreeSet::new();let mut count=0;
    while let Some(rel)=stack.pop(){
        for item in io(fs::read_dir(root.join(&rel)))?{
            let item=io(item)?;count+=1;require(count<=50_000,"physical entry budget exceeded")?;
            let name=item.file_name();let ty=io(item.file_type())?;
            if rel.as_os_str().is_empty()&&name==OsStr::new(".git"){
                require(ty.is_dir(),"Git metadata must be a real directory")?;continue;
            }
            let path=rel.join(name);require(valid_path(path.as_os_str().as_bytes()),"invalid physical path")?;
            if ty.is_dir(){require(dirs.contains(&path),"unexpected physical directory")?;stack.push(path);}
            else{require(ty.is_file()&&files.contains(&path),"unexpected physical file or nonregular object")?;
                require(seen.insert(path),"duplicate physical path")?;}
        }
    }require(&seen==files,"tracked file absent")
}
fn stamp(m:&Metadata)->(u64,u64,u64,u32,i64,i64,i64,i64){
    (m.dev(),m.ino(),m.len(),m.mode(),m.mtime(),m.mtime_nsec(),m.ctime(),m.ctime_nsec())
}
fn hash_file(root:&Path,e:&Entry,total:&mut u64)->Result<()>{
    parents(root,&e.path)?;let path=root.join(&e.path);
    let before=io(fs::symlink_metadata(&path))?;
    require(before.is_file(),"tracked regular file changed type")?;
    require((before.mode()&0o111!=0)==e.executable,"tracked executable mode changed")?;
    *total=total.checked_add(before.len()).ok_or("byte budget overflow")?;
    require(*total<=MAX_BYTES,"tracked byte budget exceeded")?;
    let mut file=io(OpenOptions::new().read(true).custom_flags(OPEN_FLAGS).open(&path))?;
    let opened=io(file.metadata())?;
    require(opened.is_file()&&stamp(&opened)==stamp(&before),"tracked file changed during open")?;
    // Hash raw bytes through Git's blob algorithm, no filters or object writes.
    let mut c=io(git(root).args(["hash-object","--stdin","--no-filters"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn())?;
    let streamed=(||->Result<()>{
        let mut stdin=c.stdin.take().ok_or("missing hash stdin")?;
        let n=io(std::io::copy(&mut (&mut file).take(opened.len()+1),&mut stdin))?;
        io(stdin.flush())?;require(n==opened.len(),"tracked file size changed during hashing")
    })();
    if let Err(err)=streamed{let _=c.kill();let _=c.wait();return Err(err);}
    let out=io(c.wait_with_output())?;require(out.status.success(),"Git blob hash failed")?;
    let mut expected=e.oid.clone();expected.push(b'\n');
    require(out.stdout==expected,"working-tree blob differs from reviewed Git tree")?;
    require(stamp(&io(file.metadata())?)==stamp(&opened)&&stamp(&io(fs::symlink_metadata(&path))?)==stamp(&opened),
        "tracked file changed during hashing")?;parents(root,&e.path)
}
fn verify(root:&Path,commit:&str,tree:&str)->Result<usize>{
    require(io(fs::symlink_metadata(root.join(".git")))?.is_dir(),"Git metadata must be a real directory")?;
    identity(root,commit,tree)?;
    let list=git_bytes(root,&["ls-tree","-rz","--full-tree",tree],16*1024*1024)?;
    let entries=entries(&list)?;let files:BTreeSet<_>=entries.iter().map(|e|e.path.clone()).collect();
    let mut dirs=BTreeSet::new();
    for e in &entries{parents(root,&e.path)?;for p in e.path.ancestors().skip(1){
        if !p.as_os_str().is_empty(){dirs.insert(p.to_path_buf());}
    }}
    physical(root,&files,&dirs)?;let mut total=0;
    for e in &entries{hash_file(root,e,&mut total)?;}
    physical(root,&files,&dirs)?;identity(root,commit,tree)?;Ok(entries.len())
}
fn main(){
    let args:Vec<_>=std::env::args().collect();
    if args.len()!=3{eprintln!("usage: verify-zorde-checkout COMMIT TREE");std::process::exit(2);}
    match io(std::env::current_dir()).and_then(|root|verify(&root,&args[1],&args[2])){
        Ok(n)=>println!("Zorde source-byte integrity: PASS ({n} exact Git blobs; no compiler executed)"),
        Err(e)=>{eprintln!("Zorde source-byte integrity refused: {e}");std::process::exit(2);}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn tree_paths_and_modes(){
        let oid="a".repeat(40);let entry=format!("100644 blob {oid}\tspace name/file\0");
        assert_eq!(entries(entry.as_bytes()).unwrap().len(),1);
        for name in ["","/abs","x/../y","x//y","x/./y",".git/config","x/.git/y"]{
            assert!(entries(format!("100644 blob {oid}\t{name}\0").as_bytes()).is_err());
        }
        for mode in ["120000","160000","100600"]{
            assert!(entries(format!("{mode} blob {oid}\tx\0").as_bytes()).is_err());
        }
        assert!(entries(format!("{entry}{entry}").as_bytes()).is_err());
        assert!(entries(entry.trim_end_matches('\0').as_bytes()).is_err());
    }
    #[test]fn exact_lowercase_sha1(){
        assert!(sha(&[b'a';40]));
        for bad in [&[b'A';40][..],&[b'a';39],&[b'a';41],&[b'g';40]]{assert!(!sha(bad));}
    }
}
