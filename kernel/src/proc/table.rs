use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Mutex;

use super::process::ProcState;
use super::{ProcContext, Process};

use riscv::asm::wfi;

pub const NPROC: usize = 64;

static GLOBAL_PID: AtomicUsize = AtomicUsize::new(1);
static PROC_TABLE: Mutex<[Process; NPROC]> = Mutex::new([Process::new(); NPROC]);

#[unsafe(no_mangle)]
extern "C" fn proc_return() -> ! {
    // TODO: Implement this
    loop {
        wfi();
    }
}

pub fn init() {
    GLOBAL_PID.store(1, Ordering::SeqCst);
}

pub fn alloc() -> Option<&'static mut Process> {
    let mut table = PROC_TABLE.lock();
    for i in 0..NPROC {
        if table[i].state == ProcState::Unused {
            // Take a raw pointer to the slot inside the static table and
            // convert it to a 'static mutable reference (unsafe).
            let p_ptr: *mut Process = &mut table[i] as *mut Process;
            let p: &'static mut Process = unsafe { &mut *p_ptr };

            p.pid = GLOBAL_PID.fetch_add(1, Ordering::SeqCst);
            p.parent = core::ptr::null_mut();
            p.exit_code = 0;
            p.sleep_chan = 0;
            p.context = ProcContext::new();
            p.context.ra = proc_return as usize;
            p.context.sp = 0;
            p.state = ProcState::Runnable;
            return Some(p);
        }
    }
    None
}

pub fn free(p: &mut Process) {
    // TODO: Implement this
    *p = Process::new();
}
