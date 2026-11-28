//! Host composition reference, NOT cryptographic authentication or guest authority.
//! A trusted transport must authenticate the complete metadata and payload together,
//! then call this API with the exact immutable bytes it verified.
#[path="actor_data_codec_policy_v1.rs"] pub mod policy;
#[path="actor_data_cbor_preflight_v1.rs"] pub mod cbor;
#[path="actor_data_protobuf_recursive_v1.rs"] pub mod proto;
#[path="actor_data_mailbox_credits_v1.rs"] pub mod mailbox;

use std::sync::{Arc, Mutex, TryLockError};

#[derive(Debug, PartialEq, Eq)]
pub enum Reject { Binding(policy::Reject), Generation, Cbor(cbor::Reject), Proto(proto::Reject), Mailbox(mailbox::Reject), Allocation, Expansion, Session, Sequence, SequenceExhausted, Work }

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
/// Owned suspended-dispatch state. Field order drops the materialized
/// value and tenant credits before releasing the lifecycle fence.
#[must_use]
pub struct SuspendedDelivery {
    delivery:MaterializedDelivery,
    lease:mailbox::SuspensionLease,
}
impl SuspendedDelivery {
    pub fn cancellation(&self)->mailbox::DispatchCancellation { self.lease.cancellation() }
    pub fn checkpoint(&self)->Result<(),Reject> { self.lease.checkpoint().map_err(Reject::Mailbox) }
    pub fn value(&self)->Result<&MaterializedValue,Reject> {
        self.checkpoint()?;
        Ok(self.delivery.value())
    }
    pub fn expansion_credit(&self)->usize { self.delivery.expansion_credit() }
}
// Reserve one credit per owned wire copy byte, and conservatively reserve
// the same planned materialization work for both passes. Planning charges
// hypothetical writes as well, so this intentionally overestimates ingress.
// The cap is per message, not a tenant rate limit or wall-clock guarantee.
fn pass_work_cap(total:usize,wire:usize)->Result<usize,Reject> {
    if total==0 || total>4*1024*1024 { return Err(Reject::Work); }
    let remaining=total.checked_sub(wire).ok_or(Reject::Work)?;
    let cap=remaining/2;
    if cap==0 { return Err(Reject::Work); }
    Ok(cap)
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
        let (expansion,planned_work)=match self.decoder {
            Decoder::Cbor(shape,mut limits)=>{
                limits.work=pass_work_cap(limits.work,bytes.len())?;
                let usage=cbor::preflight_materialization(bytes,limits,shape).map_err(Reject::Cbor)?;
                (usage.expanded,usage.work)
            }
            Decoder::Protobuf(shape,mut limits)=>{
                limits.work=pass_work_cap(limits.work,bytes.len())?;
                let usage=proto::preflight(bytes,shape,limits).map_err(Reject::Proto)?;
                (usage.logical,usage.work)
            }
        };
        lease.reserve_expansion(expansion).map_err(Reject::Mailbox)?;
        let mut copy=Vec::new();
        copy.try_reserve_exact(bytes.len()).map_err(|_|Reject::Allocation)?;
        copy.extend_from_slice(bytes);
        lease.publish_bound_with_work(copy.into_boxed_slice(),&self.binding_token,planned_work).map_err(Reject::Mailbox)
    }
    /// Synchronous fenced host dispatch. Busy does not dequeue or consume
    /// a frame; restart and close cannot advance while the handler runs.
    /// The handler borrows the materialized value; its lease is dropped
    /// before the gate is released. Copies need independent accounting.
    /// No asynchronous task, guest runtime, or external actor state is fenced.
    pub fn dispatch_next(&self, handler:impl FnOnce(&MaterializedValue))->Result<bool,Reject> {
        self.mailbox.with_generation_dispatch(self.generation, || {
            let Some(delivery)=self.receive_materialized()? else { return Ok(false); };
            handler(delivery.value());
            Ok(true)
        }).map_err(Reject::Mailbox)?
    }

    /// Cooperative suspension adapter for a trusted host scheduler. The
    /// returned object owns both lifecycle and memory-admission leases.
    /// Dropping a cancelled task must drop this object before restart.
    /// No future polling, guest suspension, or forced cancellation is implied.
    pub fn begin_suspended_receive(&self)->Result<Option<SuspendedDelivery>,Reject> {
        let lease=self.mailbox.begin_suspended_dispatch(self.generation).map_err(Reject::Mailbox)?;
        let Some(delivery)=self.receive_materialized()? else { return Ok(None); };
        lease.checkpoint().map_err(Reject::Mailbox)?;
        Ok(Some(SuspendedDelivery{delivery,lease}))
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
        let planned_work=delivery.planned_work();
        if planned_work==0 { return Err(Reject::Work); }
        let value=match self.decoder {
            Decoder::Cbor(shape,mut limits)=>{
                limits.expanded=limits.expanded.min(credit);
                limits.work=limits.work.min(planned_work);
                let (value,usage)=cbor::decode_typed(delivery.bytes(),limits,shape).map_err(Reject::Cbor)?;
                if usage.expanded!=credit { return Err(Reject::Expansion); }
                if usage.work!=planned_work { return Err(Reject::Work); }
                MaterializedValue::Cbor(value)
            }
            Decoder::Protobuf(shape,mut limits)=>{
                limits.logical=limits.logical.min(credit);
                limits.work=limits.work.min(planned_work);
                let (value,usage)=proto::decode(delivery.bytes(),shape,limits).map_err(Reject::Proto)?;
                if usage.logical!=credit { return Err(Reject::Expansion); }
                if usage.work!=planned_work { return Err(Reject::Work); }
                MaterializedValue::Protobuf(value)
            }
        };
        self.mailbox.check_generation(self.generation).map_err(Reject::Mailbox)?;
        Ok(Some(MaterializedDelivery{value,delivery,generation:self.generation}))
    }
}
/// The trusted transport must authenticate session, sequence, all binding
/// metadata and exact payload as one envelope. This is not a MAC verifier.
pub struct SequencedMetadata<'a> {
    pub binding:VerifiedMetadata<'a>,
    pub session:&'a [u8;32],
    pub sequence:u64,
}

