use crate::cpu;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

/// 读写自旋锁
///
/// 允许多个读者同时访问，或者一个写者独占访问。
/// 策略：写者优先（或者公平策略），这里简单的实现可能偏向读者，
/// 但在配合关中断使用时，由于临界区短，饿死几率较小。
#[derive(Debug)]
pub struct RwLock<T: ?Sized> {
    lock: AtomicUsize,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for RwLock<T> {}
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

const SHARE_SHIFT: usize = 1;
const SHARE_INC: usize = 1 << SHARE_SHIFT;
const WRITE_LOCKED: usize = 1;

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self { lock: AtomicUsize::new(0), data: UnsafeCell::new(data) }
    }
}

impl<T: ?Sized> RwLock<T> {
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        cpu::push_off();
        loop {
            let x = self.lock.load(Ordering::Relaxed);
            // 如果只有读锁或者无锁 (没有 WRITE_LOCKED 位)
            if x & WRITE_LOCKED == 0 {
                // 尝试增加读者计数
                if self
                    .lock
                    .compare_exchange_weak(x, x + SHARE_INC, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            } else {
                core::hint::spin_loop();
            }
        }
        RwLockReadGuard { lock: self }
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        cpu::push_off();
        loop {
            // 尝试从 0 (无锁) 变为 WRITE_LOCKED
            // 这意味着必须等待所有读者退出
            if self
                .lock
                .compare_exchange_weak(0, WRITE_LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
        RwLockWriteGuard { lock: self }
    }
}

pub struct RwLockReadGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.lock.fetch_sub(SHARE_INC, Ordering::Release);
        cpu::pop_off();
    }
}

pub struct RwLockWriteGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.lock.store(0, Ordering::Release);
        cpu::pop_off();
    }
}
