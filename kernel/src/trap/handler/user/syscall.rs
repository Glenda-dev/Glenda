use crate::printk;
use crate::syscall;
use crate::trap::TrapContext;

pub fn interrupt_handler(ctx: &mut TrapContext) {
    let ret = syscall::dispatch(ctx);
    ctx.a0 = ret;
}
