use super::CapPtr;
use super::CapType;
use super::Capability;
use alloc::collections::BTreeMap;

/// 能力空间 (CSpace)
/// 每个进程拥有一个 CSpace，用于存储它拥有的 Cap
// TDOO: 在完整微内核中通常是 Radix Tree 结构的 CNode 树，这里先用 BTreeMap 模拟扁平结构
#[derive(Default)]
pub struct CSpace {
    slots: BTreeMap<usize, Capability>,
}

impl CSpace {
    pub const fn new() -> Self {
        Self { slots: BTreeMap::new() }
    }

    pub fn get(&self, cptr: CapPtr) -> Option<&Capability> {
        self.slots.get(&cptr)
    }

    pub fn get_mut(&mut self, cptr: CapPtr) -> Option<&mut Capability> {
        self.slots.get_mut(&cptr)
    }

    pub fn insert(&mut self, cptr: CapPtr, cap: Capability) {
        self.slots.insert(cptr, cap);
    }

    pub fn remove(&mut self, cptr: CapPtr) -> Option<Capability> {
        self.slots.remove(&cptr)
    }

    /// 查找空闲槽位 (简单实现)
    pub fn alloc_slot(&self) -> CapPtr {
        let mut i = 0;
        while self.slots.contains_key(&i) {
            i += 1;
        }
        i
    }
}
