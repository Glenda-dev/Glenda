use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use crate::trap::TrapContext;

pub mod cap;
pub mod ipc;

pub const SYS_INVOKE: usize = 1;
pub const SYS_SEND: usize = 2;
pub const SYS_RECV: usize = 3;

pub mod errcode {
    pub const SUCCESS: usize = 0;
    pub const INVALID_CAP: usize = 1;
    pub const INVALID_ENDPOINT: usize = 2;
    pub const INVALID_OBJ_TYPE: usize = 3;
    pub const INVALID_METHOD: usize = 4;
    pub const MAPPING_FAILED: usize = 6;
    pub const INVALID_SLOT: usize = 7;
    pub const UNTYPE_OOM: usize = 8;
}

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
