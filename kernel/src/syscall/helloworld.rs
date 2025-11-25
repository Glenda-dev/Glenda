use crate::printk;

pub fn sys_helloworld() -> usize {
    // syscall handler implementation
    printk!("proczero: hello world!\n");
    0
}
