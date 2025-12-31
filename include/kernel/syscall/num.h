#ifndef GLENDA_SYSCALL_NUM_H
#define GLENDA_SYSCALL_NUM_H

#define SYS_helloworld 1
#define SYS_copyin     2
#define SYS_copyout    3
#define SYS_copyinstr  4
#define SYS_brk        5
#define SYS_mmap       6
#define SYS_munmap     7
#define SYS_print_str  8
#define SYS_print_int  9
#define SYS_getpid     10

#define SYS_alloc_block 11
#define SYS_free_block  12
#define SYS_alloc_inode 13
#define SYS_free_inode  14
#define SYS_show_bitmap 15
#define SYS_get_block   16
#define SYS_read_block  17
#define SYS_write_block 18
#define SYS_put_block   19
#define SYS_show_buffer 20
#define SYS_flush_buffer 21

#define SYS_fork       22
#define SYS_wait       23
#define SYS_exit       24
#define SYS_sleep      25

#define SYS_inode_create      26
#define SYS_inode_dup         27
#define SYS_inode_put         28
#define SYS_inode_set_nlink   29
#define SYS_inode_get_refcnt  30
#define SYS_inode_print       31
#define SYS_inode_write_data  32
#define SYS_inode_read_data   33
#define SYS_dentry_create     34
#define SYS_dentry_search     35
#define SYS_dentry_delete     36
#define SYS_dentry_print      37
#define SYS_path_to_inode     38
#define SYS_path_to_parent    39
#define SYS_prepare_root      40

#define SYS_exec              41

#endif // GLENDA_SYSCALL_NUM_H
