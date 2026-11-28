use atto_actor_contract_conformance::serde_receipts::verify;
use std::path::Path;
fn main(){
    let args:Vec<_>=std::env::args().collect();
    if args.len()!=7||args[1]!="--receipts"||args[3]!="--head"||args[5]!="--tree"{
        eprintln!("usage: verify-serde-receipts --receipts DIR --head SHA --tree SHA");
        std::process::exit(2);
    }
    match verify(Path::new(&args[2]),&args[4],&args[6]){
        Ok(())=>{println!("STRUCTURAL_RECEIPT_GATE=PASS");println!("INDEPENDENT_RUN_AND_ARTIFACT_VERIFICATION=STILL_REQUIRED");}
        Err(e)=>{eprintln!("STRUCTURAL_RECEIPT_GATE=FAIL: {e}");std::process::exit(1);}
    }
}
