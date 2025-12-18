use super::pmem;

pub struct PhysFrame {
    addr: usize,
}

impl PhysFrame {
    pub fn alloc(for_kernel: bool) -> Option<Self> {
        let pa = pmem::alloc(for_kernel) as usize;
        if pa == 0 { None } else { Some(Self { addr: pa }) }
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
}

impl Drop for PhysFrame {
    fn drop(&mut self) {
        pmem::free(self.addr, true);
    }
}
