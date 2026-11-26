use atto_actor_contract_conformance::{native_input_manifest::FIXED,native_receipt::{Approved,execute_and_issue},serde_receipts::sha256};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},process::Command,sync::atomic::{AtomicU64,Ordering}};
use std::os::unix::fs::{DirBuilderExt,PermissionsExt};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp{fn new()->Self{let p=std::env::temp_dir().join(format!("zorde-receipt-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::DirBuilder::new().mode(0o700).create(&p).unwrap();Self(p)}}
impl Drop for Temp{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
const G:&str="artifacts/zorde-generated";
fn put(root:&Path,name:&str,data:impl AsRef<[u8]>){let p=root.join(name);fs::create_dir_all(p.parent().unwrap()).unwrap();fs::write(p,data).unwrap();}
fn obj(root:&Path,name:&str,v:Value){put(root,name,serde_json::to_vec(&v).unwrap());}
fn hash(p:&Path)->String{sha256(&fs::read(p).unwrap()).unwrap()}
fn git(root:&Path,a:&[&str])->String{let o=Command::new("/usr/bin/git").current_dir(root).args(a).output().unwrap();assert!(o.status.success(),"{:?}",o);String::from_utf8(o.stdout).unwrap().trim().into()}
struct Fixture{root:PathBuf,evidence:PathBuf,compiler:PathBuf,node:PathBuf,head:String,tree:String,hashes:[String;5]}
impl Fixture{
    fn new(t:&Temp,mode:u32)->Self{
        let root=t.0.join("src");fs::create_dir(&root).unwrap();
        let evidence=t.0.join("evidence");fs::DirBuilder::new().mode(0o700).create(&evidence).unwrap();
        for n in FIXED{put(&root,n,b"synthetic\n");}
        for (decl,p,n) in [("Port",3,2),("ShortLabel",2,2)]{for (lane,count) in [("valid",p),("invalid",n)]{for i in 0..count{
            put(&root,&format!("contracts/zorde/scalar-instances/{decl}/{lane}/{i}.json"),b"0\n");
        }}}
        put(&root,&format!("{G}/registry-smoke.ores"),"import class D from './dep0';\n");
        for i in 0..8{put(&root,&format!("{G}/dep{i}.ores"),if i<7{format!("import class D from './dep{}';\n",i+1)}else{"leaf\n".into()});}
        let ids=json!(["Zorde.Validation.Port","Zorde.Validation.ShortLabel"]);
        obj(&root,"artifacts/zorde-scalars-contract-ir.json",json!({"irId":"synthetic","status":"passed","admissible":true}));
        obj(&root,"artifacts/zorde-scalars-consumer-verification.json",json!({"schema":"ores.typespec-json-schema-validator.consumer-verification-receipt/v1","status":"passed","admissible":true,
            "expectedIrId":"synthetic","computedIrId":"synthetic","suppliedIrId":"synthetic","declarationIds":ids}));
        let corpus="a".repeat(64);
        obj(&root,"artifacts/zorde-scalars-parity.json",json!({"configuration":{"differential":{"instanceCorpusDigest":corpus}},"differential":{"summary":{"corpusInstances":9,"declarationsWithReviewedPositiveAndNegative":2,"reviewedUnassertedInstances":0}}}));
        obj(&root,&format!("{G}/registry-plan.json"),json!({"irId":"synthetic","tjsvRunId":"synthetic","entries":[{"schemaId":"Zorde.Validation.Port"},{"schemaId":"Zorde.Validation.ShortLabel"}],"releaseReady":false,"runtimeImplementationVerified":false}));
        obj(&root,&format!("{G}/build-complete.json"),json!({"schema":"oreslang.zorde.static-registry-build-complete/v1","irId":"synthetic","tjsvRunId":"synthetic",
            "guestSmokeCompiled":false,"guestSmokeExecuted":false,"nativeGuestExecuted":false,"releaseReady":false,
            "planSha256":hash(&root.join(format!("{G}/registry-plan.json"))),"guestSha256":hash(&root.join(format!("{G}/registry-specs.ores"))),"smokeSha256":hash(&root.join(format!("{G}/registry-smoke.ores"))),"reviewedCorpusDigest":corpus,"reviewedCorpusInstances":9}));
        git(&root,&["init","-q"]);git(&root,&["config","user.name","test"]);git(&root,&["config","user.email","test@example.invalid"]);
        git(&root,&["add","."]);git(&root,&["commit","-qm","synthetic"]);
        let head=git(&root,&["rev-parse","HEAD"]);let tree=git(&root,&["rev-parse","HEAD^{tree}"]);
        let compiler=t.0.join("compiler");let node=t.0.join("node");
        let script:String=match mode{
            1=>"#!/bin/sh\nprintf 'wrong marker\\n'\n".into(),
            2=>"#!/bin/sh\nprintf 'zorde generated registry guest smoke passed\\n'\nexit 7\n".into(),
            3=>format!("#!/bin/sh\nprintf tampered > '{}/contracts/zorde/scalars.tsp'\nprintf 'zorde generated registry guest smoke passed\\n'\n",root.display()),
            _=>"#!/bin/sh\nprintf 'zorde generated registry guest smoke passed\\n'\n".into()
        };
        fs::write(&compiler,script).unwrap();fs::write(&node,b"#!/bin/sh\nexit 0\n").unwrap();
        for p in [&compiler,&node]{fs::set_permissions(p,fs::Permissions::from_mode(0o700)).unwrap();}
        let hashes=[hash(&compiler),hash(&node),hash(&root.join("artifacts/zorde-scalars-contract-ir.json")),hash(&root.join("artifacts/zorde-scalars-parity.json")),hash(&root.join("artifacts/zorde-scalars-consumer-verification.json"))];
        Self{root,evidence,compiler,node,head,tree,hashes}
    }
    fn run(&self)->Result<(),String>{
        let a=Approved{head:&self.head,tree:&self.tree,compiler_sha:&self.hashes[0],node_sha:&self.hashes[1],ir_sha:&self.hashes[2],parity_sha:&self.hashes[3],verification_sha:&self.hashes[4]};
        execute_and_issue(&self.root,&self.evidence,&self.compiler,&self.node,&a)
    }
    fn receipt(&self)->PathBuf{self.evidence.join("zorde-generated-smoke-native-receipt.json")}
}
#[test]fn synthetic_process_receipt_never_claims_native_guest(){
    let t=Temp::new();let f=Fixture::new(&t,0);f.run().unwrap();
    let raw=fs::read(f.receipt()).unwrap();let r:Value=serde_json::from_slice(&raw).unwrap();
    assert_eq!(r["schema"],"oreslang.zorde.source-local.generated-guest-candidate/v1");
    assert_eq!(r["native_guest_executed"],false);assert_eq!(r["approved_process_executed"],true);
    assert_eq!(r["release_ready"],false);assert_eq!(r["static_guest_import_module_count"],8);
    assert_eq!(r["native_log_sha256"],hash(&f.evidence.join("zorde-generated-smoke-native.log")));
    assert!(f.run().is_err());assert_eq!(fs::read(f.receipt()).unwrap(),raw);
}
#[test]fn process_failure_and_mutation_never_publish(){
    for mode in 1..=3{let t=Temp::new();let f=Fixture::new(&t,mode);assert!(f.run().is_err(),"{mode}");assert!(!f.receipt().exists());}
}
#[test]fn approval_and_preexisting_log_fail_closed(){
    let t=Temp::new();let mut f=Fixture::new(&t,0);f.hashes[2]="0".repeat(64);
    assert!(f.run().is_err());assert!(!f.receipt().exists());assert!(!f.evidence.join("zorde-generated-smoke-native.log").exists());
    let t=Temp::new();let f=Fixture::new(&t,0);put(&f.evidence,"zorde-generated-smoke-native.log",b"forged");
    assert!(f.run().is_err());assert!(!f.receipt().exists());assert_eq!(fs::read(f.evidence.join("zorde-generated-smoke-native.log")).unwrap(),b"forged");
}
