use crate::hal::cpu;
use crate::hal::proc::ProcContext;
use crate::proc::scheduler::{MAX_PRIORITY, TcbQueue};
use spin::Mutex;

#[derive(Debug)]
pub struct Cpu {
    pub id: usize,
    pub context: ProcContext,
    pub nest_count: usize,
    pub enabled: bool,
    pub ready_queues: Mutex<[TcbQueue; MAX_PRIORITY]>,
}

impl Cpu {
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

pub static mut CPUS: [Cpu; cpu::MAX_CPUS] = [const { Cpu::new() }; cpu::MAX_CPUS];

pub fn get() -> &'static mut Cpu {
    unsafe { &mut CPUS[cpu::cpu_id()] }
}

pub fn enable(cpu_id: usize) {
    let cpu = unsafe { &mut CPUS[cpu_id] };
    cpu.enabled = true;
}

pub fn init(cpu_id: usize) {
    let cpu = unsafe { &mut CPUS[cpu_id] };
    cpu.id = cpu_id;
    enable(cpu_id);
}
