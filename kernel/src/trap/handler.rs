use super::{TrapCause, TrapException, TrapInterrupt};
use crate::cap::CapType;
use crate::hal;
use crate::hal::trap::TrapFrame;
use crate::ipc;
use crate::ipc::MsgTag;
use crate::irq;
use crate::irq::timer;
use crate::mem::VirtAddr;
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
    let addr = hal::trap::get_address();
    let status = hal::trap::get_status();
    match cause {
        TrapCause::Exception(e) => {
            exception_handler(e, pc, addr, status, ctx);
        }
        TrapCause::Interrupt(i) => {
            interrupt_handler(i, pc, addr, status);
        }
        TrapCause::Unknown(code) => {
            panic!(
                "Unhandled trap: {}, pc: {:#x}, addr: {:#x}, status: {:#x}",
                code,
                pc,
                addr.as_usize(),
                status
            );
        }
    }
    irq::exit();
}

/// 处理异常情况
fn exception_handler(
    e: TrapException,
    pc: usize,
    addr: VirtAddr,
    status: usize,
    ctx: &mut TrapFrame,
) {
    if let Some(ptr) = scheduler::current() {
        let tcb = unsafe { &mut *ptr };
        if tcb.native && e == TrapException::Syscall {
            // 处理系统调用
            syscall_handler(ctx);
            return;
        }
        fault_handler(tcb, e, pc, addr, status);
    } else {
        unhandled_exception(e, pc, addr, status);
    }
}

/// 处理中断情况
fn interrupt_handler(e: TrapInterrupt, pc: usize, addr: VirtAddr, status: usize) {
    match e {
        TrapInterrupt::External => external_handler(),
        // S-mode timer interrupt
        TrapInterrupt::Timer => timer_stip(status),
        // S-mode software interrupt
        TrapInterrupt::Software => timer_ssip(status),
        // 剩下的被认为是需要打印的内容
        _ => {
            printk!(
                "{}TRAP(Interrupt){}: {}; pc=0x{:x}, addr={}, status=0x{:x}\n",
                ANSI_YELLOW,
                ANSI_RESET,
                e,
                pc,
                addr,
                status
            );
        }
    }
}

fn fault_handler(tcb: &mut TCB, e: TrapException, pc: usize, addr: VirtAddr, status: usize) {
    if let Some(handler_cap) = tcb.fault_handler.clone() {
        // 1. 将异常详情写入 UTCB (IPC Buffer)
        // 消息格式: [scause, stval, sepc]
        if let Some(utcb) = tcb.get_utcb() {
            utcb.mrs_regs[0] = e.as_usize();
            utcb.mrs_regs[1] = addr.as_usize();
            utcb.mrs_regs[2] = pc;

            let label = match e {
                TrapException::PageFault => ipc::label::PAGE_FAULT,
                _ => ipc::label::EXCEPTION,
            };

            utcb.msg_tag = MsgTag::new(label, 3);
        }

        // 2. 提取 Endpoint
        if handler_cap.cap_type() == CapType::Endpoint {
            let ep_ptr = handler_cap.obj_ptr();
            let ep = ep_ptr.as_mut::<ipc::Endpoint>();
            let badge = handler_cap.get_badge();

            // 3. 执行发送 (这会阻塞当前线程)
            ipc::send(tcb, ep, badge, None);
        } else {
            panic!("Fault handler is not an Endpoint");
        }
    } else {
        unhandled_exception(e, pc, addr, status);
    }
}

fn unhandled_exception(e: TrapException, pc: usize, addr: VirtAddr, status: usize) {
    printk!(
        "{}TRAP(Exception){}: {} pc=0x{:x}, addr={}, status=0x{:x}\n",
        ANSI_RED,
        ANSI_RESET,
        e,
        pc,
        addr,
        status
    );
    panic!("Kernel panic due to unhandled exception");
}

fn unhandled_interrupt(e: TrapInterrupt, pc: usize, addr: VirtAddr, status: usize) {
    printk!(
        "{}TRAP(Interrupt){}: {} pc=0x{:x}, addr={}, status=0x{:x}\n",
        ANSI_YELLOW,
        ANSI_RESET,
        e,
        pc,
        addr,
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
            "Syscall with null cptr, method={}, epc=0x{:x}, ra=0x{:x}, sp=0x{:x}",
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
            // Delegate to irq manager to notify bound endpoint and complete
            hal::irq::complete(id, cpuid);
        }
    }
}

fn timer_ssip(sstatus_bits: usize) {
    if hal::cpu::cpu_id() == 0 {
        timer::update();
    }
    hal::irq::clear_soft();
    if (sstatus_bits & (1 << 8)) == 0 {
        scheduler::yield_proc();
    }
}

fn timer_stip(sstatus_bits: usize) {
    if hal::cpu::cpu_id() == 0 {
        timer::update();
    }
    timer::program_next_tick();
    if (sstatus_bits & (1 << 8)) == 0 {
        scheduler::yield_proc();
    }
}
