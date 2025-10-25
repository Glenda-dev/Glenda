use riscv::register::stvec::{self, Stvec};
use riscv::register::{mscratch, sip};

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

#[unsafe(no_mangle)]
pub extern "C" fn timer_vector_body() {
    timer_vector_update_from_mscratch()
}

#[inline(always)]
fn timer_vector_update_from_mscratch() {
    unsafe {
        // 读取 mscratch 指向的 per-hart 缓冲区
        let base = mscratch::read() as *mut usize;
        // 偏移 3: CLINT_MTIMECMP 的地址；偏移 4: INTERVAL
        let mtimecmp_addr = base.add(3).read() as *mut usize;
        let interval = base.add(4).read();
        let cur = core::ptr::read_volatile(mtimecmp_addr);
        core::ptr::write_volatile(mtimecmp_addr, cur.wrapping_add(interval));
        // 在 M-mode 下触发 S-mode 软件中断
        sip::set_ssoft();
    }
}

pub fn set() {
    let kernel_vec_addr = kernel_vector as usize;
    let vec = Stvec::new(kernel_vec_addr, stvec::TrapMode::Direct);
    unsafe {
        // set supervisor trap vector address
        stvec::write(vec);
    }
}
