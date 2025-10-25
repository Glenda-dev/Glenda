use super::super::context::TrapContext;
use crate::printk;
use crate::syscall;

pub fn handle_syscall(ctx: &mut TrapContext) {
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

/// U-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler() {}

#[unsafe(no_mangle)]
pub extern "C" fn trap_user_return() {}
