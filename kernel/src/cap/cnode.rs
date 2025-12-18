use super::Capability;
use crate::mem::PhysFrame;
// TODO: Impl
pub struct CNode {}

impl CNode {
    pub fn new() -> Self {
        CNode {}
    }
    pub fn insert(&mut self, _slot: usize, _cap: Capability) -> Option<usize> {
        None
    }
    pub fn from_frame(_frame: &PhysFrame) -> Self {
        CNode {}
    }
}
