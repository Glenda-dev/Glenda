pub mod context;
pub mod user;
pub mod vector;

pub use context::TrapFrame;
pub use user::{trap_user_handler, trap_user_return};
pub use vector::kernel_vector;

use crate::trap::TrapCause;
use crate::trap::{TrapException, TrapInterrupt};
use crate::hal::aarch64::asm;

pub unsafe fn vector_init() {
    unsafe { asm::write_vbar(kernel_vector as *const () as usize) }
}

pub fn match_cause(cause: usize) -> TrapCause {
    // IRQ vector entries set CONTEXTIDR_EL1 to 1 before jumping into Rust handler.
    if asm::read_contextidr_el1() == 1 {
        let cntv_ctl = asm::read_cntv_ctl();
        // CNTV_CTL_EL0.ISTATUS bit indicates a pending virtual timer interrupt.
        if (cntv_ctl & (1 << 2)) != 0 {
            return TrapCause::Interrupt(TrapInterrupt::Timer);
        }
        return TrapCause::Interrupt(TrapInterrupt::External);
    }

    let ec = (cause >> 26) & 0x3F;
    match ec {
        0x15 => TrapCause::Exception(TrapException::Syscall),
        0x20 | 0x21 | 0x24 | 0x25 => TrapCause::Exception(TrapException::PageFault),
        _ => TrapCause::Unknown(cause),
    }
}

pub fn get_cause() -> usize {
    asm::read_esr()
}

pub fn get_pc() -> usize {
    asm::read_elr()
}

pub fn get_value() -> usize {
    asm::read_far()
}

pub fn get_status() -> usize {
    asm::read_spsr()
}

pub fn read_ttbr1() -> usize {
    asm::read_ttbr1()
}

pub fn is_user_mode(status: usize) -> bool {
    (status & 0b1111) == 0
}
