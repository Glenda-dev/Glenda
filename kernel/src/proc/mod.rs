pub mod context;
pub mod payload;
pub mod roottask;
pub mod scheduler;
pub mod thread;

pub use context::ProcContext;
pub use thread::{TCB, ThreadState};

use crate::hart;

pub fn current() -> &'static mut TCB {
    let hart = hart::get();
    let tcb_ptr = hart.proc;
    unsafe { &mut *tcb_ptr }
}

pub fn init() {
    payload::init();
}
