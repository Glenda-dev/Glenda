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
        asm!("csrw stvec, {}", in(reg) kernel_vector as usize);
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
            9 => TrapCause::Interrupt(TrapInterrupt::External), // Supervisor External Interrupt
            _ => TrapCause::Unknown(cause),
        }
    } else {
        match code {
            2 => TrapCause::Exception(TrapException::IllegalInstruction),
            3 => TrapCause::Exception(TrapException::Breakpoint),
            0 | 4 | 6 => TrapCause::Exception(TrapException::AccessMisaligned),
            1 | 5 | 7 => TrapCause::Exception(TrapException::AccessFault),
            8 => TrapCause::Exception(TrapException::Syscall), // Environment call from U-mode
            12 | 13 | 15 => TrapCause::Exception(TrapException::PageFault), // Instruction/Load/Store Page Fault
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
