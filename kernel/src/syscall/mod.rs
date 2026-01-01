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

pub const SYS_EXEC: usize = 41;
pub const SYS_OPEN: usize = 42;
pub const SYS_CLOSE: usize = 43;
pub const SYS_READ: usize = 44;
pub const SYS_WRITE: usize = 45;
pub const SYS_LSEEK: usize = 46;
pub const SYS_DUP: usize = 47;
pub const SYS_FSTAT: usize = 48;
pub const SYS_GET_DENTRIES: usize = 49;
pub const SYS_MKDIR: usize = 50;
pub const SYS_CHDIR: usize = 51;
pub const SYS_PRINT_CWD: usize = 52;
pub const SYS_LINK: usize = 53;
pub const SYS_UNLINK: usize = 54;

pub fn dispatch(ctx: &mut TrapContext) -> usize {
    match ctx.a7 {
        SYS_HELLOWORLD => helloworld::sys_helloworld(),
        SYS_COPYOUT => copy::sys_copyout(ctx),
        SYS_COPYIN => copy::sys_copyin(ctx),
        SYS_COPYINSTR => copy::sys_copyinstr(ctx),
        SYS_BRK => brk::sys_brk(ctx),
        SYS_MMAP => mmap::sys_mmap(ctx),
        SYS_MUNMAP => mmap::sys_munmap(ctx),

        SYS_PRINT_STR => util::sys_print_str(ctx),
        SYS_PRINT_INT => util::sys_print_int(ctx),
        SYS_GETPID => proc::sys_getpid(),

        SYS_ALLOC_BLOCK => fs::sys_alloc_block(),
        SYS_FREE_BLOCK => fs::sys_free_block(ctx),
        SYS_ALLOC_INODE => fs::sys_alloc_inode(),
        SYS_FREE_INODE => fs::sys_free_inode(ctx),
        SYS_SHOW_BITMAP => fs::sys_show_bitmap(ctx),
        SYS_GET_BLOCK => fs::sys_get_block(ctx),
        SYS_READ_BLOCK => fs::sys_read_block(ctx),
        SYS_WRITE_BLOCK => fs::sys_write_block(ctx),
        SYS_PUT_BLOCK => fs::sys_put_block(ctx),
        SYS_SHOW_BUFFER => fs::sys_show_buffer(),
        SYS_FLUSH_BUFFER => fs::sys_flush_buffer(ctx),

        SYS_FORK => proc::sys_fork(),
        SYS_WAIT => proc::sys_wait(ctx),
        SYS_EXIT => proc::sys_exit(ctx),
        SYS_SLEEP => proc::sys_sleep(ctx),
        SYS_EXEC => proc::sys_exec(ctx),

        // FS extended API
        SYS_INODE_CREATE => fs::sys_inode_create(ctx),
        SYS_INODE_DUP => fs::sys_inode_dup(ctx),
        SYS_INODE_PUT => fs::sys_inode_put(ctx),
        SYS_INODE_SET_NLINK => fs::sys_inode_set_nlink(ctx),
        SYS_INODE_GET_REFCNT => fs::sys_inode_get_refcnt(ctx),
        SYS_INODE_PRINT => fs::sys_inode_print(ctx),
        SYS_INODE_WRITE_DATA => fs::sys_inode_write_data(ctx),
        SYS_INODE_READ_DATA => fs::sys_inode_read_data(ctx),
        SYS_DENTRY_CREATE => fs::sys_dentry_create(ctx),
        SYS_DENTRY_SEARCH => fs::sys_dentry_search(ctx),
        SYS_DENTRY_DELETE => fs::sys_dentry_delete(ctx),
        SYS_DENTRY_PRINT => fs::sys_dentry_print(ctx),
        SYS_PATH_TO_INODE => fs::sys_path_to_inode(ctx),
        SYS_PATH_TO_PARENT => fs::sys_path_to_parent_inode(ctx),
        SYS_PREPARE_ROOT => fs::sys_prepare_root_dir(),

        // LAB-9 Syscalls
        SYS_OPEN => fs::sys_open(ctx),
        SYS_CLOSE => fs::sys_close(ctx),
        SYS_READ => fs::sys_read(ctx),
        SYS_WRITE => fs::sys_write(ctx),
        SYS_LSEEK => fs::sys_lseek(ctx),
        SYS_DUP => fs::sys_dup(ctx),
        SYS_FSTAT => fs::sys_fstat(ctx),
        SYS_GET_DENTRIES => fs::sys_get_dentries(ctx),
        SYS_MKDIR => fs::sys_mkdir(ctx),
        SYS_CHDIR => fs::sys_chdir(ctx),
        SYS_PRINT_CWD => fs::sys_print_cwd(),
        SYS_LINK => fs::sys_link(ctx),
        SYS_UNLINK => fs::sys_unlink(ctx),

        n => {
            printk!("{}[WARN] SYSCALL: unknown number {}{}\n", ANSI_YELLOW, n, ANSI_RESET);
            usize::MAX
        }
    }
}
