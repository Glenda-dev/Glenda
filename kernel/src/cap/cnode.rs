use super::Capability;
use crate::mem::PhysFrame;
// TODO: Impl
pub struct CNode {}

impl CNode {
    pub fn new() -> Self {
        unimplemented!()
    }
    pub fn insert(&mut self, _slot: usize, _cap: Capability) -> Option<usize> {
        unimplemented!()
    }
    pub fn from_frame(_frame: &PhysFrame) -> Self {
        unimplemented!()
    }
    pub fn lookup_cap(&self, _slot: usize) -> Option<Capability> {
        unimplemented!()
    }
}
