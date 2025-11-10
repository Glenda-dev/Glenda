use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Mutex;

use super::process::ProcState;
use super::{ProcContext, Process};

use riscv::asm::wfi;

pub const NPROC: usize = 64;

static PROCS_LOCK: Mutex<()> = Mutex::new(());
static GLOBAL_PID: AtomicUsize = AtomicUsize::new(1);
static mut PROCS: [Process; NPROC] = [Process::new(); NPROC];

#[unsafe(no_mangle)]
extern "C" fn proc_return() -> ! {
    // TODO: Implement this
    loop {
        wfi();
    }
}

pub fn init() {
    let _g = PROCS_LOCK.lock();
    for i in 0..NPROC {
        let p = unsafe { &mut PROCS[i] };
        *p = Process::new();
    }
    GLOBAL_PID.store(1, Ordering::SeqCst);
}

pub fn alloc() -> Option<&'static mut Process> {
    let _g = PROCS_LOCK.lock();
    for i in 0..NPROC {
        let p = unsafe { &mut PROCS[i] };
        if p.state == ProcState::Unused {
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
    let _g = PROCS_LOCK.lock();
    // TODO: Implement this
    *p = Process::new();
}
