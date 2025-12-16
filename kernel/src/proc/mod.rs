pub mod context;
pub mod payload;
pub mod process;
pub mod scheduler;
pub mod table;
pub use context::ProcContext;
pub use payload::ProcPayload;
pub use process::ProcState;
pub use process::Process;

use crate::hart;
use spin::Mutex;

// TODO: Refactor
static CURRENT_USER_SATP: Mutex<Option<usize>> = Mutex::new(None);

pub fn set_current_user_satp(satp: usize) {
    *CURRENT_USER_SATP.lock() = Some(satp);
}

pub fn current_user_satp() -> Option<usize> {
    *CURRENT_USER_SATP.lock()
}

pub fn current_proc() -> &'static mut Process {
    let hart = hart::get();
    if hart.proc.is_null() {
        panic!("current_proc: hart.proc is null");
    }
    unsafe { &mut *hart.proc }
}
