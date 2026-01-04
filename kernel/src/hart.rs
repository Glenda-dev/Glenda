use crate::proc::ProcContext;
use crate::proc::scheduler::{MAX_PRIORITY, TcbQueue};
use core::arch::asm;
use spin::Mutex;

pub const MAX_HARTS: usize = 8;

#[derive(Debug)]
pub struct Hart {
    pub id: usize,
    pub context: ProcContext,
    pub nest_count: usize,
    pub enabled: bool,
    pub ready_queues: Mutex<[TcbQueue; MAX_PRIORITY]>,
}

impl Hart {
    pub const fn new() -> Self {
        Self {
            id: 0,
            context: ProcContext::new(),
            nest_count: 0,
            enabled: false,
            ready_queues: Mutex::new([const { TcbQueue::new() }; MAX_PRIORITY]),
        }
    }
}

pub static mut HARTS: [Hart; MAX_HARTS] = [const { Hart::new() }; MAX_HARTS];

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

pub fn enable(hartid: usize) {
    let hart = unsafe { &mut HARTS[hartid] };
    hart.enabled = true;
}

pub fn init(hartid: usize) {
    let hart = unsafe { &mut HARTS[hartid] };
    hart.id = hartid;
    enable(hartid);
}
