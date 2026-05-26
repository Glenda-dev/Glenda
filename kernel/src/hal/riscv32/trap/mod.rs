mod context;
mod user;
mod vector;

use super::asm;
use crate::trap::cause::VirtExitEvent;
use crate::trap::{
    FaultEvent, InterruptEvent, RawTrapInfo, SyscallEvent, TrapEvent, TrapException, TrapInterrupt,
};
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

fn raw_trap_info() -> RawTrapInfo {
    RawTrapInfo {
        cause: asm::read_scause(),
        pc: asm::read_sepc(),
        value: asm::read_stval(),
        status: asm::read_sstatus(),
    }
}

fn decode_interrupt(code: usize) -> TrapInterrupt {
    match code {
        1 => TrapInterrupt::Software,
        5 => TrapInterrupt::Timer,
        6 => TrapInterrupt::VirtualSupervisorTimer,
        9 => TrapInterrupt::External,
        10 => TrapInterrupt::VirtualSupervisorExternal,
        2 => TrapInterrupt::VirtualSupervisorSoftware,
        _ => TrapInterrupt::Unknown(code),
    }
}

fn decode_exception(code: usize) -> TrapException {
    match code {
        2 => TrapException::IllegalInstruction,
        3 => TrapException::Breakpoint,
        0 | 4 | 6 => TrapException::AccessMisaligned,
        1 | 5 | 7 => TrapException::AccessFault,
        8 => TrapException::Syscall,
        10 => TrapException::VirtualSupervisorSyscall,
        12 | 13 | 15 => TrapException::PageFault,
        20 | 21 | 23 => TrapException::GuestPageFault,
        22 => TrapException::VirtualInstruction,
        _ => TrapException::Unknown(code),
    }
}

pub fn trap_event(ctx: &TrapFrame) -> TrapEvent {
    let raw = raw_trap_info();
    let is_interrupt = (raw.cause >> (usize::BITS - 1)) != 0;
    let code = raw.cause & !(1 << (usize::BITS - 1));

    if is_interrupt {
        let kind = decode_interrupt(code);
        return TrapEvent::Interrupt(InterruptEvent { kind, irq: None, raw });
    }

    match decode_exception(code) {
        TrapException::Syscall => {
            let (number, cptr) = ctx.get_syscall_args();
            TrapEvent::Syscall(SyscallEvent { number, cptr, raw })
        }
        kind @ (TrapException::GuestPageFault
        | TrapException::VirtualInstruction
        | TrapException::VirtualSupervisorSyscall) => {
            TrapEvent::VirtExit(VirtExitEvent { kind, raw })
        }
        TrapException::Unknown(_) => TrapEvent::Unknown(raw),
        kind => TrapEvent::Fault(FaultEvent { kind, raw }),
    }
}

/// 判断是否在用户态
pub fn is_user_mode(status: usize) -> bool {
    return (status & (1 << 8)) == 0;
}
