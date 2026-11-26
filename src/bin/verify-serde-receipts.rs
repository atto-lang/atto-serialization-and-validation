use atto_actor_contract_conformance::serde_receipts::verify;
use std::{collections::BTreeMap,ffi::OsString,path::Path};
fn usage()->!{
    eprintln!("usage: verify-serde-receipts --receipts DIR --head SHA --tree SHA");
    std::process::exit(2)
}
fn main(){
    let args:Vec<OsString>=std::env::args_os().skip(1).collect();
    let mut map=BTreeMap::new();
    if args.len()!=6{usage();}
    for pair in args.chunks_exact(2){
        let key=match pair[0].to_str(){Some("--receipts")=>"receipts",Some("--head")=>"head",Some("--tree")=>"tree",_=>usage()};
        if pair[1].is_empty()||map.insert(key,&pair[1]).is_some(){usage();}
    }
    let (Some(dir),Some(head),Some(tree))=(map.get("receipts"),map.get("head").and_then(|s|s.to_str()),map.get("tree").and_then(|s|s.to_str()))else{usage()};
    match verify(Path::new(dir),head,tree){
        Ok(())=>{println!("STRUCTURAL_RECEIPT_GATE=PASS");println!("INDEPENDENT_RUN_AND_ARTIFACT_VERIFICATION=STILL_REQUIRED");}
        Err(e)=>{eprintln!("STRUCTURAL_RECEIPT_GATE=FAIL: {e}");std::process::exit(1);}
    }
}
