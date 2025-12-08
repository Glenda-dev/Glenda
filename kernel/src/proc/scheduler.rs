use super::ProcContext;
use super::ProcState;
use super::Process;
use super::table::{NPROC, PROC_TABLE};
use crate::hart;
use crate::printk;
use riscv::register::sstatus;

unsafe extern "C" {
    fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext);
}

pub fn scheduler() {
    loop {
        // Avoid deadlocks
        unsafe {
            sstatus::clear_sie();
        }

        let mut found = false;
        for i in 0..NPROC {
            let p_ptr = {
                let mut table = PROC_TABLE.lock();
                let p = &mut table[i];
                if p.state == ProcState::Runnable {
                    p.state = ProcState::Running;
                    p as *mut Process
                } else {
                    core::ptr::null_mut()
                }
            };

            if !p_ptr.is_null() {
                found = true;
                let p = unsafe { &mut *p_ptr };
                let hart = hart::get();
                hart.proc = p_ptr;
                unsafe {
                    switch_context(&mut hart.context, &mut p.context);
                }
                hart.proc = core::ptr::null_mut();
                {
                    let mut table = PROC_TABLE.lock();
                    let p = &mut table[i];
                    if p.state == ProcState::Dying {
                        p.state = ProcState::Zombie;
                    }
                }
            }
        }
        if !found {
            // Enable interrupts to allow timer to fire and wake up processes
            unsafe {
                sstatus::set_sie();
            }
            riscv::asm::wfi();
        }
    }
}

pub fn sched() {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    // 避免 exit 把 Zombie 设成 Runnable
    if p.state == ProcState::Running {
        p.state = ProcState::Runnable;
    }
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn yield_proc() {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    if p.state == ProcState::Running {
        p.state = ProcState::Runnable;
    }
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn stop() {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn wait() -> Option<(usize, i32)> {
    let hart = crate::hart::get();
    let curr_proc = unsafe { &mut *hart.proc };

    loop {
        let mut have_kids = false;
        let mut pid = 0;
        let mut exit_code = 0;
        let mut found_zombie = false;
        let mut zombie_root_pt_pa: usize = 0;
        let mut zombie_kernel_stack_top: usize = 0;
        let mut zombie_trapframe_pa: usize = 0;

        {
            let mut table = PROC_TABLE.lock();
            for i in 0..NPROC {
                let p = &mut table[i];
                if p.parent == (curr_proc as *mut _) {
                    have_kids = true;
                    if p.state == ProcState::Zombie {
                        pid = p.pid;
                        exit_code = p.exit_code;
                        p.free();
                        *p = Process::new();
                        found_zombie = true;
                        break;
                    }
                }
            }
        }

        if found_zombie {
            return Some((pid, exit_code));
        }

        if !have_kids {
            return None;
        }

        sleep(curr_proc as *mut _ as usize);
    }
}

pub fn sleep(channel: usize) {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    p.state = ProcState::Sleeping;
    p.sleep_chan = channel;
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn wakeup(channel: usize) {
    for i in 0..NPROC {
        let mut table = PROC_TABLE.lock();
        let p = &mut table[i];
        if p.state == ProcState::Sleeping && p.sleep_chan == channel {
            p.state = ProcState::Runnable;
            p.sleep_chan = 0;
        }
    }
}
