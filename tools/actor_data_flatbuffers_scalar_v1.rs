//! Restricted FlatBuffers root-table profile. No pointers, defaults, aliases,
//! nested tables, file identifier, size prefix, or borrowed zero-copy views.
//! Standard little-endian root/table/vtable format; deliberately rejects
//! layouts outside this profile. Host reference only.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind { Bool,I64,U32,F64 }
#[derive(Debug,PartialEq)]
pub enum Value { Bool(bool),I64(i64),U32(u32),F64(f64) }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Reject { Limits,Bounds,Layout,Shape,Scalar,Budget,Allocation }
#[derive(Clone,Copy)]
pub struct Limits { pub wire:usize,pub logical:usize,pub work:usize }
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Usage { pub logical:usize,pub work:usize }
fn read<const N:usize>(b:&[u8],at:usize)->Result<[u8;N],Reject>{
    b.get(at..at.checked_add(N).ok_or(Reject::Bounds)?).ok_or(Reject::Bounds)?
        .try_into().map_err(|_|Reject::Bounds)
}
fn u16at(b:&[u8],at:usize)->Result<usize,Reject>{Ok(u16::from_le_bytes(read(b,at)?) as usize)}
fn charge(n:&mut usize,add:usize,cap:usize)->Result<(),Reject>{
    *n=n.checked_add(add).ok_or(Reject::Budget)?;
    if *n>cap{return Err(Reject::Budget)} Ok(())
}
fn parse(b:&[u8],shape:&[Kind],limits:Limits,materialize:bool)->Result<(Option<Vec<Value>>,Usage),Reject>{
    if limits.wire==0||limits.wire>4096||limits.logical==0||limits.logical>65536||
        limits.work==0||limits.work>65536||shape.is_empty()||shape.len()>64{return Err(Reject::Limits)}
    if b.len()>limits.wire{return Err(Reject::Budget)}
    let mut u=Usage::default();
    // Bound all scanning and fixed scratch before any result allocation.
    charge(&mut u.work,b.len().checked_add(16*shape.len()).ok_or(Reject::Budget)?,limits.work)?;
    charge(&mut u.logical,128+128*shape.len(),limits.logical)?;
    let root=u32::from_le_bytes(read(b,0)?) as usize;
    if root<8||root%4!=0{return Err(Reject::Layout)}
    let delta=i32::from_le_bytes(read(b,root)?);
    if delta<=0{return Err(Reject::Layout)}
    let vt=root.checked_sub(delta as usize).ok_or(Reject::Bounds)?;
    if vt<4||vt%2!=0{return Err(Reject::Layout)}
    let vlen=u16at(b,vt)?;
    let object=u16at(b,vt+2)?;
    if vlen!=4+2*shape.len()||object<4{return Err(Reject::Shape)}
    let vend=vt.checked_add(vlen).ok_or(Reject::Bounds)?;
    let end=root.checked_add(object).ok_or(Reject::Bounds)?;
    if vend>root||end!=b.len(){return Err(Reject::Layout)}
    // Fixed scratch is independent of input lengths. Forbid overlapping fields,
    // offsets into the table header, vtable, and nonzero unclaimed bytes.
    let mut occupied=[false;4096];
    occupied[..4].fill(true);occupied[vt..vend].fill(true);occupied[root..root+4].fill(true);
    let mut values=materialize.then(Vec::new);
    if let Some(v)=values.as_mut(){v.try_reserve_exact(shape.len()).map_err(|_|Reject::Allocation)?;}
    for (i,kind) in shape.iter().enumerate(){
        let offset=u16at(b,vt+4+2*i)?;
        let width=match kind{Kind::Bool=>1,Kind::U32=>4,Kind::I64|Kind::F64=>8};
        let at=root.checked_add(offset).ok_or(Reject::Bounds)?;
        let stop=at.checked_add(width).ok_or(Reject::Bounds)?;
        if offset<4||stop>end||at%width!=0{return Err(Reject::Layout)}
        if occupied[at..stop].iter().any(|x|*x){return Err(Reject::Layout)}
        occupied[at..stop].fill(true);
        let value=match kind{
            Kind::Bool=>match b[at]{0=>Value::Bool(false),1=>Value::Bool(true),_=>return Err(Reject::Scalar)},
            Kind::I64=>Value::I64(i64::from_le_bytes(read(b,at)?)),
            Kind::U32=>Value::U32(u32::from_le_bytes(read(b,at)?)),
            Kind::F64=>{
                let f=f64::from_le_bytes(read(b,at)?);
                if !f.is_finite(){return Err(Reject::Scalar)}
                Value::F64(f)
            }
        };
        if let Some(v)=values.as_mut(){v.push(value);}
    }
    if b.iter().zip(occupied.iter()).any(|(byte,used)|!used&&*byte!=0){return Err(Reject::Layout)}
    Ok((values,u))
}
pub fn preflight(b:&[u8],shape:&[Kind],limits:Limits)->Result<Usage,Reject>{
    parse(b,shape,limits,false).map(|(_,u)|u)
}
pub fn decode(b:&[u8],shape:&[Kind],limits:Limits)->Result<(Vec<Value>,Usage),Reject>{
    let(v,u)=parse(b,shape,limits,true)?;Ok((v.ok_or(Reject::Shape)?,u))
}
#[cfg(test)]
mod tests{
    use super::*;
    const SHAPE:[Kind;3]=[Kind::Bool,Kind::I64,Kind::U32];
    fn limits()->Limits{Limits{wire:4096,logical:65536,work:65536}}
    // root at 16, vtable at 4, field positions 20/24/32; padding zero.
    fn sample()->Vec<u8>{
        let mut b=vec![0;36];
        b[..4].copy_from_slice(&16u32.to_le_bytes());
        for(i,v)in [10u16,20,4,8,16].iter().enumerate(){b[4+2*i..6+2*i].copy_from_slice(&v.to_le_bytes());}
        b[16..20].copy_from_slice(&12i32.to_le_bytes());b[20]=1;
        b[24..32].copy_from_slice(&i64::MIN.to_le_bytes());
        b[32..36].copy_from_slice(&u32::MAX.to_le_bytes());b
    }
    #[test]fn exact_and_under_budget(){
        let b=sample();let(v,u)=decode(&b,&SHAPE,limits()).unwrap();
        assert_eq!(v,vec![Value::Bool(true),Value::I64(i64::MIN),Value::U32(u32::MAX)]);
        assert_eq!(preflight(&b,&SHAPE,limits()),Ok(u));
        let l=Limits{wire:b.len(),logical:u.logical,work:u.work};
        assert_eq!(decode(&b,&SHAPE,l).unwrap().1,u);
        for l in [Limits{wire:l.wire-1,..l},Limits{logical:l.logical-1,..l},Limits{work:l.work-1,..l}]{
            assert_eq!(preflight(&b,&SHAPE,l),Err(Reject::Budget));
        }
    }
    #[test]fn malformed_offsets_aliases_padding_and_scalars(){
        let b=sample();
        for n in 0..b.len(){assert!(preflight(&b[..n],&SHAPE,limits()).is_err());}
        for (at,value) in [(0,255),(4,12),(6,21),(8,0),(10,4),(12,8),(16,0),(20,2),(21,1)]{
            let mut bad=b.clone();bad[at]=value;
            assert!(preflight(&bad,&SHAPE,limits()).is_err(),"offset {at}");
        }
        let mut bad=b.clone();bad.push(0);assert!(preflight(&bad,&SHAPE,limits()).is_err());
        let mut bad=b.clone();bad[24..32].copy_from_slice(&f64::NAN.to_le_bytes());
        assert_eq!(preflight(&bad,&[Kind::Bool,Kind::F64,Kind::U32],limits()),Err(Reject::Scalar));
    }
    #[test]fn official_flatc_fixture_when_supplied(){
        // Separate CI generates this binary using the official flatc compiler.
        if let Ok(path)=std::env::var("ATTO_FLATC_FIXTURE"){
            let b=std::fs::read(path).unwrap();
            let(v,_)=decode(&b,&SHAPE,limits()).unwrap();
            assert_eq!(v,vec![Value::Bool(true),Value::I64(-42),Value::U32(4294967295)]);
        }else{println!("FLATC_FIXTURE_NOT_SUPPLIED");}
    }
}
