use crate::cpu;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Debug)]
pub struct SpinLock<T: ?Sized> {
    locked: AtomicBool,
    cpu_id: AtomicUsize,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

const NO_CPU: usize = usize::MAX;

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            cpu_id: AtomicUsize::new(NO_CPU),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        cpu::push_off();
        if self.holding() {
            panic!(
                "SpinLock::lock: Deadlock on CPU {} (Holder: {})",
                cpu::get().id,
                self.cpu_id.load(Ordering::Relaxed)
            );
        }

        loop {
            if self
                .locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }

        self.cpu_id.store(cpu::get().id, Ordering::Relaxed);

        SpinLockGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        cpu::push_off();
        if self.holding() {
            cpu::pop_off();
            return None;
        }

        if self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            self.cpu_id.store(cpu::get().id, Ordering::Relaxed);
            Some(SpinLockGuard { lock: self })
        } else {
            cpu::pop_off();
            None
        }
    }

    pub fn holding(&self) -> bool {
        if self.locked.load(Ordering::Relaxed)
            && self.cpu_id.load(Ordering::Relaxed) == cpu::get().id
        {
            true
        } else {
            false
        }
    }
}

pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
}

impl<T: ?Sized> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.cpu_id.store(NO_CPU, Ordering::Relaxed);
        self.lock.locked.store(false, Ordering::Release);
        cpu::pop_off();
    }
}