/// Strict ordered-session admission. Owns the underlying endpoint and does
/// not expose an unsequenced admission API. A trusted registrar must never
/// recreate an endpoint with a previously used session identity; recovery
/// requires persistent replay state or a fresh authenticated session.
pub struct SequencedEndpoint<'a> {
    endpoint:Endpoint<'a>,
    session:[u8;32],
    next:Mutex<Option<u64>>,
}
impl<'a> SequencedEndpoint<'a> {
    fn register(endpoint:Endpoint<'a>,session:[u8;32])->Result<Self,Reject> {
        if session==[0;32] { return Err(Reject::Session); }
        Ok(Self{endpoint,session,next:Mutex::new(Some(1))})
    }
    #[allow(clippy::too_many_arguments)]
    pub fn cbor(schema:&'a str,version:u32,digest:[u8;32],allowed:u8,
                generation:u64,shape:&'a cbor::Shape<'a>,limits:cbor::Limits,
                mailbox:mailbox::Mailbox,session:[u8;32])->Result<Self,Reject> {
        Self::register(Endpoint::cbor(schema,version,digest,allowed,generation,shape,limits,mailbox)?,session)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn protobuf(schema:&'a str,version:u32,digest:[u8;32],allowed:u8,
                    generation:u64,shape:&'a proto::Schema<'a>,limits:proto::Limits,
                    mailbox:mailbox::Mailbox,session:[u8;32])->Result<Self,Reject> {
        Self::register(Endpoint::protobuf(schema,version,digest,allowed,generation,shape,limits,mailbox)?,session)
    }

    /// A sequence is consumed only after successful mailbox publication.
    /// The replay lock spans bounded preflight/copy/publication; concurrent
    /// admissions fail Busy rather than wait. Nothing acquires this lock
    /// from mailbox or tenant code. A poisoned lock fails closed.
    pub fn admit_after_authentication(&self,meta:&SequencedMetadata<'_>,bytes:&[u8])->Result<(),Reject> {
        if meta.session!=&self.session { return Err(Reject::Session); }
        self.endpoint.binding.matches(meta.binding.codec,meta.binding.version,meta.binding.schema,meta.binding.shape_digest).map_err(Reject::Binding)?;
        if meta.binding.recipient_generation!=self.endpoint.generation { return Err(Reject::Generation); }
        let mut next=match self.next.try_lock() {
            Ok(guard)=>guard,
            Err(TryLockError::WouldBlock)=>return Err(Reject::Mailbox(mailbox::Reject::Busy)),
            Err(TryLockError::Poisoned(_))=>return Err(Reject::Mailbox(mailbox::Reject::Closed)),
        };
        let expected=next.ok_or(Reject::SequenceExhausted)?;
        if meta.sequence!=expected { return Err(Reject::Sequence); }
        self.endpoint.admit_after_authentication(&meta.binding,bytes)?;
        *next=expected.checked_add(1);
        Ok(())
    }
    pub fn dispatch_next(&self,handler:impl FnOnce(&MaterializedValue))->Result<bool,Reject> {
        self.endpoint.dispatch_next(handler)
    }
    pub fn begin_suspended_receive(&self)->Result<Option<SuspendedDelivery>,Reject> {
        self.endpoint.begin_suspended_receive()
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

    #[test] fn dispatch_excludes_restart_close_and_reentry_before_dequeue(){
        let shape=cbor::Shape::Bool;
        let t=mailbox::TenantBudget::new_with_expansion(64,4,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,4,t.clone()).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        for _ in 0..2 { e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap(); }
        assert_eq!(e.dispatch_next(|v| {
            assert_eq!(v,&MaterializedValue::Cbor(cbor::Value::Bool(true)));
            assert_eq!(m.restart(1),Err(mailbox::Reject::Busy));
            assert_eq!(m.close(),Err(mailbox::Reject::Busy));
            assert_eq!(e.dispatch_next(|_| panic!("must not reenter")),Err(Reject::Mailbox(mailbox::Reject::Busy)));
            assert_eq!(m.usage().unwrap(),(1,1));
            assert_eq!(t.usage().unwrap(),(2,2));
            // The gate is not the mailbox state lock. Enqueue can progress.
            e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap();
        }),Ok(true));
        assert_eq!(m.usage().unwrap(),(2,2));
        assert_eq!(t.usage().unwrap(),(2,2));
        assert_eq!(m.restart(1),Ok(2));
        assert_eq!(e.dispatch_next(|_| panic!("stale callback")),Err(Reject::Mailbox(mailbox::Reject::Stale)));
        assert_eq!(t.usage().unwrap(),(0,0));
        assert_eq!(t.expansion_usage().unwrap(),0);
    }
    #[test] fn dispatch_gate_shared_across_threads_and_clones(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap();
        std::thread::scope(|scope| {
            let (entered,ready)=std::sync::mpsc::channel();
            let (release,resume)=std::sync::mpsc::channel();
            let worker=scope.spawn(|| {
                e.dispatch_next(move |_| {
                    entered.send(()).unwrap();
                    resume.recv().unwrap();
                })
            });
            ready.recv().unwrap();
            let other=m.clone();
            assert_eq!(other.restart(1),Err(mailbox::Reject::Busy));
            assert_eq!(other.close(),Err(mailbox::Reject::Busy));
            assert_eq!(e.dispatch_next(|_| panic!("concurrent callback")),Err(Reject::Mailbox(mailbox::Reject::Busy)));
            release.send(()).unwrap();
            assert_eq!(worker.join().unwrap(),Ok(true));
        });
        assert_eq!(m.restart(1),Ok(2));
    }
    #[test] fn panicked_dispatch_drops_value_and_closes_gate(){
        let shape=cbor::Shape::Bool;
        let t=mailbox::TenantBudget::new_with_expansion(64,2,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,2,t.clone()).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        for _ in 0..2 { e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap(); }
        let failed=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _=e.dispatch_next(|_| panic!("synthetic handler panic"));
        }));
        assert!(failed.is_err());
        assert_eq!(t.usage().unwrap(),(1,1));
        assert_eq!(e.dispatch_next(|_| panic!("poisoned callback")),Err(Reject::Mailbox(mailbox::Reject::Closed)));
        assert_eq!(m.restart(1),Err(mailbox::Reject::Closed));
        assert_eq!(m.close(),Ok(()));
        assert_eq!(t.usage().unwrap(),(0,0));
        assert_eq!(t.expansion_usage().unwrap(),0);
    }
    #[test] fn empty_and_wrong_binding_never_run_handler(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        assert_eq!(e.dispatch_next(|_| panic!("empty")),Ok(false));
        m.reserve(1,1).unwrap().publish(vec![0xf5].into_boxed_slice()).unwrap();
        assert_eq!(e.dispatch_next(|_| panic!("wrong binding")),Err(Reject::Mailbox(mailbox::Reject::Binding)));
        assert_eq!(m.usage().unwrap(),(1,1));
        m.close().unwrap();
        assert_eq!(e.dispatch_next(|_| panic!("closed")),Err(Reject::Mailbox(mailbox::Reject::Closed)));
    }

    #[test] fn suspended_cancellation_retains_fence_and_credit_until_drop(){
        let shape=cbor::Shape::Bool;
        let t=mailbox::TenantBudget::new_with_expansion(64,2,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,2,t.clone()).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap();
        let delivery=e.begin_suspended_receive().unwrap().unwrap();
        assert_eq!(delivery.value().unwrap(),&MaterializedValue::Cbor(cbor::Value::Bool(true)));
        let cancel=delivery.cancellation();
        std::thread::spawn(move||cancel.cancel()).join().unwrap();
        assert_eq!(delivery.checkpoint(),Err(Reject::Mailbox(mailbox::Reject::Cancelled)));
        assert!(matches!(delivery.value(),Err(Reject::Mailbox(mailbox::Reject::Cancelled))));
        assert_eq!(m.restart(1),Err(mailbox::Reject::Busy));
        assert_eq!(m.close(),Err(mailbox::Reject::Busy));
        assert_eq!(e.dispatch_next(|_| panic!("must not overlap")),Err(Reject::Mailbox(mailbox::Reject::Busy)));
        assert!(matches!(e.begin_suspended_receive(),Err(Reject::Mailbox(mailbox::Reject::Busy))));
        assert_eq!(t.expansion_usage().unwrap(),delivery.expansion_credit());
        assert_eq!(t.usage().unwrap(),(1,1));
        drop(delivery);
        assert_eq!(t.expansion_usage().unwrap(),0);
        assert_eq!(t.usage().unwrap(),(0,0));
        assert_eq!(m.restart(1),Ok(2));
    }
    #[test] fn suspension_handle_is_per_dispatch_not_per_generation(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        for _ in 0..2 { e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap(); }
        let first=e.begin_suspended_receive().unwrap().unwrap();
        let stale=first.cancellation();
        drop(first);
        let second=e.begin_suspended_receive().unwrap().unwrap();
        stale.cancel();
        assert_eq!(second.checkpoint(),Ok(()));
        // Owned fence is Send; dropping on another thread unblocks lifecycle.
        std::thread::spawn(move||drop(second)).join().unwrap();
        assert_eq!(m.restart(1),Ok(2));
    }
    #[test] fn empty_and_mismatched_suspended_receive_release_fence(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        assert!(e.begin_suspended_receive().unwrap().is_none());
        m.reserve(1,1).unwrap().publish(vec![0xf5].into_boxed_slice()).unwrap();
        assert!(matches!(e.begin_suspended_receive(),Err(Reject::Mailbox(mailbox::Reject::Binding))));
        assert_eq!(m.usage().unwrap(),(1,1));
        assert_eq!(m.restart(1),Ok(2));
        assert!(matches!(e.begin_suspended_receive(),Err(Reject::Mailbox(mailbox::Reject::Stale))));
    }
    #[test] fn synchronous_handler_cannot_start_suspended_dispatch(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,2).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        e.admit_after_authentication(&meta("cbor"),&[0xf5]).unwrap();
        assert_eq!(e.dispatch_next(|_| {
            assert!(matches!(e.begin_suspended_receive(),Err(Reject::Mailbox(mailbox::Reject::Busy))));
        }),Ok(true));
        assert_eq!(m.restart(1),Ok(2));
    }

    const SESSION:[u8;32]=[9;32];
    fn seq_meta(codec:&str,sequence:u64)->SequencedMetadata<'_>{
        SequencedMetadata{binding:meta(codec),session:&SESSION,sequence}
    }
    #[test] fn ordered_replay_rejects_duplicate_gap_and_wrong_session(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,4).unwrap();
        assert!(matches!(SequencedEndpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone(),[0;32]),Err(Reject::Session)));
        let e=SequencedEndpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone(),SESSION).unwrap();
        assert_eq!(e.admit_after_authentication(&seq_meta("cbor",0),&[0xff]),Err(Reject::Sequence));
        assert_eq!(e.admit_after_authentication(&seq_meta("cbor",2),&[0xff]),Err(Reject::Sequence));
        assert_eq!(e.admit_after_authentication(&SequencedMetadata{session:&[8;32],..seq_meta("cbor",1)},&[0xff]),Err(Reject::Session));
        assert_eq!(m.usage().unwrap(),(0,0));
        e.admit_after_authentication(&seq_meta("cbor",1),&[0xf5]).unwrap();
        assert_eq!(e.admit_after_authentication(&seq_meta("cbor",1),&[0xf5]),Err(Reject::Sequence));
        e.admit_after_authentication(&seq_meta("cbor",2),&[0xf4]).unwrap();
        assert_eq!(m.usage().unwrap(),(2,2));
        assert_eq!(e.dispatch_next(|v|assert_eq!(v,&MaterializedValue::Cbor(cbor::Value::Bool(true)))),Ok(true));
        assert_eq!(e.dispatch_next(|v|assert_eq!(v,&MaterializedValue::Cbor(cbor::Value::Bool(false)))),Ok(true));
    }
    #[test] fn sequence_not_consumed_on_malformed_or_full_admission(){
        let shape=cbor::Shape::Bool;
        let t=mailbox::TenantBudget::new_with_expansion(64,1,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,1,t.clone()).unwrap();
        let e=SequencedEndpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone(),SESSION).unwrap();
        assert!(matches!(e.admit_after_authentication(&seq_meta("cbor",1),&[0xff]),Err(Reject::Cbor(_))));
        assert_eq!(t.usage().unwrap(),(0,0));
        e.admit_after_authentication(&seq_meta("cbor",1),&[0xf5]).unwrap();
        assert_eq!(e.admit_after_authentication(&seq_meta("cbor",2),&[0xf5]),Err(Reject::Mailbox(mailbox::Reject::Full)));
        let delivery=e.begin_suspended_receive().unwrap().unwrap();
        assert_eq!(e.admit_after_authentication(&seq_meta("cbor",2),&[0xf5]),Err(Reject::Mailbox(mailbox::Reject::Full)));
        drop(delivery);
        e.admit_after_authentication(&seq_meta("cbor",2),&[0xf5]).unwrap();
        m.close().unwrap();
        assert_eq!(t.usage().unwrap(),(0,0));
    }
    #[test] fn concurrent_duplicate_publishes_at_most_once(){
        let shape=cbor::Shape::Bool;
        let m=mailbox::Mailbox::new(1,64,4).unwrap();
        let e=SequencedEndpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone(),SESSION).unwrap();
        let start=std::sync::Barrier::new(3);
        let results=std::thread::scope(|scope|{
            let a=scope.spawn(||{start.wait();e.admit_after_authentication(&seq_meta("cbor",1),&[0xf5])});
            let b=scope.spawn(||{start.wait();e.admit_after_authentication(&seq_meta("cbor",1),&[0xf5])});
            start.wait();
            [a.join().unwrap(),b.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|r|r.is_ok()).count(),1);
        assert!(results.iter().all(|r|matches!(r,Ok(())|Err(Reject::Sequence)|Err(Reject::Mailbox(mailbox::Reject::Busy)))));
        assert_eq!(m.usage().unwrap(),(1,1));
    }
    #[test] fn exhausted_sequence_never_wraps_and_restart_never_reuses_session(){
        let schema=proto::Schema{fields:&[proto::Field{number:1,kind:proto::Kind::Varint}]};
        let m=mailbox::Mailbox::new(1,64,4).unwrap();
        let e=SequencedEndpoint::protobuf("order.v1",1,H,2,1,&schema,pl(),m.clone(),SESSION).unwrap();
        *e.next.lock().unwrap()=Some(u64::MAX);
        e.admit_after_authentication(&seq_meta("protobuf",u64::MAX),&[8,1]).unwrap();
        assert_eq!(e.admit_after_authentication(&seq_meta("protobuf",u64::MAX),&[8,1]),Err(Reject::SequenceExhausted));
        assert_eq!(e.admit_after_authentication(&seq_meta("protobuf",0),&[8,1]),Err(Reject::SequenceExhausted));
        assert_eq!(m.restart(1),Ok(2));
        let fresh=SequencedEndpoint::protobuf("order.v1",1,H,2,2,&schema,pl(),m.clone(),[10;32]).unwrap();
        let old=SequencedMetadata{binding:VerifiedMetadata{recipient_generation:2,..meta("protobuf")},session:&SESSION,sequence:1};
        assert_eq!(fresh.admit_after_authentication(&old,&[8,1]),Err(Reject::Session));
        let new=SequencedMetadata{session:&[10;32],..old};
        fresh.admit_after_authentication(&new,&[8,1]).unwrap();
        assert_eq!(m.usage().unwrap(),(2,1));
    }

    #[test] fn materialization_work_refused_before_publication_and_sequence_commit(){
        let shape=cbor::Shape::List(&cbor::Shape::Text);
        let bytes=[0x82,0x61,b'a',0x62,b'b',b'c'];
        let t=mailbox::TenantBudget::new_with_expansion(64,4,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,2,t.clone()).unwrap();
        let insufficient=cbor::Limits{work:39,..cl()};
        let e=SequencedEndpoint::cbor("order.v1",1,H,4,1,&shape,insufficient,m.clone(),SESSION).unwrap();
        assert!(cbor::preflight_typed(&bytes,insufficient,&shape).is_ok());
        for _ in 0..2 {
            assert_eq!(e.admit_after_authentication(&seq_meta("cbor",1),&bytes),Err(Reject::Cbor(cbor::Reject::Budget)));
            assert_eq!(m.usage().unwrap(),(0,0));
            assert_eq!(t.usage().unwrap(),(0,0));
            assert_eq!(t.expansion_usage().unwrap(),0);
        }
        let exact=cbor::Limits{work:40,..cl()};
        let ok=SequencedEndpoint::cbor("order.v1",1,H,4,1,&shape,exact,m.clone(),[11;32]).unwrap();
        let metadata=SequencedMetadata{session:&[11;32],..seq_meta("cbor",1)};
        ok.admit_after_authentication(&metadata,&bytes).unwrap();
        assert_eq!(ok.dispatch_next(|v|assert_eq!(v,&MaterializedValue::Cbor(cbor::Value::List(vec![cbor::Value::Text("a".into()),cbor::Value::Text("bc".into())])))),Ok(true));
        assert_eq!(t.usage().unwrap(),(0,0));
        assert_eq!(t.expansion_usage().unwrap(),0);
    }

    #[test] fn cumulative_work_exact_protobuf_boundary_and_rollback(){
        let schema=proto::Schema{fields:&[proto::Field{number:1,kind:proto::Kind::Varint}]};
        let bytes=[8,42];
        let plan=proto::preflight(&bytes,&schema,pl()).unwrap();
        let total=2*plan.work+bytes.len();
        let t=mailbox::TenantBudget::new_with_expansion(64,2,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,2,t.clone()).unwrap();
        let under=Endpoint::protobuf("order.v1",1,H,2,1,&schema,proto::Limits{work:total-1,..pl()},m.clone()).unwrap();
        assert_eq!(under.admit_after_authentication(&meta("protobuf"),&bytes),Err(Reject::Proto(proto::Reject::Budget)));
        assert_eq!(t.usage().unwrap(),(0,0));
        assert_eq!(t.expansion_usage().unwrap(),0);
        let exact=Endpoint::protobuf("order.v1",1,H,2,1,&schema,proto::Limits{work:total,..pl()},m.clone()).unwrap();
        exact.admit_after_authentication(&meta("protobuf"),&bytes).unwrap();
        let delivery=exact.receive_materialized().unwrap().unwrap();
        assert_eq!(delivery.delivery.planned_work(),plan.work);
        assert_eq!(delivery.value(),&MaterializedValue::Protobuf(vec![proto::FieldValue{number:1,value:proto::Value::Scalar(proto::Scalar::Varint(42))}]));
        drop(delivery);
        assert_eq!(t.usage().unwrap(),(0,0));
    }
    #[test] fn cumulative_work_invalid_and_oversize_caps_refused(){
        for (total,wire) in [(0,0),(1,0),(1,2),(2,1),(4*1024*1024+1,0),(usize::MAX,0)] {
            assert_eq!(pass_work_cap(total,wire),Err(Reject::Work));
        }
        assert_eq!(pass_work_cap(40,6),Ok(17));
        assert_eq!(pass_work_cap(39,6),Ok(16));
        assert_eq!(pass_work_cap(4*1024*1024,0),Ok(2*1024*1024));
    }
    #[test] fn unplanned_bound_frame_never_materializes(){
        let shape=cbor::Shape::Bool;
        let t=mailbox::TenantBudget::new_with_expansion(64,2,1024).unwrap();
        let m=mailbox::Mailbox::new_with_tenant(1,64,2,t.clone()).unwrap();
        let e=Endpoint::cbor("order.v1",1,H,4,1,&shape,cl(),m.clone()).unwrap();
        let mut lease=m.reserve(1,1).unwrap();
        lease.reserve_expansion(32).unwrap();
        lease.publish_bound(vec![0xf5].into_boxed_slice(),&e.binding_token).unwrap();
        assert!(matches!(e.receive_materialized(),Err(Reject::Work)));
        assert_eq!(t.usage().unwrap(),(0,0));
        assert_eq!(t.expansion_usage().unwrap(),0);
    }

}
