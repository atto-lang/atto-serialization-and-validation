use atto_actor_contract_conformance::native_input_manifest::{build,capture,recheck,FIXED};
use std::fs;
use std::os::unix::fs::{DirBuilderExt,symlink};
use std::path::{Path,PathBuf};
use std::sync::atomic::{AtomicU64,Ordering};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp{
    fn new()->Self{
        let p=std::env::temp_dir().join(format!("zorde-input-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();Self(p)
    }
}
impl Drop for Temp{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
const G:&str="artifacts/zorde-generated";
fn put(root:&Path,name:&str,bytes:impl AsRef<[u8]>){
    let p=root.join(name);fs::create_dir_all(p.parent().unwrap()).unwrap();fs::write(p,bytes).unwrap();
}
fn fixture(t:&Temp)->PathBuf{
    let root=t.0.join("source");fs::create_dir(&root).unwrap();
    for n in FIXED{put(&root,n,b"synthetic\n");}
    for (decl,valid,invalid) in [("Port",3,2),("ShortLabel",2,2)]{
        for (lane,count) in [("valid",valid),("invalid",invalid)]{
            for n in 0..count{put(&root,&format!("contracts/zorde/scalar-instances/{decl}/{lane}/{n}.json"),b"0\n");}
        }
    }
    put(&root,&format!("{G}/registry-smoke.ores"),"import {D0} from './dep0';\n");
    for n in 0..8{
        let content=if n<7{format!("import class D from './dep{}';\n",n+1)}else{"synthetic leaf\n".into()};
        put(&root,&format!("{G}/dep{n}.ores"),content);
    }root
}
#[test]fn capture_and_recheck_closed_graph(){
    let t=Temp::new();let r=fixture(&t);let out=t.0.join("manifest.json");
    let m=build(&r).unwrap();assert_eq!(m["files"].as_object().unwrap().len(),27);
    assert_eq!(m["staticGuestImportClosure"].as_array().unwrap().len(),8);
    let hash=capture(&r,&out).unwrap();recheck(&r,&out,&hash).unwrap();
    assert!(capture(&r,&out).is_err());
    put(&r,&format!("{G}/dep7.ores"),"import class D from './dep0';\n");
    assert!(recheck(&r,&out,&hash).is_err());
    assert_eq!(build(&r).unwrap()["staticGuestImportClosure"].as_array().unwrap().len(),8);
}
#[test]fn unsafe_paths_and_inputs_are_refused(){
    for mode in 0..9{
        let t=Temp::new();let r=fixture(&t);let out=t.0.join("manifest.json");
        let dep=r.join(format!("{G}/dep4.ores"));
        match mode{
            0=>{fs::remove_file(&dep).unwrap();symlink(r.join(format!("{G}/dep5.ores")),&dep).unwrap();}
            1=>{fs::hard_link(r.join("scripts/zorde-static-contract-ir.mjs"),r.join("alias")).unwrap();}
            2=>{put(&r,&format!("{G}/dep7.ores"),vec![b'x';8*1024*1024+1]);}
            3=>{let p=r.join(G);let q=r.join("artifacts/staged");fs::rename(&p,&q).unwrap();symlink(q,p).unwrap();}
            4=>{put(&r,&format!("{G}/dep2.ores"),"import class D from '../../../../escape';\n");}
            5=>{symlink(t.0.join("outside"),&out).unwrap();}
            6=>{put(&r,&format!("{G}/dep2.ores"),"import\n class D from './dep3';\n");}
            7=>{put(&r,&format!("{G}/dep2.ores"),b"\xff");}
            8=>{let p=r.join("contracts/zorde/scalar-instances/Port/valid/0.json");fs::remove_file(&p).unwrap();
                assert!(std::process::Command::new("/usr/bin/mkfifo").arg(p).status().unwrap().success());}
            _=>unreachable!()
        }
        assert!(capture(&r,&out).is_err(),"mode {mode}");
        assert!(!out.is_file(),"mode {mode}");
    }
}
#[test]fn manifest_recheck_refuses_changed_corpus_and_forged_paths(){
    let t=Temp::new();let r=fixture(&t);let out=t.0.join("manifest.json");
    let hash=capture(&r,&out).unwrap();
    let p=r.join("contracts/zorde/scalar-instances/Port/valid/0.json");
    fs::rename(&p,p.with_file_name("renamed.json")).unwrap();
    assert!(recheck(&r,&out,&hash).is_err());
    fs::rename(p.with_file_name("renamed.json"),p).unwrap();
    let original=fs::read(&out).unwrap();fs::write(&out,b"{\"schema\":\"forged\"}").unwrap();
    assert!(recheck(&r,&out,&hash).is_err());fs::write(&out,original).unwrap();
    recheck(&r,&out,&hash).unwrap();
    assert!(capture(&r,&r.join("manifest.json")).is_err());
}
