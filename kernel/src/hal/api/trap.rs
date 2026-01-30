use crate::mem::VirtAddr;
use crate::trap::TrapCause;
use core::arch::naked_asm;
/// 陷阱帧结构体
/// 保存陷阱发生时的寄存器状态
pub struct TrapFrame;

impl TrapFrame {
    /// 获取程序计数器 (PC/EPC)
    pub const fn get_epc(&self) -> usize {
        unimplemented!()
    }
    /// 设置程序计数器 (PC/EPC)
    pub fn set_epc(&mut self, epc: usize) {
        unimplemented!()
    }
    /// 配置用户态返回信息
    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize) {
        unimplemented!()
    }
    /// 配置内核态返回信息
    pub fn configure_kernel(
        &mut self,
        mmu: usize,
        hartid: usize,
        kstack_top: usize,
        kernel_vec: usize,
    ) {
        unimplemented!()
    }
    /// 设置系统调用的返回值
    pub fn set_return_value(&mut self, value: usize) {
        unimplemented!()
    }
    /// 获取系统调用的参数
    pub const fn get_syscall_args(&self) -> (usize, usize) {
        unimplemented!()
    }
    /// 推进程序计数器，跳过当前指令
    pub fn advance_pc(&mut self) {
        unimplemented!()
    }
    /// 获取返回地址寄存器的值
    pub const fn get_ra(&self) -> usize {
        unimplemented!()
    }
    /// 获取栈指针寄存器的值
    pub const fn get_sp(&self) -> usize {
        unimplemented!()
    }
}

/// 初始化异常向量表
///
/// 将 CPU 的异常入口基地址 (stvec/vbar_el1) 指向内核的 trap handler
pub unsafe fn vector_init() {
    unimplemented!()
}

/// 获取导致 Trap 的原因
/// 返回架构无关的枚举 (Syscall, Timer, ExternalIrq, PageFault...)
pub fn get_cause() -> TrapCause {
    unimplemented!()
}
/// 获取 Trap 发生时的程序计数器 (PC/EPC)
pub fn get_pc() -> usize {
    unimplemented!()
}

/// 获取导致异常的地址 (如页错误的 faulting address)
pub fn get_address() -> VirtAddr {
    unimplemented!()
}

/// 获取陷阱发生时的状态寄存器 (sstatus/spsr)
pub fn get_status() -> usize {
    unimplemented!()
}

/// 内核态陷阱处理函数
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector() {
    naked_asm!("")
}

/// 用户态陷阱处理函数
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector() {
    naked_asm!("");
}

/// U-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler() {
    unimplemented!()
}

/// 从内核态返回用户态
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_return(trapframe: u64, satp: u64) {
    naked_asm!("");
}

/// 从用户态返回的函数
pub fn trap_user_return() {
    unimplemented!()
}
