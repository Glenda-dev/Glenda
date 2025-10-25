use crate::printk;
use crate::syscall;
use crate::trap::TrapContext;

pub fn interrupt_handler(ctx: &mut TrapContext) {
    // TODO: Add Real Linux Syscall for compatibility
    match ctx.a7 {
        1 => {
            // SYS_helloworld
            syscall::helloworld::handle();
        }
        n => {
            printk!("SYSCALL: unknown number {}\n", n);
        }
    }
}
