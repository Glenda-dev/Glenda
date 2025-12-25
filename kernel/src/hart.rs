use crate::proc::ProcContext;
use core::arch::asm;

pub const MAX_HARTS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Hart {
    pub id: usize,
    pub context: ProcContext,
    pub nest_count: usize,
    pub enabled: bool,
}

impl Hart {
    pub const fn new() -> Self {
        Self { id: 0, context: ProcContext::new(), nest_count: 0, enabled: false }
    }
}

pub static mut HARTS: [Hart; MAX_HARTS] = [Hart::new(); MAX_HARTS];

#[inline(always)]
fn getid() -> usize {
    let mut id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) id);
    }
    id
}

pub fn get() -> &'static mut Hart {
    unsafe { &mut HARTS[getid()] }
}

pub fn enable(hartid: usize) {
    let hart = unsafe { &mut HARTS[hartid] };
    hart.enabled = true;
}

pub fn init(hartid: usize) {
    let hart = unsafe { &mut HARTS[hartid] };
    hart.id = hartid;
    enable(hartid);
}
