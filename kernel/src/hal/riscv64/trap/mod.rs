mod context;
mod user;
mod vector;

use super::asm;
use crate::trap::TrapCause;
use crate::trap::{TrapException, TrapInterrupt};
use core::arch::asm;
use vector::kernel_vector;

/// 上下文结构体
/// 保存寄存器等上下文信息
pub use context::TrapFrame;
pub use user::{trap_user_handler, trap_user_return};

/// 初始化异常向量表
///
/// 将 CPU 的异常入口基地址 (stvec/vbar_el1) 指向内核的 trap handler
pub unsafe fn vector_init() {
    unsafe {
        asm!("csrw stvec, {}", in(reg) kernel_vector as *const () as usize);
    }
}

/// 获取导致 Trap 的原因
/// 返回架构无关的枚举 (Syscall, Timer, ExternalIrq, PageFault...)
pub fn match_cause(cause: usize) -> TrapCause {
    // RISC-V scause 布局:
    // 最高位 (Interrupt Bit): 1=Interrupt, 0=Exception
    // 低位 (Exception Code)
    let is_interrupt = (cause >> 63) != 0;
    let code = cause & !(1 << 63);

    if is_interrupt {
        match code {
            5 => TrapCause::Interrupt(TrapInterrupt::Timer), // Supervisor Timer Interrupt
            6 => TrapCause::Interrupt(TrapInterrupt::VirtualSupervisorTimer),
            9 => TrapCause::Interrupt(TrapInterrupt::External), // Supervisor External Interrupt
            10 => TrapCause::Interrupt(TrapInterrupt::VirtualSupervisorExternal),
            2 => TrapCause::Interrupt(TrapInterrupt::VirtualSupervisorSoftware),
            _ => TrapCause::Unknown(cause),
        }
    } else {
        match code {
            2 => TrapCause::Exception(TrapException::IllegalInstruction),
            3 => TrapCause::Exception(TrapException::Breakpoint),
            0 | 4 | 6 => TrapCause::Exception(TrapException::AccessMisaligned),
            1 | 5 | 7 => TrapCause::Exception(TrapException::AccessFault),
            8 => TrapCause::Exception(TrapException::Syscall), // Environment call from U-mode
            10 => TrapCause::Exception(TrapException::VirtualSupervisorSyscall),
            12 | 13 | 15 => TrapCause::Exception(TrapException::PageFault), // Instruction/Load/Store Page Fault
            20 | 21 | 23 => TrapCause::Exception(TrapException::GuestPageFault),
            22 => TrapCause::Exception(TrapException::VirtualInstruction),
            _ => TrapCause::Unknown(cause),
        }
    }
}
/// 获取 Trap 发生时的程序计数器 (PC/EPC)
pub fn get_pc() -> usize {
    asm::read_sepc()
}

pub fn get_value() -> usize {
    asm::read_stval()
}

pub fn get_status() -> usize {
    asm::read_sstatus()
}

pub fn get_cause() -> usize {
    asm::read_scause()
}

/// 判断是否在用户态
pub fn is_user_mode(status: usize) -> bool {
    return (status & (1 << 8)) == 0;
}
