use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct Once<T> {
    state: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send + Sync> Sync for Once<T> {}
unsafe impl<T: Send> Send for Once<T> {}

const INCOMPLETE: usize = 0;
const RUNNING: usize = 1;
const COMPLETE: usize = 2;

impl<T> Once<T> {
    pub const fn new() -> Self {
        Self { state: AtomicUsize::new(INCOMPLETE), data: UnsafeCell::new(MaybeUninit::uninit()) }
    }

    pub fn call_once<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        let mut status = self.state.load(Ordering::Acquire);

        if status == INCOMPLETE {
            match self.state.compare_exchange(
                INCOMPLETE,
                RUNNING,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // We won the race, execute the init function
                    unsafe {
                        (*self.data.get()).write(f());
                    }
                    self.state.store(COMPLETE, Ordering::Release);
                    status = COMPLETE;
                }
                Err(current_state) => {
                    status = current_state;
                }
            }
        }

        loop {
            match status {
                INCOMPLETE => {
                    // Start over if we saw incomplete but failed compare_exchange due to spurious failure?
                    // No, if compare_exchange failed, status is updated.
                    // If we are here, it means we are waiting.
                    unreachable!("Should have handled INCOMPLETE path or won race");
                }
                RUNNING => {
                    spin_loop();
                    status = self.state.load(Ordering::Acquire);
                }
                COMPLETE => {
                    return unsafe { (*self.data.get()).assume_init_ref() };
                }
                _ => unreachable!("Invalid Once state"),
            }
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) == COMPLETE {
            unsafe { Some((*self.data.get()).assume_init_ref()) }
        } else {
            None
        }
    }

    pub fn is_completed(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }
}
