pub mod context;
pub mod user;
pub mod vector;

use crate::hal::aarch64::asm;
use crate::trap::{
    FaultEvent, InterruptEvent, RawTrapInfo, SyscallEvent, TrapEvent, TrapException, TrapInterrupt,
};
pub use context::TrapFrame;
pub use user::{trap_user_handler, trap_user_return};
pub use vector::kernel_vector;

const AARCH64_IRQ_IPI: usize = 1;
const AARCH64_IRQ_TIMER: usize = 27;
const AARCH64_TRAP_SYNC: usize = 0;
const AARCH64_TRAP_IRQ: usize = 1;
const AARCH64_TRAP_FIQ: usize = 2;
const AARCH64_TRAP_SERROR: usize = 3;

pub unsafe fn vector_init() {
    unsafe { asm::write_vbar(kernel_vector as *const () as usize) }
}

fn raw_trap_info() -> RawTrapInfo {
    RawTrapInfo {
        cause: asm::read_esr(),
        pc: asm::read_elr(),
        value: asm::read_far(),
        status: asm::read_spsr(),
    }
}

fn decode_abort_fault(iss: usize) -> TrapException {
    let fsc = iss & 0x3f;
    match fsc {
        0x21 => TrapException::AccessMisaligned,
        0x00..=0x0f => TrapException::PageFault,
        _ => TrapException::AccessFault,
    }
}

fn decode_sync_exception(raw: RawTrapInfo) -> TrapEvent {
    let ec = (raw.cause >> 26) & 0x3f;
    let iss = raw.cause & 0x01ff_ffff;

    match ec {
        0x15 => TrapEvent::Syscall(SyscallEvent { number: 0, cptr: 0, raw }),
        0x20 | 0x21 | 0x24 | 0x25 => {
            TrapEvent::Fault(FaultEvent { kind: decode_abort_fault(iss), raw })
        }
        0x22 | 0x26 => TrapEvent::Fault(FaultEvent { kind: TrapException::AccessMisaligned, raw }),
        0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 | 0x3c => {
            TrapEvent::Fault(FaultEvent { kind: TrapException::Breakpoint, raw })
        }
        0x00 | 0x0e | 0x18 => {
            TrapEvent::Fault(FaultEvent { kind: TrapException::IllegalInstruction, raw })
        }
        _ => TrapEvent::Unknown(raw),
    }
}

fn decode_event(raw: RawTrapInfo) -> TrapEvent {
    let trap_class = asm::read_contextidr_el1();
    if trap_class == AARCH64_TRAP_SYNC {
        return decode_sync_exception(raw);
    }

    if trap_class == AARCH64_TRAP_SERROR {
        return TrapEvent::Fault(FaultEvent { kind: TrapException::SystemError, raw });
    }

    let cpuid = crate::hal::cpu::cpu_id();
    let irq = crate::hal::irq::claim(cpuid).map(|irq| irq as usize);
    let kind = match (trap_class, irq) {
        (_, Some(AARCH64_IRQ_IPI)) => TrapInterrupt::Software,
        (_, Some(AARCH64_IRQ_TIMER)) => TrapInterrupt::Timer,
        (AARCH64_TRAP_FIQ, Some(_)) => TrapInterrupt::Fast,
        (_, Some(_)) => TrapInterrupt::External,
        (AARCH64_TRAP_FIQ, None) => TrapInterrupt::Fast,
        _ => TrapInterrupt::Unknown(trap_class),
    };
    TrapEvent::Interrupt(InterruptEvent { kind, irq, raw })
}

pub fn trap_event(ctx: &TrapFrame) -> TrapEvent {
    let raw = raw_trap_info();
    match decode_event(raw) {
        TrapEvent::Syscall(_) => {
            let (number, cptr) = ctx.get_syscall_args();
            TrapEvent::Syscall(SyscallEvent { number, cptr, raw })
        }
        other => other,
    }
}

pub fn read_ttbr1() -> usize {
    asm::read_ttbr1()
}

pub fn is_user_mode(status: usize) -> bool {
    (status & 0b1111) == 0
}
