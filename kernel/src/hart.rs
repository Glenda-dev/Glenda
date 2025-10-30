use crate::proc::context::ProcContext;
use crate::proc::process::Process;
use core::arch::asm;
use core::ptr;

pub const MAX_HARTS: usize = 8;

unsafe extern "C" {
    fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext) -> !;
}

#[derive(Debug, Clone, Copy)]
pub struct Hart {
    pub proc: *mut Process,
    pub context: ProcContext,
    pub nest_count: usize,
    pub interrupt_state: bool,
    pub enabled: bool,
}

impl Hart {
    pub const fn new() -> Self {
        Self {
            proc: ptr::null_mut(),
            context: ProcContext::new(),
            nest_count: 0,
            interrupt_state: false,
            enabled: false,
        }
    }
}

pub static mut HARTS: [Hart; MAX_HARTS] = [Hart::new(); MAX_HARTS];

#[inline(always)]
pub fn getid() -> usize {
    let mut id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) id);
    }
    id
}

pub fn get() -> &'static mut Hart {
    unsafe { &mut HARTS[getid()] }
}

pub fn switch_process(old: &mut Process, new: &mut Process) {
    unsafe {
        switch_context(&mut old.context, &mut new.context);
    }
}

pub fn enable_hart(hartid: usize) {
    let hart = unsafe { &mut HARTS[hartid] };
    hart.enabled = true;
}
