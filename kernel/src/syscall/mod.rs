use crate::irq::TrapContext;
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};

pub mod cap;
pub mod ipc;
pub mod proc;

pub const SYS_INVOKE: usize = 1;
pub const SYS_SEND: usize = 2;
pub const SYS_RECV: usize = 3;

pub fn dispatch(ctx: &mut TrapContext) -> usize {
    match ctx.a7 {
        n if n == SYS_INVOKE => cap::sys_invoke(ctx),
        n if n == SYS_SEND => ipc::sys_send(ctx),
        n if n == SYS_RECV => ipc::sys_recv(ctx),

        n => {
            printk!("{}[WARN] SYSCALL: unknown number {}{}\n", ANSI_YELLOW, n, ANSI_RESET);
            usize::MAX
        }
    }
}
