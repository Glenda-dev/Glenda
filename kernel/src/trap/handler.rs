use super::context::TrapContext;
use super::timer;
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET, ANSI_YELLOW};
use core::arch::asm;
use core::panic;
use riscv::interrupt::supervisor::Interrupt;
use riscv::register::{
    scause::{self, Trap},
    sepc, sip, sstatus, stval,
};

const EXCEPTION_INFO: [&str; 16] = [
    "Instruction address misaligned", // 0
    "Instruction access fault",       // 1
    "Illegal instruction",            // 2
    "Breakpoint",                     // 3
    "Load address misaligned",        // 4
    "Load access fault",              // 5
    "Store/AMO address misaligned",   // 6
    "Store/AMO access fault",         // 7
    "Environment call from U-mode",   // 8
    "Environment call from S-mode",   // 9
    "reserved-1",                     // 10
    "Environment call from M-mode",   // 11
    "Instruction page fault",         // 12
    "Load page fault",                // 13
    "reserved-2",                     // 14
    "Store/AMO page fault",           // 15
];

const INTERRUPT_INFO: [&str; 16] = [
    "U-mode software interrupt", // 0
    "S-mode software interrupt", // 1
    "reserved-1",                // 2
    "M-mode software interrupt", // 3
    "U-mode timer interrupt",    // 4
    "S-mode timer interrupt",    // 5
    "reserved-2",                // 6
    "M-mode timer interrupt",    // 7
    "U-mode external interrupt", // 8
    "S-mode external interrupt", // 9
    "reserved-3",                // 10
    "M-mode external interrupt", // 11
    "reserved-4",                // 12
    "reserved-5",                // 13
    "reserved-6",                // 14
    "reserved-7",                // 15
];

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
    _ctx: &mut TrapContext,
) {
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
    match e {
        9 => external_interrupt_handler(),
        1 => timer_interrupt_handler(),
        _ => {}
    }
}

// 外设中断处理 (基于PLIC，lab-3只需要识别和处理UART中断)
pub fn external_interrupt_handler() {}

pub fn timer_interrupt_handler() {
    // Get hart id
    let mut hartid = 0;
    unsafe {
        asm!(
            "mv {hartid}, tp",
            hartid = out(reg) hartid,
            options(nomem, nostack, preserves_flags)
        );
    }
    if hartid == 0 {
        timer::update();
    }
    // 清除 SSIP bit (S-mode software interrupt pending)
    // 宣布 S-mode 软件中断处理完成
    unsafe {
        sip::clear_pending(Interrupt::SupervisorSoft);
    }
}
