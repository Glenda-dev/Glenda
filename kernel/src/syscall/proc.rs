use crate::irq::TrapContext;
use crate::irq::timer;
use crate::mem::PageTable;
use crate::mem::uvm;
use crate::proc::{current_proc, scheduler};

pub fn sys_getpid() -> usize {
    current_proc().pid
}

pub fn sys_fork() -> usize {
    let child = current_proc().fork();
    child.pid
}

pub fn sys_exit(ctx: &mut TrapContext) -> usize {
    let code = ctx.a0 as i32;
    let p = current_proc();
    p.exit_code = code;
    p.exit();
    scheduler::yield_proc();
    // Should not reach here
    0
}

pub fn sys_wait(ctx: &mut TrapContext) -> usize {
    let addr = ctx.a0;
    match scheduler::wait() {
        Some((pid, code)) => {
            if addr != 0 {
                let p = current_proc();
                let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
                let bytes = code.to_ne_bytes();
                let _ = uvm::copyout(pt, addr, &bytes);
            }
            pid
        }
        None => usize::MAX,
    }
}

pub fn sys_sleep(ctx: &mut TrapContext) -> usize {
    let ticks = ctx.a0;
    timer::wait(ticks);
    0
}
