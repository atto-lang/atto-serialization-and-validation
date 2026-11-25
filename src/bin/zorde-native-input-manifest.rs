use atto_actor_contract_conformance::native_input_manifest::{capture,recheck};
use std::path::Path;
fn main(){
    let a:Vec<_>=std::env::args().collect();
    let result=match a.as_slice(){
        [_,cmd,root,out] if cmd=="capture"=>capture(Path::new(root),Path::new(out)).map(|s|println!("{s}")),
        [_,cmd,root,out,digest] if cmd=="recheck"=>recheck(Path::new(root),Path::new(out),digest),
        _=>{eprintln!("usage: zorde-native-input-manifest capture ROOT OUT | recheck ROOT OUT SHA256");std::process::exit(2);}
    };
    if let Err(e)=result{eprintln!("native input manifest refused: {e}");std::process::exit(1);}
}
