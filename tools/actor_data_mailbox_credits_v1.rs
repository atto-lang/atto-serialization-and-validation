//! Host reference: bounded per-endpoint DATA mailbox with generation-fenced
//! reservations. No guest, peer authentication, CONTROL or tenant-wide quota.
//! The trusted caller reserves before allocation/decoding and never retains
//! more memory than it reserved. Payload is owned bytes, not decoded objects.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, PartialEq, Eq)]
pub enum Reject { InvalidLimits, Stale, Full, Size, Closed, GenerationExhausted, Allocation }
struct State {
    generation: u64, closed: bool, bytes: usize, slots: usize,
    max_bytes: usize, max_slots: usize,
    queue: VecDeque<Box<[u8]>>,
}
#[derive(Clone)]
pub struct Mailbox(Arc<Mutex<State>>);
pub struct Reservation { mailbox: Mailbox, generation: u64, bytes: usize, live: bool }
impl Mailbox {
    pub fn new(generation: u64, max_bytes: usize, max_slots: usize) -> Result<Self, Reject> {
        if generation == 0 || max_bytes == 0 || max_bytes > 16*1024*1024 ||
           max_slots == 0 || max_slots > 4096 { return Err(Reject::InvalidLimits); }
        Ok(Self(Arc::new(Mutex::new(State { generation, closed:false, bytes:0, slots:0,
            max_bytes, max_slots, queue:VecDeque::new() }))))
    }
    fn lock(&self) -> Result<MutexGuard<'_,State>, Reject> {
        self.0.lock().map_err(|_| Reject::Closed)
    }
    pub fn reserve(&self, generation: u64, bytes: usize) -> Result<Reservation,Reject> {
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if generation != s.generation { return Err(Reject::Stale); }
        if bytes > 1024*1024 { return Err(Reject::Size); }
        if bytes > s.max_bytes-s.bytes || s.slots == s.max_slots { return Err(Reject::Full); }
        s.bytes+=bytes; s.slots+=1;
        Ok(Reservation { mailbox:self.clone(), generation, bytes, live:true })
    }
    pub fn usage(&self) -> Result<(usize,usize),Reject> {
        let s=self.lock()?; Ok((s.bytes,s.slots))
    }
    // This model releases mailbox credit when delivery removes the owned frame;
    // production must separately charge receiver-owned allocations.
    pub fn pop(&self, generation:u64) -> Result<Option<Box<[u8]>>,Reject> {
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if generation != s.generation { return Err(Reject::Stale); }
        let value=s.queue.pop_front();
        if let Some(ref v)=value { s.bytes-=v.len(); s.slots-=1; }
        Ok(value)
    }
    // Pending old-generation reservations remain charged until dropped; never
    // reset counters under in-flight work. Queued owned frames are discarded.
    pub fn restart(&self, generation:u64) -> Result<u64,Reject> {
        let mut s=self.lock()?;
        if s.closed { return Err(Reject::Closed); }
        if s.generation != generation { return Err(Reject::Stale); }
        let next=s.generation.checked_add(1).ok_or(Reject::GenerationExhausted)?;
        while let Some(v)=s.queue.pop_front() { s.bytes-=v.len(); s.slots-=1; }
        s.generation=next; Ok(next)
    }
    pub fn close(&self) -> Result<(),Reject> {
        let mut s=self.lock()?; s.closed=true;
        while let Some(v)=s.queue.pop_front() { s.bytes-=v.len(); s.slots-=1; }
        Ok(())
    }
}
impl Reservation {
    // Consumes non-cloneable lease. Failure drops it after lock release.
    pub fn publish(mut self, frame:Box<[u8]>) -> Result<(),Reject> {
        if frame.len()!=self.bytes { return Err(Reject::Size); }
        {
            let mut s=self.mailbox.lock()?;
            if s.closed { return Err(Reject::Closed); }
            if s.generation!=self.generation { return Err(Reject::Stale); }
            s.queue.try_reserve(1).map_err(|_|Reject::Allocation)?;
            s.queue.push_back(frame);
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
    #[test] fn concurrent_credit_race() {
        let m=Mailbox::new(1,8,1).unwrap();
        let a=m.reserve(1,8).unwrap();
        let other=m.clone();
        assert!(std::thread::spawn(move|| matches!(other.reserve(1,1),Err(Reject::Full))).join().unwrap());
        drop(a); assert_eq!(m.usage().unwrap(),(0,0));
    }
}
