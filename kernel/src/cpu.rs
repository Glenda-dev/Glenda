use crate::hal::cpu;
use crate::hal::proc::ProcContext;
use crate::proc::scheduler::{MAX_PRIORITY, TcbQueue};
use crate::sync::SpinLock;

#[derive(Debug)]
pub struct Cpu {
    pub id: usize,
    pub context: ProcContext,
    pub noff: usize,
    pub intena: bool,
    pub enabled: bool,
    pub ready_queues: SpinLock<[TcbQueue; MAX_PRIORITY]>,
    pub last_tick_time: usize,
}

impl Cpu {
    pub const fn new() -> Self {
        Self {
            id: 0,
            context: ProcContext::new(),
            noff: 0,
            intena: false,
            enabled: false,
            ready_queues: SpinLock::new([const { TcbQueue::new() }; MAX_PRIORITY]),
            last_tick_time: 0,
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

pub fn init() {
    let cpu_id = cpu::cpu_id();
    let cpu = unsafe { &mut CPUS[cpu_id] };
    cpu.id = cpu_id;
    enable(cpu_id);
}

pub fn push_off() {
    let old = crate::hal::irq::is_enabled();
    unsafe {
        crate::hal::irq::disable();
    }
    let cpu = get();
    if cpu.noff == 0 {
        cpu.intena = old;
    }
    cpu.noff += 1;
}

pub fn pop_off() {
    let cpu = get();
    if crate::hal::irq::is_enabled() {
        panic!("pop_off - interruptible");
    }
    if cpu.noff < 1 {
        panic!("pop_off - underflow");
    }
    cpu.noff -= 1;
    if cpu.noff == 0 && cpu.intena {
        unsafe {
            crate::hal::irq::enable();
        }
    }
}
