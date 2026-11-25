//! Dependency-free HOST reference for cumulative decoder resource accounting.
//! Not a mailbox credit allocator, decoder, or authority gate.
//! Call charge BEFORE allocation, recursion or parser work; no refund on failure.
//! Limits must be selected by trusted runtime policy (never from incoming bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits { pub wire: u64, pub expanded: u64, pub nodes: u64, pub elements: u64, pub work: u64, pub depth: u32 }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Charge { pub wire: u64, pub expanded: u64, pub nodes: u64, pub elements: u64, pub work: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject { InvalidLimits, OverflowOrLimit, Depth, Poisoned }
pub struct Meter { limits: Limits, used: Charge, poisoned: bool }
impl Meter {
    pub fn new(limits: Limits) -> Result<Self, Reject> {
        if limits.wire == 0 || limits.expanded == 0 || limits.nodes == 0 ||
           limits.elements == 0 || limits.work == 0 || limits.depth == 0 {
            return Err(Reject::InvalidLimits);
        }
        Ok(Self { limits, used: Charge::default(), poisoned: false })
    }
    pub fn used(&self) -> Charge { self.used }
    fn add(a: u64, b: u64, cap: u64) -> Option<u64> { a.checked_add(b).filter(|v| *v <= cap) }
    /// One atomic multi-dimensional accounting transaction. Any rejected charge
    /// poisons this per-message meter; failed parses cannot retry with reset fuel.
    pub fn charge(&mut self, c: Charge, depth: u32) -> Result<(), Reject> {
        if self.poisoned { return Err(Reject::Poisoned); }
        if depth > self.limits.depth { self.poisoned = true; return Err(Reject::Depth); }
        let next = (|| Some(Charge {
            wire: Self::add(self.used.wire,c.wire,self.limits.wire)?,
            expanded: Self::add(self.used.expanded,c.expanded,self.limits.expanded)?,
            nodes: Self::add(self.used.nodes,c.nodes,self.limits.nodes)?,
            elements: Self::add(self.used.elements,c.elements,self.limits.elements)?,
            work: Self::add(self.used.work,c.work,self.limits.work)?,
        }))();
        match next {
            Some(next) => { self.used = next; Ok(()) }
            None => { self.poisoned = true; Err(Reject::OverflowOrLimit) }
        }
    }
    /// Checked array byte preflight, including multiplication overflow.
    /// Count is total collection elements, not a per-container exemption.
    pub fn array(&mut self, count: u64, element_bytes: u64, depth: u32) -> Result<(), Reject> {
        if self.poisoned { return Err(Reject::Poisoned); }
        if let Some(expanded) = count.checked_mul(element_bytes) {
            self.charge(Charge { expanded, elements: count, nodes: count, work: count.max(1), ..Charge::default() }, depth)
        } else { self.poisoned = true; Err(Reject::OverflowOrLimit) }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const L: Limits = Limits { wire: 16, expanded: 32, nodes: 8, elements: 8, work: 20, depth: 3 };
    #[test] fn exact_caps() {
        let mut m=Meter::new(L).unwrap();
        assert_eq!(m.charge(Charge{wire:16,expanded:32,nodes:8,elements:8,work:20},3),Ok(()));
        assert_eq!(m.charge(Charge{work:1,..Charge::default()},0),Err(Reject::OverflowOrLimit));
        assert_eq!(m.charge(Charge::default(),0),Err(Reject::Poisoned));
    }
    #[test] fn no_partial_charge() {
        let mut m=Meter::new(L).unwrap();
        m.array(2,4,1).unwrap();
        let prev=m.used();
        assert_eq!(m.charge(Charge{wire:2,expanded:100,..Charge::default()},1),Err(Reject::OverflowOrLimit));
        assert_eq!(m.used(),prev);
    }
    #[test] fn overflow_and_depth() {
        let mut m=Meter::new(L).unwrap();
        assert_eq!(m.array(u64::MAX,2,1),Err(Reject::OverflowOrLimit));
        assert_eq!(m.used(),Charge::default());
        let mut m=Meter::new(L).unwrap();
        assert_eq!(m.charge(Charge::default(),4),Err(Reject::Depth));
        assert_eq!(m.array(1,1,1),Err(Reject::Poisoned));
    }
    #[test] fn cumulative_collections_and_empty_work() {
        let mut m=Meter::new(L).unwrap();
        m.array(4,1,1).unwrap(); m.array(4,1,1).unwrap();
        assert_eq!(m.array(1,1,1),Err(Reject::OverflowOrLimit));
        let mut m=Meter::new(L).unwrap();
        for _ in 0..20 { m.array(0,0,1).unwrap(); }
        assert_eq!(m.array(0,0,1),Err(Reject::OverflowOrLimit));
    }
    #[test] fn addition_overflow() {
        let mut m=Meter::new(Limits{work:u64::MAX,..L}).unwrap();
        m.charge(Charge{work:u64::MAX,..Charge::default()},0).unwrap();
        assert_eq!(m.charge(Charge{work:1,..Charge::default()},0),Err(Reject::OverflowOrLimit));
    }
}
