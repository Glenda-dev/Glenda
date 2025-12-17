use super::ProcContext;
use super::ProcState;
use super::Process;
use super::table::{NPROC, PROC_TABLE};
use crate::hart;
use crate::printk;
use riscv::register::sstatus;

unsafe extern "C" {
    fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext) -> !;
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
                if p.state == ProcState::Ready {
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
    // 避免 exit 把 Zombie 设成 Ready
    if p.state == ProcState::Running {
        p.state = ProcState::Ready;
    }
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn yield_proc() {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    if p.state == ProcState::Running {
        p.state = ProcState::Ready;
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

        // Disable interrupts to prevent deadlock with ISR using PROC_TABLE
        let sstatus_val = sstatus::read();
        let sie_enabled = sstatus_val.sie();
        unsafe {
            sstatus::clear_sie();
        }

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

            if !found_zombie && have_kids {
                curr_proc.state = ProcState::Sleeping;
                curr_proc.sleep_chan = curr_proc as *mut _ as usize;
            }
        }

        if found_zombie {
            if sie_enabled {
                unsafe {
                    sstatus::set_sie();
                }
            }
            return Some((pid, exit_code));
        }

        if !have_kids {
            if sie_enabled {
                unsafe {
                    sstatus::set_sie();
                }
            }
            return None;
        }

        stop();

        if sie_enabled {
            unsafe {
                sstatus::set_sie();
            }
        }
    }
}

pub fn sleep(channel: usize) {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };

    let sstatus_val = sstatus::read();
    let sie_enabled = sstatus_val.sie();
    unsafe {
        sstatus::clear_sie();
    }

    {
        let _lock = PROC_TABLE.lock();
        p.state = ProcState::Sleeping;
        p.sleep_chan = channel;
    }

    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }

    if sie_enabled {
        unsafe {
            sstatus::set_sie();
        }
    }
}

pub fn wakeup(channel: usize) {
    let sstatus_val = sstatus::read();
    let sie_enabled = sstatus_val.sie();
    unsafe {
        sstatus::clear_sie();
    }

    for i in 0..NPROC {
        let mut table = PROC_TABLE.lock();
        let p = &mut table[i];
        if p.state == ProcState::Sleeping && p.sleep_chan == channel {
            p.state = ProcState::Ready;
            p.sleep_chan = 0;
        }
    }

    if sie_enabled {
        unsafe {
            sstatus::set_sie();
        }
    }
}
