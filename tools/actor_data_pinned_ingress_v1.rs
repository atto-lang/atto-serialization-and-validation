//! Host composition reference, NOT cryptographic authentication or guest authority.
//! A trusted transport must authenticate the complete metadata and payload together,
//! then call this API with the exact immutable bytes it verified.
#[path="actor_data_codec_policy_v1.rs"] pub mod policy;
#[path="actor_data_cbor_preflight_v1.rs"] pub mod cbor;
#[path="actor_data_protobuf_recursive_v1.rs"] pub mod proto;
#[path="actor_data_mailbox_credits_v1.rs"] pub mod mailbox;

use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
pub enum Reject { Binding(policy::Reject), Generation, Cbor(cbor::Reject), Proto(proto::Reject), Mailbox(mailbox::Reject), Allocation, Expansion }

pub struct VerifiedMetadata<'a> {
    pub codec: &'a str, pub version: u32, pub schema: &'a str,
    pub shape_digest: &'a [u8;32], pub recipient_generation: u64,
}
enum Decoder<'a> {
    Cbor(&'a cbor::Shape<'a>, cbor::Limits),
    Protobuf(&'a proto::Schema<'a>, proto::Limits),
}
pub struct Endpoint<'a> {
    binding: policy::Binding<'a>, generation:u64,
    decoder:Decoder<'a>, mailbox:mailbox::Mailbox, binding_token:Arc<()>,
}
/// Owned host wire values only; not a nominal guest ADT or actor authority.
#[derive(Debug, PartialEq)]
pub enum MaterializedValue { Cbor(cbor::Value), Protobuf(Vec<proto::FieldValue>) }
/// The value drops before the retained wire/expansion lease. No Clone or
/// into_value API. Trusted consumers must separately account their own copies.
pub struct MaterializedDelivery {
    value:MaterializedValue,
    delivery:mailbox::Delivery,
    generation:u64,
}
impl MaterializedDelivery {
    pub fn value(&self)->&MaterializedValue { &self.value }
    pub fn expansion_credit(&self)->usize { self.delivery.expansion_credit() }
    pub fn generation(&self)->u64 { self.generation }
}
impl<'a> Endpoint<'a> {
    pub fn cbor(schema:&'a str, version:u32, digest:[u8;32], allowed:u8,
                generation:u64, shape:&'a cbor::Shape<'a>, limits:cbor::Limits,
                mailbox:mailbox::Mailbox)->Result<Self,Reject>{
        Self::register("cbor",4,schema,version,digest,allowed,generation,Decoder::Cbor(shape,limits),mailbox)
    }
    pub fn protobuf(schema:&'a str, version:u32, digest:[u8;32], allowed:u8,
                    generation:u64, shape:&'a proto::Schema<'a>, limits:proto::Limits,
                    mailbox:mailbox::Mailbox)->Result<Self,Reject>{
        Self::register("protobuf",2,schema,version,digest,allowed,generation,Decoder::Protobuf(shape,limits),mailbox)
    }
    #[allow(clippy::too_many_arguments)]
    fn register(codec:&str,supported:u8,schema:&'a str,version:u32,digest:[u8;32],allowed:u8,
                generation:u64,decoder:Decoder<'a>,mailbox:mailbox::Mailbox)->Result<Self,Reject>{
        if generation==0{return Err(Reject::Generation);}
        let binding=policy::Binding::select(Some(codec),allowed,supported,version,schema,digest)
            .map_err(Reject::Binding)?;
        Ok(Self{binding,generation,decoder,mailbox,binding_token:Arc::new(())})
    }
    /// Trusted post-authentication entrypoint. Metadata is an assertion from
    /// the caller, not a proof; never expose this API to an untrusted peer.
    /// Reserve owned wire storage before preflight/copy. Publication rechecks
    /// generation under the mailbox lock, even if restart races validation.
    pub fn admit_after_authentication(&self, meta:&VerifiedMetadata<'_>, bytes:&[u8])->Result<(),Reject>{
        self.binding.matches(meta.codec,meta.version,meta.schema,meta.shape_digest).map_err(Reject::Binding)?;
        if meta.recipient_generation!=self.generation{return Err(Reject::Generation);}
        let mut lease=self.mailbox.reserve(self.generation,bytes.len()).map_err(Reject::Mailbox)?;
        let expansion=match self.decoder {
            Decoder::Cbor(shape,limits)=>cbor::preflight_typed(bytes,limits,shape).map_err(Reject::Cbor)?.expanded,
            Decoder::Protobuf(shape,limits)=>proto::preflight(bytes,shape,limits).map_err(Reject::Proto)?.logical,
        };
        lease.reserve_expansion(expansion).map_err(Reject::Mailbox)?;
        let mut copy=Vec::new();
        copy.try_reserve_exact(bytes.len()).map_err(|_|Reject::Allocation)?;
        copy.extend_from_slice(bytes);
        lease.publish_bound(copy.into_boxed_slice(),&self.binding_token).map_err(Reject::Mailbox)
    }
    /// Receiver-side host materialization. Only frames admitted by this
    /// endpoint instance are consumed. A second endpoint must not decode
    /// a frame using a different descriptor, even at the same generation.
    /// The final generation check linearizes delivery eligibility, but
    /// actual actor dispatch must fence generation under its runtime lock.
    pub fn receive_materialized(&self)->Result<Option<MaterializedDelivery>,Reject> {
        let Some(delivery)=self.mailbox.pop_bound(self.generation,&self.binding_token).map_err(Reject::Mailbox)? else {
            return Ok(None);
        };
        let credit=delivery.expansion_credit();
        if credit==0 { return Err(Reject::Expansion); }
        let value=match self.decoder {
            Decoder::Cbor(shape,mut limits)=>{
                limits.expanded=limits.expanded.min(credit);
                let (value,usage)=cbor::decode_typed(delivery.bytes(),limits,shape).map_err(Reject::Cbor)?;
                if usage.expanded!=credit { return Err(Reject::Expansion); }
                MaterializedValue::Cbor(value)
            }
            Decoder::Protobuf(shape,mut limits)=>{
                limits.logical=limits.logical.min(credit);
                let (value,usage)=proto::decode(delivery.bytes(),shape,limits).map_err(Reject::Proto)?;
                if usage.logical!=credit { return Err(Reject::Expansion); }
                MaterializedValue::Protobuf(value)
            }
        };
        self.mailbox.check_generation(self.generation).map_err(Reject::Mailbox)?;
        Ok(Some(MaterializedDelivery{value,delivery,generation:self.generation}))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const H:[u8;32]=[7;32];
    fn cl()->cbor::Limits{cbor::Limits{wire:64,expanded:1024,nodes:16,elements:16,work:128,depth:4}}
    fn pl()->proto::Limits{proto::Limits{wire:64,logical:1024,work:128,fields:16,messages:16,depth:4}}
    fn meta(codec:&str)->VerifiedMetadata<'_>{VerifiedMetadata{codec,version:1,schema:"order.v1",shape_digest:&H,recipient_generation:1}}
    #[test] fn fixed_descriptor_and_metadata_before_payload(){
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let fields=[cbor::Shape::Bool,cbor::Shape::I64];let shape=cbor::Shape::Record(&fields);
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        for bad in [
            VerifiedMetadata{codec:"protobuf",..meta("cbor")},
            VerifiedMetadata{schema:"other.v1",..meta("cbor")},
            VerifiedMetadata{version:2,..meta("cbor")},
            VerifiedMetadata{shape_digest:&[8;32],..meta("cbor")},
        ]{
            assert_eq!(e.admit_after_authentication(&bad,&[0xff]),Err(Reject::Binding(policy::Reject::Mismatch)));
            assert_eq!(m.usage().unwrap(),(0,0));
        }
        assert_eq!(e.admit_after_authentication(&meta("cbor"),&[0x82,0x01,0xf5]),Err(Reject::Cbor(cbor::Reject::Shape)));
        assert_eq!(m.usage().unwrap(),(0,0));
        let b=[0x82,0xf5,1];e.admit_after_authentication(&meta("cbor"),&b).unwrap();
        assert_eq!(m.pop(1).unwrap().unwrap().bytes(),&b);
    }
    #[test] fn protobuf_nested_reject_rolls_back_shared_credit(){
        let tenant=mailbox::TenantBudget::new(16,1).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,16,1,tenant.clone()).unwrap();
        let child=proto::Schema{fields:&[proto::Field{number:1,kind:proto::Kind::Varint}]};
        let root=proto::Schema{fields:&[proto::Field{number:1,kind:proto::Kind::Message(&child)}]};
        let e=Endpoint::protobuf("order.v1",1,H,2,1,&root,pl(),m.clone()).unwrap();
        assert_eq!(e.admit_after_authentication(&meta("protobuf"),&[0x0a,1,0xff]),Err(Reject::Proto(proto::Reject::Truncated)));
        assert_eq!(tenant.usage().unwrap(),(0,0));
        let b=[0x0a,2,8,1];e.admit_after_authentication(&meta("protobuf"),&b).unwrap();
        let d=m.pop(1).unwrap().unwrap();assert_eq!(d.bytes(),&b);
        assert_eq!(tenant.usage().unwrap(),(4,1));
        assert_eq!(e.admit_after_authentication(&meta("protobuf"),&b),Err(Reject::Mailbox(mailbox::Reject::Full)));
        drop(d);assert_eq!(tenant.usage().unwrap(),(0,0));
    }
    #[test] fn generation_and_policy_fail_closed(){
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let shape=cbor::Shape::Bool;
        assert!(matches!(Endpoint::cbor("order.v1",1,H,2,1,&shape,cl(),m.clone()),Err(Reject::Binding(policy::Reject::Denied))));
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        assert_eq!(e.admit_after_authentication(&VerifiedMetadata{recipient_generation:2,..meta("cbor")},&[0xf5]),Err(Reject::Generation));
        m.restart(1).unwrap();
        assert_eq!(e.admit_after_authentication(&meta("cbor"),&[0xf5]),Err(Reject::Mailbox(mailbox::Reject::Stale)));
        assert!(m.pop(2).unwrap().is_none());
    }
    #[test] fn cross_codec_expansion_budget_follows_wire_delivery(){
        let shape=cbor::Shape::Bool;
        let schema=proto::Schema{fields:&[proto::Field{number:1,kind:proto::Kind::Varint}]};
        let cb=[0xf5]; let pb=[8,1];
        let c=cbor::preflight_typed(&cb,cl(),&shape).unwrap().expanded;
        let p=proto::preflight(&pb,&schema,pl()).unwrap().logical;
        let tenant=mailbox::TenantBudget::new_with_expansion(64,4,c+p-1).unwrap();
        let a=mailbox::Mailbox::new_with_tenant(1,64,2,tenant.clone()).unwrap();
        let b=mailbox::Mailbox::new_with_tenant(1,64,2,tenant.clone()).unwrap();
        let ec=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),a.clone()).unwrap();
        let ep=Endpoint::protobuf("order.v1",1,H,2,1,&schema,pl(),b.clone()).unwrap();
        ec.admit_after_authentication(&meta("cbor"),&cb).unwrap();
        let delivery=a.pop(1).unwrap().unwrap();
        assert_eq!(delivery.expansion_credit(),c);
        assert_eq!(ep.admit_after_authentication(&meta("protobuf"),&pb),Err(Reject::Mailbox(mailbox::Reject::Full)));
        assert_eq!(b.usage().unwrap(),(0,0));
        assert_eq!(tenant.expansion_usage().unwrap(),c);
        drop(delivery);
        ep.admit_after_authentication(&meta("protobuf"),&pb).unwrap();
        assert_eq!(tenant.expansion_usage().unwrap(),p);
        b.close().unwrap();
        assert_eq!(tenant.expansion_usage().unwrap(),0);
        assert_eq!(tenant.usage().unwrap(),(0,0));
    }

    #[test] fn materialization_keeps_shared_expansion_and_wire_credit(){
        let shape=cbor::Shape::Bool;
        let b=[0xf5];
        let cost=cbor::preflight_typed(&b,cl(),&shape).unwrap().expanded;
        let tenant=mailbox::TenantBudget::new_with_expansion(64,4,cost).unwrap();
        let a=mailbox::Mailbox::new_with_tenant(1,64,2,tenant.clone()).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),a.clone()).unwrap();
        e.admit_after_authentication(&meta("cbor"),&b).unwrap();
        let value=e.receive_materialized().unwrap().unwrap();
        assert_eq!(value.value(),&MaterializedValue::Cbor(cbor::Value::Bool(true)));
        assert_eq!(value.expansion_credit(),cost);
        assert_eq!(value.generation(),1);
        assert_eq!(a.usage().unwrap(),(0,0));
        assert_eq!(tenant.usage().unwrap(),(1,1));
        assert_eq!(tenant.expansion_usage().unwrap(),cost);
        assert_eq!(e.admit_after_authentication(&meta("cbor"),&b),Err(Reject::Mailbox(mailbox::Reject::Full)));
        a.restart(1).unwrap();
        assert!(matches!(e.receive_materialized(),Err(Reject::Mailbox(mailbox::Reject::Stale))));
        assert_eq!(tenant.expansion_usage().unwrap(),cost);
        drop(value);
        assert_eq!(tenant.expansion_usage().unwrap(),0);
        assert_eq!(tenant.usage().unwrap(),(0,0));
    }
    #[test] fn endpoint_binding_refuses_other_instance_without_dequeue(){
        let m=mailbox::Mailbox::new(1,64,4).unwrap();
        let shape=cbor::Shape::Bool;
        let a=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        let b=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        a.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap();
        assert!(matches!(b.receive_materialized(),Err(Reject::Mailbox(mailbox::Reject::Binding))));
        assert_eq!(m.usage().unwrap(),(1,1));
        assert!(a.receive_materialized().unwrap().is_some());
        assert!(a.receive_materialized().unwrap().is_none());
        m.reserve(1,1).unwrap().publish(vec![0xf5].into_boxed_slice()).unwrap();
        assert!(matches!(a.receive_materialized(),Err(Reject::Mailbox(mailbox::Reject::Binding))));
        assert_eq!(m.usage().unwrap(),(1,1));
    }
    #[test] fn protobuf_owned_value_and_credit(){
        let schema=proto::Schema{fields:&[proto::Field{number:1,kind:proto::Kind::Varint}]};
        let t=mailbox::TenantBudget::new_with_expansion(64,2,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,2,t.clone()).unwrap();
        let e=Endpoint::protobuf("order.v1",1,H,2,1,&schema,pl(),m.clone()).unwrap();
        let b=[8,42];
        let cost=proto::preflight(&b,&schema,pl()).unwrap().logical;
        e.admit_after_authentication(&meta("protobuf"),&b).unwrap();
        let value=e.receive_materialized().unwrap().unwrap();
        assert_eq!(value.value(),&MaterializedValue::Protobuf(vec![proto::FieldValue{number:1,value:proto::Value::Scalar(proto::Scalar::Varint(42))}]));
        assert_eq!(value.expansion_credit(),cost);
        m.close().unwrap();
        assert_eq!(t.expansion_usage().unwrap(),cost);
        drop(value);
        assert_eq!(t.expansion_usage().unwrap(),0);
    }

}
