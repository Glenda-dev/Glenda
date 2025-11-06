mod external;
mod timer;
mod uart;

use super::super::TrapContext;
use super::user;
use super::{EXCEPTION_INFO, INTERRUPT_INFO};
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET, ANSI_YELLOW};
use core::panic;
use riscv::register::{
    scause::{self, Trap},
    sepc, sstatus, stval,
};

/// S-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
/// # 参数
/// - `ctx`: 指向栈上保存的寄存器上下文的指针
#[unsafe(no_mangle)]
pub extern "C" fn trap_kernel_handler(ctx: &mut TrapContext) {
    let sc = scause::read();
    let epc = sepc::read();
    let tval = stval::read();
    let sstatus_bits = sstatus::read().bits();

    match sc.cause() {
        Trap::Exception(e) => {
            exception_handler(e, epc, tval, sstatus_bits, ctx);
        }
        Trap::Interrupt(i) => {
            interrupt_handler(i, epc, tval, sstatus_bits, ctx);
        }
    }
}

/// 处理异常情况
fn exception_handler(
    e: usize,
    epc: usize,
    tval: usize,
    sstatus_bits: usize,
    ctx: &mut TrapContext,
) {
    // 8: Environment call from U-mode (syscall)
    if e == 8 {
        user::syscall::interrupt_handler(ctx);
        // advance sepc to next instruction
        unsafe {
            sepc::write(epc.wrapping_add(4));
        }
        return;
    }

    // 13: Load Page Fault, 15: Store/AMO Page Fault
    if e == 13 || e == 15 {
        let p = crate::proc::current_proc();
        if p.ustack_grow(tval).is_ok() {
            return;
        }
    }
    printk!(
        "{}TRAP(Exception){}: code={} ({}); epc=0x{:x}, tval=0x{:x}, sstatus=0x{:x}",
        ANSI_RED,
        ANSI_RESET,
        e,
        EXCEPTION_INFO.get(e).unwrap_or(&"Unknown Exception"),
        epc,
        tval,
        sstatus_bits
    );
    panic!("Kernel panic due to exception");
}

/// 处理中断情况
fn interrupt_handler(
    e: usize,
    epc: usize,
    tval: usize,
    sstatus_bits: usize,
    _ctx: &mut TrapContext,
) {
    match e {
        9 => external::interrupt_handler(),
        // S-mode timer interrupt
        5 => timer::interrupt_handler_stip(),
        // S-mode software interrupt
        1 => timer::interrupt_handler_ssip(),
        // 剩下的被认为是需要打印的内容
        _ => {
            printk!(
                "{}TRAP(Interrupt){}: code={} ({}); epc=0x{:x}, tval=0x{:x}, sstatus=0x{:x}",
                ANSI_YELLOW,
                ANSI_RESET,
                e,
                INTERRUPT_INFO.get(e).unwrap_or(&"Unknown Interrupt"),
                epc,
                tval,
                sstatus_bits
            );
        }
    }
}
