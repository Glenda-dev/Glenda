use crate::printk;
use crate::trap::TrapContext;

pub mod brk;
pub mod copy;
pub mod helloworld;
pub mod mmap;

// 对齐用户侧 include/kernel/syscall/num.h
pub const SYS_HELLOWORLD: usize = 1;
pub const SYS_COPYIN: usize = 2;
pub const SYS_COPYOUT: usize = 3;
pub const SYS_COPYINSTR: usize = 4;
pub const SYS_BRK: usize = 5;
pub const SYS_MMAP: usize = 6;
pub const SYS_MUNMAP: usize = 7;

pub fn dispatch(ctx: &mut TrapContext) -> usize {
    match ctx.a7 {
        n if n == SYS_HELLOWORLD => helloworld::sys_helloworld(),
        n if n == SYS_COPYOUT => copy::sys_copyout(ctx),
        n if n == SYS_COPYIN => copy::sys_copyin(ctx),
        n if n == SYS_COPYINSTR => copy::sys_copyinstr(ctx),
        n if n == SYS_BRK => brk::sys_brk(ctx),
        n if n == SYS_MMAP => mmap::sys_mmap(ctx),
        n if n == SYS_MUNMAP => mmap::sys_munmap(ctx),
        n => {
            printk!("SYSCALL: unknown number {}", n);
            usize::MAX
        }
    }
}
