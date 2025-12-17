use crate::irq::TrapContext;
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};

pub mod cap;
pub mod ipc;
pub mod proc;
pub mod util;

pub const SYS_INVOKE: usize = 1;
pub const SYS_REPLY_RECV: usize = 2;
pub const SYS_SEND: usize = 3;
pub const SYS_RECV: usize = 4;
pub const SYS_YIELD: usize = 5;
pub const SYS_PRINT: usize = 6;

pub fn dispatch(ctx: &mut TrapContext) -> usize {
    match ctx.a7 {
        n if n == SYS_INVOKE => cap::sys_invoke(ctx),
        n if n == SYS_REPLY_RECV => ipc::sys_reply_recv(ctx),
        n if n == SYS_SEND => ipc::sys_send(ctx),
        n if n == SYS_RECV => ipc::sys_recv(ctx),
        n if n == SYS_YIELD => proc::sys_yield(ctx),
        n if n == SYS_PRINT => util::sys_print(ctx),

        n => {
            printk!("{}[WARN] SYSCALL: unknown number {}{}\n", ANSI_YELLOW, n, ANSI_RESET);
            usize::MAX
        }
    }
}
