use super::ProcContext;
use super::ProcState;
use super::process;
use super::table::{NPROC, PROC_TABLE};
use crate::hart;
unsafe extern "C" {
    fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext) -> !;
}

pub fn scheduler() {
    loop {
        for i in 0..NPROC {
            let mut table = PROC_TABLE.lock();
            let p = &mut table[i];
            if p.state != ProcState::Runnable {
                continue;
            }
            p.state = ProcState::Running;
            let hart = hart::get();
            hart.proc = p as *mut _;
            unsafe {
                switch_context(&mut hart.context, &mut p.context);
            }
        }
    }
}

pub fn sched() {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    p.state = ProcState::Runnable;
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn yield_proc() {
    let hart = crate::hart::get();
    let p = unsafe { &mut *hart.proc };
    p.state = ProcState::Runnable;
    unsafe {
        switch_context(&mut p.context, &mut hart.context);
    }
}

pub fn wait() {
    let proc = {
        let hart = crate::hart::get();
        unsafe { &mut *hart.proc }
    };
    for i in 0..NPROC {
        let mut table = PROC_TABLE.lock();
        let p = &mut table[i];
        let cpu = hart::get();
        if p.parent.is_null()
            || unsafe { &*p.parent }.pid != unsafe { cpu.proc.as_ref() }.unwrap().pid
        {
            continue;
        }
        if p.state == ProcState::Zombie {
            // Found a zombie child
            process::free(p);
        }
    }
    let proc_addr = proc as *mut process::Process as usize;
    sleep(proc_addr);
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
