use atto_actor_contract_conformance::native_receipt::{Approved,execute_and_issue};
use std::path::Path;
fn main(){
    let a:Vec<_>=std::env::args().collect();
    if a.len()!=12{eprintln!("usage: zorde-generated-receipt ROOT EVIDENCE COMPILER NODE HEAD TREE COMPILER_SHA NODE_SHA IR_SHA PARITY_SHA VERIFICATION_SHA");std::process::exit(2);}
    let approved=Approved{head:&a[5],tree:&a[6],compiler_sha:&a[7],node_sha:&a[8],ir_sha:&a[9],parity_sha:&a[10],verification_sha:&a[11]};
    if let Err(e)=execute_and_issue(Path::new(&a[1]),Path::new(&a[2]),Path::new(&a[3]),Path::new(&a[4]),&approved){
        eprintln!("ZORDE_CANDIDATE_RECEIPT_REFUSED: {e}");std::process::exit(1);
    }
    println!("ZORDE_PROCESS_OBSERVATION_CANDIDATE_CREATED; NATIVE_GUEST_NOT_CERTIFIED");
}
