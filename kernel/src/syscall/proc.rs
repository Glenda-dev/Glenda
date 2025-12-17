use crate::irq::TrapContext;
use crate::proc::scheduler;

pub fn sys_yield(_ctx: &mut TrapContext) -> usize {
    scheduler::yield_proc();
    0
}
