use crate::mem::PhysAddr;
use crate::mem::VirtAddr;

use super::PGSIZE;
use super::pmem;

#[derive(Debug)]
pub struct PhysFrame {
    addr: PhysAddr,
}

impl PhysFrame {
    pub fn alloc() -> Option<Self> {
        pmem::alloc_frame().map(|addr| Self { addr: addr })
    }
    pub fn addr(&self) -> PhysAddr {
        self.addr
    }

    /// Construct a PhysFrame from a raw physical address.
    pub unsafe fn from(addr: PhysAddr) -> Self {
        Self { addr }
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.addr.as_ptr::<T>()
    }

    pub fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.addr.as_mut_ptr::<T>()
    }

    pub fn va(&self) -> VirtAddr {
        self.addr.to_va()
    }

    pub fn zero(&mut self) {
        unsafe {
            core::ptr::write_bytes(self.as_mut_ptr::<u8>(), 0, PGSIZE);
        }
    }

    /// 消耗 PhysFrame 并返回其物理地址，不触发 Drop
    pub fn leak(self) -> PhysAddr {
        let addr = self.addr;
        core::mem::forget(self);
        addr
    }
}

impl Drop for PhysFrame {
    fn drop(&mut self) {
        pmem::free_frame(self.addr);
    }
}
