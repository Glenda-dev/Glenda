pub mod context;
pub mod process;
pub use context::ProcContext;
pub use process::Process;

use spin::Mutex;

static CURRENT_USER_SATP: Mutex<Option<usize>> = Mutex::new(None);

pub fn set_current_user_satp(satp: usize) {
    *CURRENT_USER_SATP.lock() = Some(satp);
}

pub fn current_user_satp() -> Option<usize> {
    *CURRENT_USER_SATP.lock()
}
