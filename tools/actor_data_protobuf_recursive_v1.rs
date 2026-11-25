//! Allocation-free host preflight for descriptor-directed Protobuf nesting.
//! Not a typed decoder or a schema authentication mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject { Limits, Budget, Truncated, Varint, Tag, Wire, Descriptor, Unknown }
#[derive(Clone, Copy)]
pub struct Limits {
    pub wire: usize, pub logical: usize, pub work: usize,
    pub fields: usize, pub messages: usize, pub depth: usize,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage { pub logical: usize, pub work: usize, pub fields: usize, pub messages: usize }
#[derive(Clone, Copy)]
pub enum Kind<'a> { Varint, Fixed64, Bytes, Fixed32, Message(&'a Schema<'a>) }
#[derive(Clone, Copy)]
pub struct Field<'a> { pub number: u32, pub kind: Kind<'a> }
pub struct Schema<'a> { pub fields: &'a [Field<'a>] }
struct Meter { limits: Limits, used: Usage }
fn charge(n: &mut usize, amount: usize, cap: usize) -> Result<(), Reject> {
    let next = n.checked_add(amount).ok_or(Reject::Budget)?;
    if next > cap { return Err(Reject::Budget); }
    *n = next; Ok(())
}
impl Meter {
    fn work(&mut self, n: usize) -> Result<(), Reject> { charge(&mut self.used.work,n,self.limits.work) }
    fn logical(&mut self,n:usize)->Result<(),Reject>{charge(&mut self.used.logical,n,self.limits.logical)}
    fn payload(&mut self,n:usize)->Result<(),Reject>{
        self.logical(n.checked_mul(16).ok_or(Reject::Budget)?)
    }
}
struct Cursor<'a> { input: &'a [u8], pos: usize }
impl<'a> Cursor<'a> {
    fn byte(&mut self,m:&mut Meter)->Result<u8,Reject>{
        let b=*self.input.get(self.pos).ok_or(Reject::Truncated)?;
        m.work(1)?; self.pos+=1; Ok(b)
    }
    fn varint(&mut self,m:&mut Meter)->Result<(u64,usize),Reject>{
        let start=self.pos; let mut value=0u64;
        for i in 0..10 {
            let b=self.byte(m)?;
            if i==9 && b>1 {return Err(Reject::Varint);}
            value|=u64::from(b&127)<<(7*i);
            if b<128 {return Ok((value,self.pos-start));}
        }
        Err(Reject::Varint)
    }
    fn sub(&mut self,n:usize)->Result<&'a [u8],Reject>{
        let end=self.pos.checked_add(n).ok_or(Reject::Budget)?;
        let out=self.input.get(self.pos..end).ok_or(Reject::Truncated)?;
        self.pos=end; Ok(out)
    }
}
fn expected_wire(k:Kind<'_>)->u64 {
    match k {Kind::Varint=>0,Kind::Fixed64=>1,Kind::Bytes|Kind::Message(_)=>2,Kind::Fixed32=>5}
}
fn lookup<'a>(schema:&Schema<'a>,tag:u32,m:&mut Meter)->Result<Kind<'a>,Reject>{
    let mut lo=0;let mut hi=schema.fields.len();
    while lo<hi {
        m.work(1)?;
        let mid=lo+(hi-lo)/2;
        let f=schema.fields[mid];
        match f.number.cmp(&tag) {
            std::cmp::Ordering::Less=>lo=mid+1,
            std::cmp::Ordering::Greater=>hi=mid,
            std::cmp::Ordering::Equal=>return Ok(f.kind),
        }
    }
    Err(Reject::Unknown)
}
fn message(input:&[u8],schema:&Schema<'_>,depth:usize,m:&mut Meter)->Result<(),Reject>{
    if depth>m.limits.depth{return Err(Reject::Budget);}
    charge(&mut m.used.messages,1,m.limits.messages)?;
    m.logical(128)?;m.work(1)?;
    if schema.fields.len()>1024{return Err(Reject::Descriptor);}
    let mut prior=0;
    // Check descriptor ordering and legal schema tags, charging each visit.
    // Recursive descriptors are allowed; only encountered wire nesting recurses.
    for f in schema.fields {
        m.work(1)?;
        if f.number<=prior||f.number>536870911||(19000..=19999).contains(&f.number){
            return Err(Reject::Descriptor);
        }
        prior=f.number;
    }
    let mut c=Cursor{input,pos:0};
    while c.pos<input.len(){
        charge(&mut m.used.fields,1,m.limits.fields)?;
        m.logical(128)?;m.work(1)?;
        let (key,_)=c.varint(m)?;
        let tag=key>>3;let wire=key&7;
        if tag==0||tag>536870911{return Err(Reject::Tag);}
        if !matches!(wire,0|1|2|5){return Err(Reject::Wire);}
        let kind=lookup(schema,tag as u32,m)?;
        if wire!=expected_wire(kind){return Err(Reject::Wire);}
        match kind {
            Kind::Varint=>{let (_,n)=c.varint(m)?;m.payload(n)?;}
            Kind::Fixed64|Kind::Fixed32=>{
                let n=if wire==1{8}else{4};
                m.payload(n)?;m.work(n)?;c.sub(n)?;
            }
            Kind::Bytes|Kind::Message(_)=>{
                let (n,_)=c.varint(m)?;
                let n=usize::try_from(n).map_err(|_|Reject::Budget)?;
                // Charge any copied length-delimited bytes, even when the
                // nested parser additionally charges materialized child fields.
                m.payload(n)?;
                let body=c.sub(n)?;
                match kind {
                    Kind::Message(child)=>message(body,child,depth+1,m)?,
                    Kind::Bytes=>m.work(n)?,
                    _=>unreachable!(),
                }
            }
        }
    }
    Ok(())
}
pub fn preflight(input:&[u8],schema:&Schema<'_>,limits:Limits)->Result<Usage,Reject>{
    if limits.wire==0||limits.wire>1024*1024||limits.logical==0||limits.logical>8*1024*1024||
       limits.work==0||limits.work>4*1024*1024||limits.fields>100000||
       limits.messages==0||limits.messages>100000||limits.depth>64{return Err(Reject::Limits);}
    if input.len()>limits.wire{return Err(Reject::Budget);}
    let mut m=Meter{limits,used:Usage::default()};
    // Charge root input retention once. Child slices never allocate.
    m.payload(input.len())?;
    message(input,schema,0,&mut m)?;
    Ok(m.used)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn limits()->Limits{Limits{wire:1024,logical:8192,work:4096,fields:100,messages:100,depth:8}}
    #[test] fn exact_nested_meter_and_all_one_under_limits(){
        let leaf=Schema{fields:&[Field{number:1,kind:Kind::Varint}]};
        let root=Schema{fields:&[Field{number:1,kind:Kind::Message(&leaf)}]};
        let bytes=[0x0a,2,0x08,1];
        let exact=Limits{wire:4,logical:624,work:12,fields:2,messages:2,depth:1};
        let expected=Usage{logical:624,work:12,fields:2,messages:2};
        assert_eq!(preflight(&bytes,&root,exact),Ok(expected));
        for l in [Limits{wire:3,..exact},Limits{logical:623,..exact},Limits{work:11,..exact},
                  Limits{fields:1,..exact},Limits{messages:1,..exact},Limits{depth:0,..exact}]{
            assert_eq!(preflight(&bytes,&root,l),Err(Reject::Budget));
        }
    }
    #[test] fn repeated_empty_children_consume_shared_credit(){
        let leaf=Schema{fields:&[]};
        let root=Schema{fields:&[Field{number:1,kind:Kind::Message(&leaf)}]};
        let b=[0x0a,0,0x0a,0,0x0a,0];
        let u=preflight(&b,&root,limits()).unwrap();
        assert_eq!((u.messages,u.fields),(4,3));
        assert_eq!(preflight(&b,&root,Limits{messages:3,..limits()}),Err(Reject::Budget));
        assert_eq!(preflight(&b,&root,Limits{fields:2,..limits()}),Err(Reject::Budget));
    }
    #[test] fn opaque_bytes_never_infer_nested_messages(){
        let bytes=Schema{fields:&[Field{number:1,kind:Kind::Bytes}]};
        let empty=Schema{fields:&[]};
        let nested=Schema{fields:&[Field{number:1,kind:Kind::Message(&empty)}]};
        let b=[0x0a,1,0xff];
        assert!(preflight(&b,&bytes,limits()).is_ok());
        assert_eq!(preflight(&b,&nested,limits()),Err(Reject::Truncated));
    }
    #[test] fn malformed_and_descriptor_mismatch_fail_closed(){
        let schema=Schema{fields:&[Field{number:1,kind:Kind::Varint}]};
        assert_eq!(preflight(&[0],&schema,limits()),Err(Reject::Tag));
        assert_eq!(preflight(&[0x10,1],&schema,limits()),Err(Reject::Unknown));
        assert_eq!(preflight(&[0x0a,0],&schema,limits()),Err(Reject::Wire));
        assert_eq!(preflight(&[0x0b],&schema,limits()),Err(Reject::Wire));
        assert_eq!(preflight(&[8,128],&schema,limits()),Err(Reject::Truncated));
        let mut over=[0xff;11];over[0]=8;
        assert_eq!(preflight(&over,&schema,limits()),Err(Reject::Varint));
        let duplicate=Schema{fields:&[Field{number:1,kind:Kind::Varint},Field{number:1,kind:Kind::Bytes}]};
        assert_eq!(preflight(&[],&duplicate,limits()),Err(Reject::Descriptor));
        let reserved=Schema{fields:&[Field{number:19000,kind:Kind::Varint}]};
        assert_eq!(preflight(&[],&reserved,limits()),Err(Reject::Descriptor));
    }
    #[test] fn fixed_width_and_oversized_child(){
        let schema=Schema{fields:&[Field{number:1,kind:Kind::Fixed32},Field{number:2,kind:Kind::Fixed64},Field{number:3,kind:Kind::Bytes}]};
        assert!(preflight(&[0x0d,1,2,3,4,0x11,1,2,3,4,5,6,7,8],&schema,limits()).is_ok());
        assert_eq!(preflight(&[0x0d,1],&schema,limits()),Err(Reject::Truncated));
        assert_eq!(preflight(&[0x1a,0x80,0x80,0x80,0x80,0x10],&schema,limits()),Err(Reject::Budget));
    }
}
