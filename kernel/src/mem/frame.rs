pub struct PhysFrame {
    addr: usize,
}

impl PhysFrame {
    pub fn alloc() -> Option<Self> {
        let pa = crate::mem::pmem::alloc(true) as usize;
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
        crate::mem::pmem::free(self.addr, true);
    }
}
