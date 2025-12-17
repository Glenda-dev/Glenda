extern crate alloc;
use super::PGSIZE;
use alloc::alloc::{Layout, alloc, dealloc};

pub struct PhysFrame {
    addr: usize,
}

impl PhysFrame {
    pub fn alloc() -> Option<Self> {
        let layout = Layout::from_size_align(PGSIZE, PGSIZE).ok()?;
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            None
        } else {
            unsafe { core::ptr::write_bytes(ptr, 0, PGSIZE) };
            Some(Self { addr: ptr as usize })
        }
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

    pub fn as_slice<T>(&self, len: usize) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.as_ptr::<T>(), len) }
    }
}

impl Drop for PhysFrame {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(PGSIZE, PGSIZE).unwrap();
        unsafe { dealloc(self.addr as *mut u8, layout) };
    }
}
