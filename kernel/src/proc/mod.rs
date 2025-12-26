pub mod context;
pub mod payload;
pub mod roottask;
pub mod scheduler;
pub mod thread;

pub use context::ProcContext;
pub use thread::{TCB, ThreadState};

pub fn init(hartid: usize, _dtb: *const u8) {
    payload::init();
    if hartid == 0 {
        roottask::init();
    }
}
