use riscv::register::stvec::{self, Stvec};

unsafe extern "C" {
    // 这个函数会保存所有通用寄存器，调用 trap_kernel_handler，然后恢复寄存器
    pub fn kernel_vector() -> !;
    // 用于处理机器模式下的时钟中断，并触发 S-mode 软件中断
    pub fn timer_vector_base();
    // 这个函数会保存所有通用寄存器，调用 trap_user_handler，然后恢复寄存器
    pub fn user_vector() -> !;
    // 这个函数会从栈上恢复寄存器并返回到用户态
    pub fn user_return(trapframe: u64, pagetable: u64) -> !;
    // 这个函数会跳转到 trampoline 区域，切换到用户态
    pub fn trampoline() -> !;
}

pub fn init() {
    let kernel_vec_addr = kernel_vector as usize;
    let vec = Stvec::new(kernel_vec_addr, stvec::TrapMode::Direct);
    unsafe {
        // set supervisor trap vector address
        stvec::write(vec);
    }
}
