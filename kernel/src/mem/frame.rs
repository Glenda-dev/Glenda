use super::PGSIZE;
use super::addr;
use super::pmem;
pub struct PhysFrame {
    addr: usize,
}

impl PhysFrame {
    pub fn alloc() -> Option<Self> {
        pmem::alloc_frame().map(|addr| Self { addr })
    }
    pub fn addr(&self) -> usize {
        self.addr
    }

    /// Construct a PhysFrame from a raw physical address.
    pub unsafe fn from_raw(addr: usize) -> Self {
        Self { addr }
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.addr as *const T
    }

    pub fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.addr as *mut T
    }

    pub fn va(&self) -> usize {
        addr::phys_to_virt(self.addr)
    }

    pub fn zero(&mut self) {
        unsafe {
            core::ptr::write_bytes(self.as_mut_ptr::<u8>(), 0, PGSIZE);
        }
    }

    /// 消耗 PhysFrame 并返回其物理地址，不触发 Drop
    pub fn leak(self) -> usize {
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
