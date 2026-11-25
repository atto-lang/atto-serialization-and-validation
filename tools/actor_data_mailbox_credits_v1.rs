//! Host reference: bounded per-endpoint DATA mailbox with generation-fenced
//! reservations. No guest, peer authentication or CONTROL. Shared tenant credits cover owned wire bytes.
//! The trusted caller reserves before allocation/decoding and never retains
//! more memory than it reserved. Payload is owned bytes, not decoded objects.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

#[derive(Debug, PartialEq, Eq)]
pub enum Reject { InvalidLimits, Stale, Full, Size, Closed, GenerationExhausted, Allocation, Binding, Busy }
// Shared by all endpoints in one trusted tenant. Never chosen by wire data.
struct TenantState { used_bytes: usize, used_slots: usize, max_bytes: usize, max_slots: usize, used_expansion: usize, max_expansion: usize }
#[derive(Clone)]
pub struct TenantBudget(Arc<Mutex<TenantState>>);
struct TenantLease { tenant: TenantBudget, bytes: usize, expansion: usize }
impl TenantBudget {
    pub fn new(max_bytes:usize,max_slots:usize)->Result<Self,Reject> {
        Self::new_with_expansion(max_bytes,max_slots,256*1024*1024)
    }
    /// Independent logical expansion credits, not physical allocation bytes.
    pub fn new_with_expansion(max_bytes:usize,max_slots:usize,max_expansion:usize)->Result<Self,Reject> {
        if max_bytes==0 || max_bytes>256*1024*1024 || max_slots==0 || max_slots>65536 ||
           max_expansion==0 || max_expansion>256*1024*1024 {
            return Err(Reject::InvalidLimits);
        }
        Ok(Self(Arc::new(Mutex::new(TenantState{used_bytes:0,used_slots:0,max_bytes,max_slots,used_expansion:0,max_expansion}))))
    }
    fn reserve(&self,bytes:usize)->Result<TenantLease,Reject> {
        let mut s=self.0.lock().map_err(|_|Reject::Closed)?;
        if bytes>s.max_bytes-s.used_bytes || s.used_slots==s.max_slots { return Err(Reject::Full); }
        s.used_bytes+=bytes; s.used_slots+=1;
        Ok(TenantLease{tenant:self.clone(),bytes,expansion:0})
    }
    pub fn usage(&self)->Result<(usize,usize),Reject> {
        let s=self.0.lock().map_err(|_|Reject::Closed)?;
        Ok((s.used_bytes,s.used_slots))
    }
    pub fn expansion_usage(&self)->Result<usize,Reject> {
        Ok(self.0.lock().map_err(|_|Reject::Closed)?.used_expansion)
    }
}
impl TenantLease {
    fn reserve_expansion(&mut self, amount:usize)->Result<(),Reject> {
        // One positive charge per lease. No replacement/refund to evade admission.
        if amount==0 || self.expansion!=0 { return Err(Reject::Size); }
        let mut s=self.tenant.0.lock().map_err(|_|Reject::Closed)?;
        if amount>s.max_expansion-s.used_expansion { return Err(Reject::Full); }
        s.used_expansion+=amount;
        self.expansion=amount;
        Ok(())
    }
}
impl Drop for TenantLease {
    fn drop(&mut self) {
        if let Ok(mut s)=self.tenant.0.lock() { s.used_bytes-=self.bytes; s.used_slots-=1; s.used_expansion-=self.expansion; }
    }
}
// Tenant credit follows the owned frame through the queue and receiver delivery.
// Only borrowed access is exposed; a trusted receiver must separately account copies.
pub struct Delivery { frame:Box<[u8]>, binding:Option<Arc<()>>, _tenant:TenantLease }
impl Delivery {
    pub fn bytes(&self)->&[u8] { &self.frame }
    pub fn len(&self)->usize { self.frame.len() }
    pub fn is_empty(&self)->bool { self.frame.is_empty() }
    /// Admission-time logical estimate; no actual heap measurement.
    pub fn expansion_credit(&self)->usize { self._tenant.expansion }
}
struct State {
    generation: u64, closed: bool, bytes: usize, slots: usize,
    max_bytes: usize, max_slots: usize,
    queue: VecDeque<Delivery>,
    tenant: TenantBudget,
}
#[derive(Clone)]
// All clones share the same dispatch gate. The gate is separate from
// mailbox state so a synchronous handler can enqueue without deadlocking.
pub struct Mailbox(Arc<Mutex<State>>, Arc<Mutex<()>>);
pub struct Reservation { mailbox: Mailbox, generation: u64, bytes: usize, live: bool, tenant:Option<TenantLease> }
impl Mailbox {
    pub fn new(generation: u64, max_bytes: usize, max_slots: usize) -> Result<Self, Reject> {
        let tenant=TenantBudget::new(max_bytes,max_slots)?;
        Self::new_with_tenant(generation,max_bytes,max_slots,tenant)
    }
    pub fn new_with_tenant(generation:u64,max_bytes:usize,max_slots:usize,tenant:TenantBudget)->Result<Self,Reject> {
        if generation == 0 || max_bytes == 0 || max_bytes > 16*1024*1024 ||
           max_slots == 0 || max_slots > 4096 { return Err(Reject::InvalidLimits); }
        Ok(Self(Arc::new(Mutex::new(State { generation, closed:false, bytes:0, slots:0,
            max_bytes, max_slots, queue:VecDeque::new(), tenant })),Arc::new(Mutex::new(()))))
    }
    fn lock(&self) -> Result<MutexGuard<'_,State>, Reject> {
        self.0.lock().map_err(|_| Reject::Closed)
    }
    // Acquire the dispatch gate before state. No path holding state may
    // acquire the gate. Busy is explicit; never wait on a running handler.
    fn dispatch_gate(&self)->Result<MutexGuard<'_,()>,Reject> {
        match self.1.try_lock() {
            Ok(guard)=>Ok(guard),
            Err(TryLockError::WouldBlock)=>Err(Reject::Busy),
            Err(TryLockError::Poisoned(_))=>Err(Reject::Closed),
        }
    }
    /// Trusted synchronous host dispatch: excludes restart, close and
    /// another fenced dispatch for this mailbox through callback completion.
    /// No mailbox-state or tenant lock is held while the callback runs.
    /// Panic poisons this gate; only close may recover to discard the queue.
    pub fn with_generation_dispatch<R>(&self,generation:u64,run:impl FnOnce()->R)->Result<R,Reject> {
        let _guard=self.dispatch_gate()?;
        self.check_generation(generation)?;
        Ok(run())
    }
    pub fn reserve(&self, generation: u64, bytes: usize) -> Result<Reservation,Reject> {
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if generation != s.generation { return Err(Reject::Stale); }
        if bytes > 1024*1024 { return Err(Reject::Size); }
        if bytes > s.max_bytes-s.bytes || s.slots == s.max_slots { return Err(Reject::Full); }
        // Lock order is always mailbox -> tenant. Tenant code never enters mailbox.
        let tenant=s.tenant.reserve(bytes)?;
        s.bytes+=bytes; s.slots+=1;
        Ok(Reservation { mailbox:self.clone(), generation, bytes, live:true, tenant:Some(tenant) })
    }
    pub fn usage(&self) -> Result<(usize,usize),Reject> {
        let s=self.lock()?; Ok((s.bytes,s.slots))
    }
    // This model releases endpoint mailbox credit when delivery removes the frame;
    // tenant credit follows delivery; decoded allocations require separate accounting.
    pub fn pop(&self, generation:u64) -> Result<Option<Delivery>,Reject> {
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if generation != s.generation { return Err(Reject::Stale); }
        let value=s.queue.pop_front();
        if let Some(ref v)=value { s.bytes-=v.len(); s.slots-=1; }
        Ok(value)
    }
    /// Trusted in-process binding: a mismatched head remains queued.
    /// Not a cryptographic identity or transport authentication.
    pub fn pop_bound(&self, generation:u64, binding:&Arc<()>)->Result<Option<Delivery>,Reject> {
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if generation != s.generation { return Err(Reject::Stale); }
        if let Some(v)=s.queue.front() {
            if !v.binding.as_ref().is_some_and(|b| Arc::ptr_eq(b,binding)) {
                return Err(Reject::Binding);
            }
        }
        let value=s.queue.pop_front();
        if let Some(ref v)=value { s.bytes-=v.len(); s.slots-=1; }
        Ok(value)
    }
    /// A successful check is an observation, not a lock held across actor execution.
    pub fn check_generation(&self, generation:u64)->Result<(),Reject> {
        let s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if generation!=s.generation { return Err(Reject::Stale); }
        Ok(())
    }
    // Pending old-generation reservations remain charged until dropped; never
    // reset counters under in-flight work. Queued owned frames are discarded.
    pub fn restart(&self, generation:u64) -> Result<u64,Reject> {
        let _guard=self.dispatch_gate()?;
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if s.generation != generation { return Err(Reject::Stale); }
        let next=s.generation.checked_add(1).ok_or(Reject::GenerationExhausted)?;
        while let Some(v)=s.queue.pop_front() { s.bytes-=v.len(); s.slots-=1; }
        s.generation=next; Ok(next)
    }
    pub fn close(&self) -> Result<(),Reject> {
        // A panicked handler cannot restart the mailbox. Cleanup may
        // recover the poisoned gate, but never run another handler.
        let _guard=match self.1.try_lock() {
            Ok(guard)=>guard,
            Err(TryLockError::WouldBlock)=>return Err(Reject::Busy),
            Err(TryLockError::Poisoned(error))=>error.into_inner(),
        };
        let mut s=self.lock()?; s.closed=true;
        while let Some(v)=s.queue.pop_front() { s.bytes-=v.len(); s.slots-=1; }
        Ok(())
    }
}
impl Reservation {
    /// Reserve preflight's logical expansion charge before copying/publishing.
    /// The lock order remains mailbox -> tenant; restart cannot mint credit.
    pub fn reserve_expansion(&mut self, amount:usize)->Result<(),Reject> {
        let s=self.mailbox.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if s.generation!=self.generation { return Err(Reject::Stale); }
        self.tenant.as_mut().ok_or(Reject::Closed)?.reserve_expansion(amount)
    }
    // Consumes non-cloneable lease. Failure drops it after lock release.
    pub fn publish(self, frame:Box<[u8]>) -> Result<(),Reject> {
        self.publish_inner(frame,None)
    }
    pub fn publish_bound(self, frame:Box<[u8]>, binding:&Arc<()>) -> Result<(),Reject> {
        self.publish_inner(frame,Some(binding.clone()))
    }
    fn publish_inner(mut self, frame:Box<[u8]>, binding:Option<Arc<()>>) -> Result<(),Reject> {
        if frame.len()!=self.bytes { return Err(Reject::Size); }
        {
            let mut s=self.mailbox.lock()?;
            if s.closed { return Err(Reject::Closed); }
            if s.generation!=self.generation { return Err(Reject::Stale); }
            s.queue.try_reserve(1).map_err(|_|Reject::Allocation)?;
            let tenant=self.tenant.take().ok_or(Reject::Closed)?;
            s.queue.push_back(Delivery { frame, binding, _tenant:tenant });
            self.live=false; // credit now belongs to queue entry
        }
        Ok(())
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if !self.live { return; }
        // Poisoned state rejects new activity. Do not unwind from Drop.
        if let Ok(mut s)=self.mailbox.lock() {
            s.bytes-=self.bytes; s.slots-=1;
        }
        self.live=false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(n:usize)->Box<[u8]> { vec![0;n].into_boxed_slice() }
    #[test] fn limits_and_cancel() {
        let m=Mailbox::new(1,8,2).unwrap();
        let a=m.reserve(1,8).unwrap(); let b=m.reserve(1,0).unwrap();
        assert!(matches!(m.reserve(1,0),Err(Reject::Full)));
        drop(a); drop(b); assert_eq!(m.usage().unwrap(),(0,0));
    }
    #[test] fn publication_and_pop() {
        let m=Mailbox::new(1,8,2).unwrap();
        m.reserve(1,4).unwrap().publish(frame(4)).unwrap();
        assert_eq!(m.usage().unwrap(),(4,1));
        assert_eq!(m.pop(1).unwrap().unwrap().len(),4);
        assert_eq!(m.usage().unwrap(),(0,0));
        assert!(m.pop(1).unwrap().is_none());
    }
    #[test] fn wrong_size_rollback() {
        let m=Mailbox::new(1,8,2).unwrap();
        assert_eq!(m.reserve(1,4).unwrap().publish(frame(3)),Err(Reject::Size));
        assert_eq!(m.usage().unwrap(),(0,0));
    }
    #[test] fn stale_lease_and_generation() {
        let m=Mailbox::new(1,8,2).unwrap();
        let pending=m.reserve(1,4).unwrap();
        m.reserve(1,4).unwrap().publish(frame(4)).unwrap();
        assert_eq!(m.restart(1),Ok(2));
        assert_eq!(m.usage().unwrap(),(4,1));
        assert!(matches!(m.reserve(1,0),Err(Reject::Stale)));
        assert_eq!(pending.publish(frame(4)),Err(Reject::Stale));
        assert_eq!(m.usage().unwrap(),(0,0));
        assert!(m.pop(2).unwrap().is_none());
    }
    #[test] fn close_and_exhaustion() {
        let m=Mailbox::new(u64::MAX,8,2).unwrap();
        assert_eq!(m.restart(u64::MAX),Err(Reject::GenerationExhausted));
        let a=m.reserve(u64::MAX,4).unwrap();
        m.close().unwrap(); m.close().unwrap();
        assert_eq!(a.publish(frame(4)),Err(Reject::Closed));
        assert_eq!(m.usage().unwrap(),(0,0));
    }
    #[test] fn shared_tenant_budget_survives_delivery_and_restart() {
        let t=TenantBudget::new(8,2).unwrap();
        let a=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        let b=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        a.reserve(1,8).unwrap().publish(frame(8)).unwrap();
        assert!(matches!(b.reserve(1,1),Err(Reject::Full)));
        let delivery=a.pop(1).unwrap().unwrap();
        assert_eq!(delivery.bytes().len(),8);
        assert_eq!(a.usage().unwrap(),(0,0));
        assert_eq!(t.usage().unwrap(),(8,1));
        a.restart(1).unwrap();
        assert!(matches!(b.reserve(1,1),Err(Reject::Full)));
        drop(delivery);
        let pending=b.reserve(1,8).unwrap();
        b.restart(1).unwrap();
        assert_eq!(t.usage().unwrap(),(8,1));
        drop(pending);
        assert_eq!(t.usage().unwrap(),(0,0));
    }
    #[test] fn tenant_slots_and_queue_close() {
        let t=TenantBudget::new(8,1).unwrap();
        let a=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        let b=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        a.reserve(1,0).unwrap().publish(frame(0)).unwrap();
        assert!(matches!(b.reserve(1,0),Err(Reject::Full)));
        a.close().unwrap();
        assert_eq!(t.usage().unwrap(),(0,0));
        assert!(b.reserve(1,0).is_ok());
    }
    #[test] fn concurrent_credit_race() {
        let m=Mailbox::new(1,8,1).unwrap();
        let a=m.reserve(1,8).unwrap();
        let other=m.clone();
        assert!(std::thread::spawn(move|| matches!(other.reserve(1,1),Err(Reject::Full))).join().unwrap());
        drop(a); assert_eq!(m.usage().unwrap(),(0,0));
    }
    #[test] fn expansion_credit_follows_delivery_and_blocks_sibling() {
        let t=TenantBudget::new_with_expansion(16,4,10).unwrap();
        let a=Mailbox::new_with_tenant(1,16,2,t.clone()).unwrap();
        let b=Mailbox::new_with_tenant(1,16,2,t.clone()).unwrap();
        let mut pending=a.reserve(1,1).unwrap();
        pending.reserve_expansion(10).unwrap();
        assert_eq!(pending.reserve_expansion(1),Err(Reject::Size));
        pending.publish(frame(1)).unwrap();
        let delivery=a.pop(1).unwrap().unwrap();
        assert_eq!(delivery.expansion_credit(),10);
        assert_eq!(a.usage().unwrap(),(0,0));
        a.restart(1).unwrap();
        let mut other=b.reserve(1,1).unwrap();
        assert_eq!(other.reserve_expansion(1),Err(Reject::Full));
        drop(delivery);
        other.reserve_expansion(10).unwrap();
        drop(other);
        assert_eq!(t.expansion_usage().unwrap(),0);
        assert_eq!(t.usage().unwrap(),(0,0));
    }
    #[test] fn expansion_stale_and_wrong_size_rollback() {
        let t=TenantBudget::new_with_expansion(8,2,16).unwrap();
        let a=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        let mut pending=a.reserve(1,4).unwrap();
        pending.reserve_expansion(16).unwrap();
        a.restart(1).unwrap();
        assert_eq!(pending.publish(frame(4)),Err(Reject::Stale));
        assert_eq!(t.expansion_usage().unwrap(),0);
        let mut pending=a.reserve(2,4).unwrap();
        pending.reserve_expansion(16).unwrap();
        assert_eq!(pending.publish(frame(3)),Err(Reject::Size));
        assert_eq!(t.expansion_usage().unwrap(),0);
        assert_eq!(t.usage().unwrap(),(0,0));
    }
    #[test] fn expansion_close_clears_queued_credit_not_delivered_credit() {
        let t=TenantBudget::new_with_expansion(8,2,16).unwrap();
        let a=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        for _ in 0..2 {
            let mut pending=a.reserve(1,1).unwrap();
            pending.reserve_expansion(8).unwrap();
            pending.publish(frame(1)).unwrap();
        }
        let delivery=a.pop(1).unwrap().unwrap();
        a.close().unwrap();
        assert_eq!(t.expansion_usage().unwrap(),8);
        drop(delivery);
        assert_eq!(t.expansion_usage().unwrap(),0);
    }
    #[test] fn expansion_concurrent_admission() {
        let t=TenantBudget::new_with_expansion(8,4,8).unwrap();
        let a=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        let b=Mailbox::new_with_tenant(1,8,2,t.clone()).unwrap();
        let mut first=a.reserve(1,1).unwrap();
        first.reserve_expansion(8).unwrap();
        assert_eq!(std::thread::spawn(move|| {
            let mut other=b.reserve(1,1).unwrap();
            other.reserve_expansion(1)
        }).join().unwrap(),Err(Reject::Full));
        drop(first);
        assert_eq!(t.expansion_usage().unwrap(),0);
    }

}
