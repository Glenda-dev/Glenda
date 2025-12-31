use crate::irq::TrapContext;
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};

pub mod brk;
pub mod copy;
pub mod helloworld;
pub mod mmap;
pub mod proc;
pub mod util;
pub mod fs;

// 对齐用户侧 include/kernel/syscall/num.h
pub const SYS_HELLOWORLD: usize = 1;
pub const SYS_COPYIN: usize = 2;
pub const SYS_COPYOUT: usize = 3;
pub const SYS_COPYINSTR: usize = 4;
pub const SYS_BRK: usize = 5;
pub const SYS_MMAP: usize = 6;
pub const SYS_MUNMAP: usize = 7;
pub const SYS_PRINT_STR: usize = 8;
pub const SYS_PRINT_INT: usize = 9;
pub const SYS_GETPID: usize = 10;

pub const SYS_ALLOC_BLOCK: usize = 11;
pub const SYS_FREE_BLOCK: usize = 12;
pub const SYS_ALLOC_INODE: usize = 13;
pub const SYS_FREE_INODE: usize = 14;
pub const SYS_SHOW_BITMAP: usize = 15;
pub const SYS_GET_BLOCK: usize = 16;
pub const SYS_READ_BLOCK: usize = 17;
pub const SYS_WRITE_BLOCK: usize = 18;
pub const SYS_PUT_BLOCK: usize = 19;
pub const SYS_SHOW_BUFFER: usize = 20;
pub const SYS_FLUSH_BUFFER: usize = 21;

pub const SYS_FORK: usize = 22;
pub const SYS_WAIT: usize = 23;
pub const SYS_EXIT: usize = 24;
pub const SYS_SLEEP: usize = 25;

// FS API for C tests
pub const SYS_INODE_CREATE: usize = 26;
pub const SYS_INODE_DUP: usize = 27;
pub const SYS_INODE_PUT: usize = 28;
pub const SYS_INODE_SET_NLINK: usize = 29;
pub const SYS_INODE_GET_REFCNT: usize = 30;
pub const SYS_INODE_PRINT: usize = 31;
pub const SYS_INODE_WRITE_DATA: usize = 32;
pub const SYS_INODE_READ_DATA: usize = 33;
pub const SYS_DENTRY_CREATE: usize = 34;
pub const SYS_DENTRY_SEARCH: usize = 35;
pub const SYS_DENTRY_DELETE: usize = 36;
pub const SYS_DENTRY_PRINT: usize = 37;
pub const SYS_PATH_TO_INODE: usize = 38;
pub const SYS_PATH_TO_PARENT: usize = 39;
pub const SYS_PREPARE_ROOT: usize = 40;

pub fn dispatch(ctx: &mut TrapContext) -> usize {
    match ctx.a7 {
        n if n == SYS_HELLOWORLD => helloworld::sys_helloworld(),
        n if n == SYS_COPYOUT => copy::sys_copyout(ctx),
        n if n == SYS_COPYIN => copy::sys_copyin(ctx),
        n if n == SYS_COPYINSTR => copy::sys_copyinstr(ctx),
        n if n == SYS_BRK => brk::sys_brk(ctx),
        n if n == SYS_MMAP => mmap::sys_mmap(ctx),
        n if n == SYS_MUNMAP => mmap::sys_munmap(ctx),

        n if n == SYS_PRINT_STR => util::sys_print_str(ctx),
        n if n == SYS_PRINT_INT => util::sys_print_int(ctx),
        n if n == SYS_GETPID => proc::sys_getpid(),

        n if n == SYS_ALLOC_BLOCK => fs::sys_alloc_block(),
        n if n == SYS_FREE_BLOCK => fs::sys_free_block(ctx),
        n if n == SYS_ALLOC_INODE => fs::sys_alloc_inode(),
        n if n == SYS_FREE_INODE => fs::sys_free_inode(ctx),
        n if n == SYS_SHOW_BITMAP => fs::sys_show_bitmap(ctx),
        n if n == SYS_GET_BLOCK => fs::sys_get_block(ctx),
        n if n == SYS_READ_BLOCK => fs::sys_read_block(ctx),
        n if n == SYS_WRITE_BLOCK => fs::sys_write_block(ctx),
        n if n == SYS_PUT_BLOCK => fs::sys_put_block(ctx),
        n if n == SYS_SHOW_BUFFER => fs::sys_show_buffer(),
        n if n == SYS_FLUSH_BUFFER => fs::sys_flush_buffer(ctx),

        n if n == SYS_FORK => proc::sys_fork(),
        n if n == SYS_WAIT => proc::sys_wait(ctx),
        n if n == SYS_EXIT => proc::sys_exit(ctx),
        n if n == SYS_SLEEP => proc::sys_sleep(ctx),

        // FS extended API
        n if n == SYS_INODE_CREATE => fs::sys_inode_create(ctx),
        n if n == SYS_INODE_DUP => fs::sys_inode_dup(ctx),
        n if n == SYS_INODE_PUT => fs::sys_inode_put(ctx),
        n if n == SYS_INODE_SET_NLINK => fs::sys_inode_set_nlink(ctx),
        n if n == SYS_INODE_GET_REFCNT => fs::sys_inode_get_refcnt(ctx),
        n if n == SYS_INODE_PRINT => fs::sys_inode_print(ctx),
        n if n == SYS_INODE_WRITE_DATA => fs::sys_inode_write_data(ctx),
        n if n == SYS_INODE_READ_DATA => fs::sys_inode_read_data(ctx),
        n if n == SYS_DENTRY_CREATE => fs::sys_dentry_create(ctx),
        n if n == SYS_DENTRY_SEARCH => fs::sys_dentry_search(ctx),
        n if n == SYS_DENTRY_DELETE => fs::sys_dentry_delete(ctx),
        n if n == SYS_DENTRY_PRINT => fs::sys_dentry_print(ctx),
        n if n == SYS_PATH_TO_INODE => fs::sys_path_to_inode(ctx),
        n if n == SYS_PATH_TO_PARENT => fs::sys_path_to_parent_inode(ctx),
        n if n == SYS_PREPARE_ROOT => fs::sys_prepare_root_dir(),

        n => {
            printk!("{}[WARN] SYSCALL: unknown number {}{}\n", ANSI_YELLOW, n, ANSI_RESET);
            usize::MAX
        }
    }
}
