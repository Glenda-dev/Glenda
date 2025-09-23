use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple spinlock-based mutex for no_std environments.
pub struct SpinMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    /// Creates a new spin mutex with the given data.
    pub const fn new(data: T) -> Self {
        Self { locked: AtomicBool::new(false), data: UnsafeCell::new(data) }
    }

    /// Acquires the lock, spinning until it becomes available.
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinMutexGuard { mutex: self }
    }

    /// Attempts to acquire the lock without spinning.
    /// Returns `Some(guard)` if successful, `None` otherwise.
    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_, T>> {
        if !self.locked.swap(true, Ordering::Acquire) {
            Some(SpinMutexGuard { mutex: self })
        } else {
            None
        }
    }
}

/// A guard that provides exclusive access to the data protected by the spin mutex.
/// The lock is automatically released when the guard is dropped.
pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}

impl<'a, T> Deref for SpinMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}
