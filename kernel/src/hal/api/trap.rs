use crate::ipc::MsgArgs;
use crate::proc::thread;
use crate::trap::TrapCause;
use core::arch::naked_asm;
/// 陷阱帧结构体
/// 保存陷阱发生时的寄存器状态
pub struct TrapFrame;

impl TrapFrame {
    /// 创建一个新的 TrapFrame 实例
    pub const fn new() -> Self {
        Self
    }
    /// 获取程序计数器 (PC/EPC)
    pub const fn get_epc(&self) -> usize {
        unimplemented!()
    }
    /// 设置程序计数器 (PC/EPC)
    pub fn set_epc(&mut self, epc: usize) {
        unimplemented!()
    }
    /// 设置CPU ID
    pub fn set_cpuid(&mut self, id: usize) {
        unimplemented!()
    }
    /// 配置用户态返回信息
    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize, thread_pointer: usize) {
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
    /// 设置 fast IPC 提示位（架构可选择实现，未实现时可为空操作）
    pub fn set_fast_ipc_hint(&mut self, enabled: bool) {
        let _ = enabled;
        unimplemented!()
    }
    /// 获取系统调用的参数
    pub const fn get_syscall_args(&self) -> (isize, usize) {
        unimplemented!()
    }
    /// 获取 IPC fastpath 的寄存器参数 (msgtag, badge, mr0..mr3)
    pub const fn get_syscall_ipc_args(&self) -> (usize, usize, [usize; 4]) {
        unimplemented!()
    }
    /// 写回 IPC fastpath 的寄存器返回值 (msgtag, badge, mr0..mr3)
    pub fn set_syscall_ipc_ret(&mut self, msgtag: usize, badge: usize, mrs: [usize; 4]) {
        let _ = (msgtag, badge, mrs);
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
    /// 获取常见寄存器的值
    pub fn get_registers(&self) -> MsgArgs {
        unimplemented!()
    }
    /// 获取系统调用参数寄存器的值，按照(n, a, b, c, d, e, f)的顺序返回
    pub fn get_syscall_registers(&self) -> MsgArgs {
        unimplemented!()
    }
    /// 设置常见寄存器的值
    pub fn set_registers(&mut self, regs: &MsgArgs) {
        unimplemented!()
    }
    /// 遍历所有寄存器并应用函数 f
    pub fn for_each_register<F>(&self, mut f: F)
    where
        F: FnMut(usize),
    {
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
pub fn match_cause(cause: usize) -> TrapCause {
    unimplemented!()
}
/// 获取导致 Trap 的原因
pub fn get_cause() -> usize {
    unimplemented!()
}
/// 获取 Trap 发生时的程序计数器 (PC/EPC)
pub fn get_pc() -> usize {
    unimplemented!()
}

/// 获取导致异常的值 (如页错误的 faulting address)
pub fn get_value() -> usize {
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
pub unsafe extern "C" fn user_return(trapframe: usize, satp: usize) {
    naked_asm!("");
}

/// 从用户态返回的函数
pub fn trap_user_return() {
    unimplemented!()
}

/// 判断是否在用户态
pub fn is_user_mode(status: usize) -> bool {
    unimplemented!()
}
