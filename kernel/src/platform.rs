use crate::mem::PhysAddr;

#[derive(Debug, Clone, Copy)]
pub struct MemoryRange {
    pub start: PhysAddr,
    pub size: usize,
}

impl MemoryRange {
    pub fn from(start: PhysAddr, size: usize) -> MemoryRange {
        MemoryRange { start, size }
    }
    pub fn end(&self) -> PhysAddr {
        self.start + self.size
    }
    pub fn empty() -> Self {
        Self { start: PhysAddr::from(0), size: 0 }
    }
}
