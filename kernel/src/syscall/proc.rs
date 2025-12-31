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

pub fn sys_exec(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let u_argv = ctx.a1;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    // Read path
    let mut path_buf = [0u8; 256];
    if let Err(_) = uvm::copyin_str(pt, &mut path_buf, u_path) { return usize::MAX; }
    let path_len = path_buf.iter().position(|&b| b == 0).unwrap_or(path_buf.len());

    // Read argv (pointers to strings)
    let mut argv = [0usize; 16]; // Max 16 args for now
    let mut argc = 0;
    loop {
        let mut u_arg_ptr = 0usize;
        if let Err(_) = uvm::copyin(pt, unsafe { core::slice::from_raw_parts_mut(&mut u_arg_ptr as *mut usize as *mut u8, 8) }, u_argv + argc * 8) {
            break;
        }
        if u_arg_ptr == 0 || argc >= 15 { break; }
        
        // We could copy strings here, but proc_exec can do it after switching PT if needed, 
        // or we do it now into a kernel buffer. Let's do it now for simplicity.
        // Actually, let's just pass the user pointers and have proc_exec copy them to the NEW stack.
        argv[argc] = u_arg_ptr;
        argc += 1;
    }

    match p.proc_exec(&path_buf[..path_len], &argv[..argc]) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}
