use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};
pub struct SpinLock {
    lock: AtomicBool,
}
impl SpinLock {
    pub const fn new() -> Self {
        SpinLock { lock: AtomicBool::new(false) }
    }
    pub fn lock(&self) {
        // Lock spinning
        while self.lock.swap(true, Ordering::Acquire) {}
        // CPU relaxation
        spin_loop();
    }
    pub fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}
