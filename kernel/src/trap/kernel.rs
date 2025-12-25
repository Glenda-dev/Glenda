use super::TrapContext;
use super::info::{EXCEPTION_INFO, INTERRUPT_INFO};
use super::interrupt;
use super::timer;
use super::user;
use crate::cap::CapType;
use crate::hart;
use crate::ipc;
use crate::ipc::MsgTag;
use crate::irq;
use crate::irq::plic;
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET, ANSI_YELLOW};
use crate::proc::scheduler;
use core::panic;
use riscv::interrupt::Interrupt;
use riscv::register::scause::Trap;
use riscv::register::{scause, sepc, sip, sstatus, stval};

/// S-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
/// # 参数
/// - `ctx`: 指向栈上保存的寄存器上下文的指针
#[unsafe(no_mangle)]
pub extern "C" fn trap_kernel_handler(ctx: &mut TrapContext) {
    interrupt::enter();
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
    interrupt::exit();
}

/// 处理异常情况
fn exception_handler(
    e: usize,
    epc: usize,
    tval: usize,
    sstatus_bits: usize,
    ctx: &mut TrapContext,
) {
    // 其他同步异常交给 handle_exception 处理
    let sc = scause::read().bits();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };

    if let Some(handler_cap) = tcb.fault_handler.clone() {
        // 1. 将异常详情写入 UTCB (IPC Buffer)
        // 消息格式: [scause, stval, sepc]
        if let Some(utcb) = tcb.get_utcb() {
            utcb.mrs_regs[0] = sc;
            utcb.mrs_regs[1] = tval;
            utcb.mrs_regs[2] = epc;
            utcb.msg_tag = MsgTag::new(ipc::label::FAULT, 3); // Label: 0xFFFF (Fault), Length: 3
        }

        // 2. 提取 Endpoint
        if let CapType::Endpoint { ep_ptr } = handler_cap.object {
            let ep = ep_ptr.as_mut::<ipc::Endpoint>();
            let badge = handler_cap.badge.unwrap_or(0);

            // 3. 执行发送 (这会阻塞当前线程)
            ipc::send(tcb, ep, badge, None);
        } else {
            panic!("Fault handler is not an Endpoint");
        }
    } else {
        // 8: Environment call from U-mode (syscall)
        if e == 8 {
            user::syscall_handler(ctx);
            // advance sepc to next instruction
            unsafe {
                sepc::write(epc.wrapping_add(4));
            }
            return;
        }
        printk!(
            "{}TRAP(Exception){}: code={} ({}); epc=0x{:x}, tval=0x{:x}, sstatus=0x{:x}\n",
            ANSI_RED,
            ANSI_RESET,
            e,
            EXCEPTION_INFO.get(e).unwrap_or(&"Unknown Exception"),
            epc,
            tval,
            sstatus_bits
        );
        panic!("Kernel panic due to unhandled exception");
    }
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
        9 => external_handler(),
        // S-mode timer interrupt
        5 => timer_handler_stip(sstatus_bits),
        // S-mode software interrupt
        1 => timer_handler_ssip(sstatus_bits),
        // 剩下的被认为是需要打印的内容
        _ => {
            printk!(
                "{}TRAP(Interrupt){}: code={} ({}); epc=0x{:x}, tval=0x{:x}, sstatus=0x{:x}\n",
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

// 外设中断处理 (基于PLIC)
pub fn external_handler() {
    let hartid = hart::get().id;
    let id = plic::claim(hartid);
    match id {
        0 => return,
        _ => {
            // Delegate to irq manager to notify bound endpoint and complete
            irq::handle_claimed(hartid, id);
        }
    }
}

pub fn timer_handler_ssip(sstatus_bits: usize) {
    if hart::get().id == 0 {
        timer::update();
    }
    unsafe {
        sip::clear_pending(Interrupt::SupervisorSoft);
    }

    if (sstatus_bits & (1 << 8)) == 0 {
        scheduler::yield_proc();
    }
}

pub fn timer_handler_stip(sstatus_bits: usize) {
    if hart::get().id == 0 {
        timer::update();
    }
    timer::program_next_tick();

    if (sstatus_bits & (1 << 8)) == 0 {
        scheduler::yield_proc();
    }
}
