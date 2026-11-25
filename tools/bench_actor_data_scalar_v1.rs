//! Optimized host-reference microbenchmarks, not Atto compiler or end-to-end IPC.
//! Identical logical scalar record; wire semantics and host representations differ.
#![allow(dead_code)]
#[path="actor_data_cbor_preflight_v1.rs"] mod cbor;
#[path="actor_data_protobuf_recursive_v1.rs"] mod proto;
#[path="actor_data_flatbuffers_scalar_v1.rs"] mod flat;
use std::{hint::black_box,time::Instant};

fn sample<F:FnMut()>(codec:&str,phase:&str,bytes:usize,iterations:u32,mut action:F){
    for _ in 0..10_000{action();}
    let mut ns=[0.0_f64;7];
    for n in &mut ns{
        let start=Instant::now();
        for _ in 0..iterations{action();}
        *n=start.elapsed().as_secs_f64()*1e9/f64::from(iterations);
    }
    ns.sort_by(f64::total_cmp);
    let median=ns[3];assert!(median.is_finite()&&median>0.0);
    println!("{codec},{phase},{bytes},{iterations},7,{median:.2},{:.2},{:.2},{:.2},{:.2}",
        ns[0],ns[6],1e9/median,bytes as f64*1e9/median/1048576.0);
}
fn main(){
    let args:Vec<String>=std::env::args().collect();
    assert_eq!(args.len(),3,"usage: bench <flatc-generated fixture> <iterations>");
    let n:u32=args[2].parse().unwrap();assert!((10_000..=1_000_000).contains(&n));
    let fb=std::fs::read(&args[1]).unwrap();
    let cb=[0x83,0xf5,0x38,41,0x1a,0xff,0xff,0xff,0xff];
    let mut pb=vec![8,1,17];pb.extend_from_slice(&(-42i64).to_le_bytes());
    pb.push(29);pb.extend_from_slice(&u32::MAX.to_le_bytes());
    let cf=[cbor::Shape::Bool,cbor::Shape::I64,cbor::Shape::I64];
    let cs=cbor::Shape::Record(&cf);
    let pf=[proto::Field{number:1,kind:proto::Kind::Varint},proto::Field{number:2,kind:proto::Kind::Fixed64},proto::Field{number:3,kind:proto::Kind::Fixed32}];
    let ps=proto::Schema{fields:&pf};
    let fs=[flat::Kind::Bool,flat::Kind::I64,flat::Kind::U32];
    let cl=cbor::Limits{wire:4096,expanded:65536,nodes:64,elements:64,work:65536,depth:8};
    let pl=proto::Limits{wire:4096,logical:65536,work:65536,fields:64,messages:16,depth:8};
    let fl=flat::Limits{wire:4096,logical:65536,work:65536};
    assert_eq!(cbor::decode_typed(&cb,cl,&cs).unwrap().0,cbor::Value::Record(vec![cbor::Value::Bool(true),cbor::Value::I64(-42),cbor::Value::I64(i64::from(u32::MAX))]));
    assert_eq!(proto::decode(&pb,&ps,pl).unwrap().0,vec![
        proto::FieldValue{number:1,value:proto::Value::Scalar(proto::Scalar::Varint(1))},
        proto::FieldValue{number:2,value:proto::Value::Scalar(proto::Scalar::Fixed64((-42i64) as u64))},
        proto::FieldValue{number:3,value:proto::Value::Scalar(proto::Scalar::Fixed32(u32::MAX))}]);
    assert_eq!(flat::decode(&fb,&fs,fl).unwrap().0,vec![flat::Value::Bool(true),flat::Value::I64(-42),flat::Value::U32(u32::MAX)]);
    println!("codec,phase,wire_bytes,iterations_per_round,rounds,median_ns_per_op,min_ns_per_op,max_ns_per_op,median_ops_per_second,median_wire_mib_per_second");
    sample("cbor","preflight",cb.len(),n,||{black_box(cbor::preflight_typed(black_box(&cb),black_box(cl),black_box(&cs)).unwrap());});
    sample("protobuf","preflight",pb.len(),n,||{black_box(proto::preflight(black_box(&pb),black_box(&ps),black_box(pl)).unwrap());});
    sample("flatbuffers","preflight",fb.len(),n,||{black_box(flat::preflight(black_box(&fb),black_box(&fs),black_box(fl)).unwrap());});
    sample("cbor","owned_decode",cb.len(),n,||{black_box(cbor::decode_typed(black_box(&cb),black_box(cl),black_box(&cs)).unwrap());});
    sample("protobuf","owned_decode",pb.len(),n,||{black_box(proto::decode(black_box(&pb),black_box(&ps),black_box(pl)).unwrap());});
    sample("flatbuffers","owned_decode",fb.len(),n,||{black_box(flat::decode(black_box(&fb),black_box(&fs),black_box(fl)).unwrap());});
    sample("cbor","preflight_plus_decode",cb.len(),n,||{black_box(cbor::preflight_typed(black_box(&cb),cl,&cs).unwrap());black_box(cbor::decode_typed(black_box(&cb),cl,&cs).unwrap());});
    sample("protobuf","preflight_plus_decode",pb.len(),n,||{black_box(proto::preflight(black_box(&pb),&ps,pl).unwrap());black_box(proto::decode(black_box(&pb),&ps,pl).unwrap());});
    sample("flatbuffers","preflight_plus_decode",fb.len(),n,||{black_box(flat::preflight(black_box(&fb),&fs,fl).unwrap());black_box(flat::decode(black_box(&fb),&fs,fl).unwrap());});
}
