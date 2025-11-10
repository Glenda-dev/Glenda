pub mod context;
pub mod process;
pub mod table;
pub use context::ProcContext;
pub use process::Process;
pub use process::ProcState;
pub use table::{alloc, free, init, NPROC};

use crate::hart;
use spin::Mutex;

static CURRENT_USER_SATP: Mutex<Option<usize>> = Mutex::new(None);

pub fn set_current_user_satp(satp: usize) {
    *CURRENT_USER_SATP.lock() = Some(satp);
}

pub fn current_user_satp() -> Option<usize> {
    *CURRENT_USER_SATP.lock()
}

pub fn current_proc() -> &'static mut Process {
    let hart = hart::get();
    unsafe { &mut *hart.proc }
}
