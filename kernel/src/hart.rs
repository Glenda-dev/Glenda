use crate::proc::{ProcContext, Process};
use core::arch::asm;
use core::ptr;

pub const MAX_HARTS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Hart {
    pub proc: *mut Process,
    pub context: ProcContext,
    pub nest_count: usize,
    pub enabled: bool,
}

impl Hart {
    pub const fn new() -> Self {
        Self { proc: ptr::null_mut(), context: ProcContext::new(), nest_count: 0, enabled: false }
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

pub fn enable_hart(hartid: usize) {
    let hart = unsafe { &mut HARTS[hartid] };
    hart.enabled = true;
}
