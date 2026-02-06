use super::{TrapCause, TrapException, TrapInterrupt};
use crate::cap::CapType;
use crate::debug::gdb;
use crate::hal;
use crate::hal::trap::TrapFrame;
use crate::ipc;
use crate::ipc::protocol;
use crate::ipc::{MsgFlags, MsgTag};
use crate::irq;
use crate::irq::timer;
use crate::irq::timer::TIME_SLICE_MS;
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET, ANSI_YELLOW};
use crate::proc::TCB;
use crate::proc::scheduler;
use crate::trap::syscall;

#[unsafe(no_mangle)]
pub extern "C" fn trap_kernel_handler(ctx: &mut TrapFrame) {
    irq::enter();
    let cause = hal::trap::get_cause();
    let pc = hal::trap::get_pc();
    let value = hal::trap::get_value();
    let status = hal::trap::get_status();
    match hal::trap::match_cause(cause) {
        TrapCause::Exception(e) => {
            exception_handler(e, pc, cause, value, status, ctx);
        }
        TrapCause::Interrupt(i) => {
            interrupt_handler(i, pc, cause, value, status);
        }
        TrapCause::Unknown(code) => {
            panic!(
                "Unhandled trap: {}, cause: 0x{:#x}, pc: 0x{:#x}, value: 0x{:#x}, status: 0x{:#x}",
                code, cause, pc, value, status
            );
        }
    }
    irq::exit();
}

/// 处理异常情况
fn exception_handler(
    e: TrapException,
    pc: usize,
    cause: usize,
    value: usize,
    status: usize,
    ctx: &mut TrapFrame,
) {
    if e == TrapException::Breakpoint {
        gdb::enter(ctx);
        ctx.advance_pc();
        return;
    }

    if let Some(ptr) = scheduler::current() {
        let tcb = unsafe { &mut *ptr };
        if tcb.native && e == TrapException::Syscall {
            // 处理系统调用
            syscall_handler(ctx);
            return;
        }
        fault_handler(tcb, e, cause, pc, value, status, ctx);
    } else {
        unhandled_exception(e, cause, pc, value, status);
    }
}

/// 处理中断情况
fn interrupt_handler(e: TrapInterrupt, pc: usize, cause: usize, value: usize, status: usize) {
    match e {
        TrapInterrupt::External => external_handler(),
        // S-mode timer interrupt
        TrapInterrupt::Timer => timer_stip(status),
        // S-mode software interrupt
        TrapInterrupt::Software => timer_ssip(status),
        // 剩下的被认为是需要打印的内容
        _ => {
            printk!(
                "{}TRAP(Interrupt){}: {}; pc={:#x}, cause={:#x}, value={:#x}, status={:#x}\n",
                ANSI_YELLOW,
                ANSI_RESET,
                e,
                pc,
                cause,
                value,
                status
            );
        }
    }
}

fn fault_handler(
    tcb: &mut TCB,
    e: TrapException,
    cause: usize,
    pc: usize,
    value: usize,
    status: usize,
    ctx: &mut TrapFrame,
) {
    if let Some(handler_cap) = tcb.fault_handler.clone()
        && handler_cap.cap_type() == CapType::Endpoint
    {
        // 1. 将异常详情写入 UTCB (IPC Buffer)
        if let Some(utcb) = tcb.get_utcb() {
            let label = match e {
                TrapException::PageFault => {
                    utcb.mrs_regs[0] = value; // addr
                    utcb.mrs_regs[1] = pc; // pc
                    utcb.mrs_regs[2] = cause; // cause
                    protocol::PAGE_FAULT
                }
                TrapException::IllegalInstruction => {
                    utcb.mrs_regs[0] = value; // instruction
                    utcb.mrs_regs[1] = pc; // pc
                    protocol::ILLEGAL_INSTRUCTION
                }
                TrapException::Breakpoint => {
                    utcb.mrs_regs[0] = pc; // pc
                    protocol::BREAKPOINT
                }
                TrapException::AccessFault => {
                    utcb.mrs_regs[0] = value; // addr
                    utcb.mrs_regs[1] = pc; // pc
                    protocol::ACCESS_FAULT
                }
                TrapException::AccessMisaligned => {
                    utcb.mrs_regs[0] = value; // addr
                    utcb.mrs_regs[1] = pc; // pc
                    protocol::ACCESS_MISALIGNED
                }
                TrapException::Syscall => {
                    utcb.mrs_regs = ctx.get_registers();
                    protocol::SYSCALL
                }
                _ => {
                    utcb.mrs_regs[0] = cause; // cause
                    utcb.mrs_regs[1] = value; // value
                    utcb.mrs_regs[2] = pc; // pc
                    protocol::UNKNOWN_FAULT
                }
            };

            utcb.msg_tag = MsgTag::new(protocol::KERNEL_PROTO, label, MsgFlags::NONE);
        }
        let ep_ptr = handler_cap.obj_ptr();
        let ep = ep_ptr.as_mut::<ipc::Endpoint>();
        let badge = handler_cap.get_badge();

        // 3. 执行发送 (这会阻塞当前线程)
        ipc::send(tcb, ep, badge, None);
    } else {
        unhandled_exception(e, cause, pc, value, status);
    }
}

fn unhandled_exception(e: TrapException, cause: usize, pc: usize, value: usize, status: usize) {
    printk!(
        "\n{}TRAP(Exception){}: {} cause={:#x}, pc={:#x}, value={:#x}, status={:#x}\n",
        ANSI_RED,
        ANSI_RESET,
        e,
        cause,
        pc,
        value,
        status
    );
    panic!("Kernel panic due to unhandled exception");
}

fn unhandled_interrupt(e: TrapInterrupt, cause: usize, pc: usize, value: usize, status: usize) {
    printk!(
        "{}TRAP(Interrupt){}: {} cause={:#x}, pc={:#x}, value={:#x}, status={:#x}\n",
        ANSI_YELLOW,
        ANSI_RESET,
        e,
        cause,
        pc,
        value,
        status
    );
}

fn syscall_handler(ctx: &mut TrapFrame) {
    let (cptr, method) = ctx.get_syscall_args();
    if cptr == 0 {
        let epc = ctx.get_epc();
        let ra = ctx.get_ra();
        let sp = ctx.get_sp();
        panic!(
            "Syscall with null cptr, method={:#x}, epc={:#x}, ra={:#x}, sp={:#x}",
            method, epc, ra, sp
        );
    }
    let ret = syscall::dispatch(cptr, method);
    ctx.set_return_value(ret);
    ctx.advance_pc();
}

// 外设中断处理 (基于PLIC)
fn external_handler() {
    let cpuid = hal::cpu::cpu_id();
    let id = hal::irq::claim(cpuid);
    match id {
        None => return,
        Some(id) => {
            irq::handle_claimed(cpuid, id as usize);
        }
    }
}

fn timer_ssip(status: usize) {
    hal::irq::clear_soft();
    if hal::trap::is_user_mode(status) {
        scheduler::yield_proc();
    }
}

fn timer_stip(status: usize) {
    hal::timer::set_next_event(timer::msec_to_cycles(TIME_SLICE_MS));
    if hal::trap::is_user_mode(status) {
        scheduler::yield_proc();
    }
}
