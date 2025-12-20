use crate::proc::scheduler;
use crate::trap::TrapContext;

pub fn sys_yield(_ctx: &mut TrapContext) -> usize {
    scheduler::yield_proc();
    0
}
