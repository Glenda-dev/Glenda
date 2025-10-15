use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET, ANSI_YELLOW};

use riscv::asm::wfi;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::{scause::{self, Trap}, sepc, sstatus, stval};

#[unsafe(no_mangle)]
pub extern "C" fn glenda_trap() -> ! {
    let sc = scause::read();
    let epc = sepc::read();
    let tval = stval::read();
    let sstatus_bits = sstatus::read().bits();

    match sc.cause() {
        Trap::Exception(e) => {
            printk!(
                "{}TRAP(Exception){}: {:?} | sepc={:#018x} stval={:#018x} sstatus={:#x}",
                ANSI_RED,
                ANSI_RESET,
                e,
                epc,
                tval,
                sstatus_bits
            );
            // 对于异常，直接停机以暴露问题
            loop { wfi(); }
        }
        Trap::Interrupt(i) => {
            printk!(
                "{}TRAP(Interrupt){}: {:?} | sepc={:#018x} stval={:#018x} sstatus={:#x}",
                ANSI_YELLOW,
                ANSI_RESET,
                i,
                epc,
                tval,
                sstatus_bits
            );
            loop { wfi(); }
        }
    }
}
