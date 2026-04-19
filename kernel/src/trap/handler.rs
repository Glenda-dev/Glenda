use super::{TrapCause, TrapException, TrapInterrupt};
use crate::cap::method::{ipcmethod, replymethod};
use crate::cap::{Badge, CapPtr, CapType, Capability, Rights, Slot};
use crate::error::Error;
use crate::hal;
use crate::hal::trap::TrapFrame;
use crate::ipc;
use crate::ipc::protocol;
use crate::ipc::{MsgFlags, MsgTag};
use crate::irq;
use crate::printk::{ANSI_RED, ANSI_RESET, ANSI_YELLOW};
use crate::proc::TCB;
use crate::proc::scheduler;
use crate::proc::virt::vcpu_state_from_cap;
use crate::trap::syscall;

#[unsafe(no_mangle)]
pub extern "C" fn trap_kernel_handler(ctx: &mut TrapFrame) {
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
                "Unhandled trap: {}, cause: {:#x}, pc: {:#x}, value: {:#x}, status: {:#x}",
                code, cause, pc, value, status
            );
        }
    }
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
    if let Some(ptr) = scheduler::current() {
        let tcb = unsafe { &mut *ptr };
        if e == TrapException::Syscall && syscall_handler(ctx) {
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
            printk_unsynced!(
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
    if matches!(
        e,
        TrapException::GuestPageFault
            | TrapException::VirtualInstruction
            | TrapException::VirtualSupervisorSyscall
    ) && let Some(vcpu_cap) = tcb.bound_vcpu.as_ref()
        && let Ok(vcpu) = vcpu_state_from_cap(vcpu_cap)
    {
        vcpu.exit_reason = match e {
            TrapException::GuestPageFault => crate::proc::virt::VcpuExitReason::GuestPageFault,
            TrapException::VirtualInstruction => {
                crate::proc::virt::VcpuExitReason::VirtualInstruction
            }
            _ => crate::proc::virt::VcpuExitReason::HostTrap,
        };
        vcpu.exit_detail0 = value;
        vcpu.exit_detail1 = cause;
        vcpu.exit_detail2 = pc;
    }

    log!(
        "trap: Fault in thread {:p}: {}, cause={:#x}, pc={:#x}, value={:#x}, status={:#x}",
        tcb,
        e,
        cause,
        pc,
        value,
        status
    );
    if let Some(handler_cap) = tcb.fault_handler.clone()
        && handler_cap.cap_type() == CapType::Endpoint
    {
        // 1. 将异常详情写入 UTCB (IPC Buffer)
        if let Some(utcb) = tcb.get_utcb() {
            let (label, flags) = match e {
                TrapException::PageFault => {
                    utcb.mrs_regs[0] = value; // addr
                    utcb.mrs_regs[1] = pc; // pc
                    utcb.mrs_regs[2] = cause; // cause
                    (protocol::PAGE_FAULT, MsgFlags::NONE)
                }
                TrapException::IllegalInstruction => {
                    utcb.mrs_regs[0] = value; // instruction
                    utcb.mrs_regs[1] = pc; // pc
                    (protocol::ILLEGAL_INSTRUCTION, MsgFlags::NONE)
                }
                TrapException::Breakpoint => {
                    utcb.mrs_regs[0] = pc; // pc
                    (protocol::BREAKPOINT, MsgFlags::NONE)
                }
                TrapException::AccessFault => {
                    utcb.mrs_regs[0] = value; // addr
                    utcb.mrs_regs[1] = pc; // pc
                    (protocol::ACCESS_FAULT, MsgFlags::NONE)
                }
                TrapException::AccessMisaligned => {
                    utcb.mrs_regs[0] = value; // addr
                    utcb.mrs_regs[1] = pc; // pc
                    (protocol::ACCESS_MISALIGNED, MsgFlags::NONE)
                }
                TrapException::Syscall => {
                    utcb.mrs_regs = ctx.get_syscall_registers();
                    utcb.mrs = utcb.mrs_regs.len();
                    (protocol::SYSCALL, MsgFlags::HAS_MRS)
                }
                TrapException::GuestPageFault => {
                    utcb.mrs_regs[0] = value; // guest fault addr
                    utcb.mrs_regs[1] = pc; // host trap pc
                    utcb.mrs_regs[2] = cause; // scause
                    (protocol::VIRT_EXIT, MsgFlags::NONE)
                }
                TrapException::VirtualInstruction => {
                    utcb.mrs_regs[0] = value; // trapping instruction encoding/value
                    utcb.mrs_regs[1] = pc; // pc
                    utcb.mrs_regs[2] = cause; // scause
                    (protocol::VIRT_EXIT, MsgFlags::NONE)
                }
                TrapException::VirtualSupervisorSyscall => {
                    utcb.mrs_regs[0] = cause; // scause
                    utcb.mrs_regs[1] = value; // stval/htval proxy
                    utcb.mrs_regs[2] = pc; // pc
                    (protocol::VIRT_EXIT, MsgFlags::NONE)
                }
                _ => {
                    utcb.mrs_regs[0] = cause; // cause
                    utcb.mrs_regs[1] = value; // value
                    utcb.mrs_regs[2] = pc; // pc
                    (protocol::UNKNOWN_FAULT, MsgFlags::NONE)
                }
            };

            utcb.msg_tag = MsgTag::new(protocol::KERNEL_PROTO, label, flags);
        }
        let ep_ptr = handler_cap.obj_ptr();
        let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
        let badge = handler_cap.get_badge();

        // 3. 执行 Call (这会阻塞当前线程，直到收到 Reply)
        ipc::call(tcb, ep, badge, None).unwrap_or_else(|err| {
            error!(
                "trap: Fault handler IPC call failed: {:?}, terminating thread. Fault: {}, cause={:#x}, pc={:#x}\n",
                err,
                e,
                cause,
                pc
            );
            scheduler::block_current_thread();
        });

        // 4. 如果是Syscall，跳过epc
        if e == TrapException::Syscall {
            if tcb.upcall_delivery_armed {
                // 已由 TCB::DeliverUpcall 预设用户态返回现场（epc/ra/a0..a3）。
                // 这里必须避免默认 syscall 返回流程覆盖寄存器。
                tcb.upcall_delivery_armed = false;
            } else {
                ctx.advance_pc();
                if let Some(utcb) = tcb.get_utcb() {
                    ctx.set_return_value(utcb.mrs_regs[0]);
                } else {
                    warn!(
                        "trap: Syscall fault handler returned but UTCB is missing. Returning error to caller."
                    );
                    ctx.set_return_value(usize::MAX);
                }
            }
        }
    } else {
        unhandled_exception(e, cause, pc, value, status);
    }
}

fn unhandled_exception(e: TrapException, cause: usize, pc: usize, value: usize, status: usize) {
    printk_unsynced!(
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
    printk_unsynced!(
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

fn lookup_fast_ipc_slot(tcb: &TCB, cptr: usize) -> Option<*mut Slot> {
    let slot_ptr = tcb.lookup_slot(CapPtr::from(cptr))?;
    let cap_type = unsafe { (*slot_ptr).cap.cap_type() };
    if matches!(cap_type, CapType::Endpoint | CapType::Reply) { Some(slot_ptr) } else { None }
}

#[inline(always)]
fn fast_ipc_needs_sync_in(cap_type: CapType, method: usize) -> bool {
    match cap_type {
        CapType::Endpoint => matches!(method, ipcmethod::SEND | ipcmethod::CALL | ipcmethod::PROXY),
        CapType::Reply => method == replymethod::REPLY,
        _ => false,
    }
}

#[inline(always)]
fn fast_ipc_needs_sync_out(cap_type: CapType, method: usize) -> bool {
    match cap_type {
        CapType::Endpoint => matches!(method, ipcmethod::RECV | ipcmethod::CALL),
        CapType::Reply => false,
        _ => false,
    }
}

#[inline(always)]
fn fast_ipc_can_inline_payload(msgtag: MsgTag) -> bool {
    let flags = msgtag.flags();
    !flags.intersects(MsgFlags::HAS_CAP | MsgFlags::HAS_BUFFER | MsgFlags::HAS_MRS)
}

fn dispatch_fast_ipc_inline(
    tcb: &mut TCB,
    slot_ptr: *mut Slot,
    method: usize,
    fast_msgtag: MsgTag,
    fast_mrs: [usize; 4],
) -> Option<usize> {
    let cap_type = unsafe { (*slot_ptr).cap.cap_type() };

    match cap_type {
        CapType::Endpoint => {
            let cap = unsafe { &(*slot_ptr).cap };
            let ep_ptr = cap.obj_ptr();
            let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
            let badge = cap.get_badge();

            let ret = match method {
                ipcmethod::SEND => {
                    if !cap.has_rights(Rights::SEND) {
                        Err(Error::PermissionDenied)
                    } else {
                        match ipc::send_inline_if_ready(tcb, ep, badge, fast_msgtag, fast_mrs) {
                            Ok(true) => Ok(()),
                            Ok(false) => return None,
                            Err(e) => Err(e),
                        }
                    }
                }
                ipcmethod::CALL => {
                    if !cap.has_rights(Rights::CALL) {
                        Err(Error::PermissionDenied)
                    } else {
                        match ipc::call_inline_if_ready(tcb, ep, badge, fast_msgtag, fast_mrs) {
                            Ok(true) => Ok(()),
                            Ok(false) => return None,
                            Err(e) => Err(e),
                        }
                    }
                }
                _ => return None,
            };

            Some(match ret {
                Ok(_) => Error::Success as usize,
                Err(e) => e as usize,
            })
        }
        CapType::Reply => {
            if method != replymethod::REPLY {
                return None;
            }

            let target_tcb = unsafe { (*slot_ptr).cap.obj_ptr().as_mut::<TCB>() };
            let ret = ipc::reply_inline(tcb, target_tcb, fast_msgtag, fast_mrs);

            // Reply cap is one-shot; consume it regardless of delivery result.
            unsafe {
                (*slot_ptr).cap = Capability::empty();
            }

            Some(match ret {
                Ok(_) => Error::Success as usize,
                Err(e) => e as usize,
            })
        }
        _ => None,
    }
}

fn dispatch_fast_recv_notify_inline(slot_ptr: *mut Slot, method: usize) -> Option<(usize, Badge)> {
    if method != ipcmethod::RECV {
        return None;
    }

    let cap = unsafe { &(*slot_ptr).cap };
    if cap.cap_type() != CapType::Endpoint || !cap.has_rights(Rights::RECV) {
        return None;
    }

    let ep_ptr = cap.obj_ptr();
    let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
    let pending = ep.poll_notification();
    if pending.is_null() {
        return None;
    }

    Some((
        MsgTag::new(protocol::KERNEL_PROTO, protocol::NOTIFY, MsgFlags::NONE).as_usize(),
        pending,
    ))
}

fn dispatch_fast_ipc(
    tcb: &mut TCB,
    slot_ptr: *mut Slot,
    method: usize,
    fast_badge: Badge,
) -> Option<usize> {
    let cap_type = unsafe { (*slot_ptr).cap.cap_type() };

    match cap_type {
        CapType::Endpoint => {
            let cap = unsafe { &(*slot_ptr).cap };
            let ep_ptr = cap.obj_ptr();
            let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
            let badge = cap.get_badge();

            let ret = match method {
                ipcmethod::SEND => {
                    if !cap.has_rights(Rights::SEND) {
                        Err(Error::PermissionDenied)
                    } else {
                        let cap_to_send = ipc::transfer_cap(tcb);
                        ipc::send(tcb, ep, badge, cap_to_send)
                    }
                }
                ipcmethod::RECV => {
                    if !cap.has_rights(Rights::RECV) {
                        Err(Error::PermissionDenied)
                    } else {
                        ipc::recv(tcb, ep)
                    }
                }
                ipcmethod::CALL => {
                    if !cap.has_rights(Rights::CALL) {
                        Err(Error::PermissionDenied)
                    } else {
                        let cap_to_send = ipc::transfer_cap(tcb);
                        ipc::call(tcb, ep, badge, cap_to_send)
                    }
                }
                ipcmethod::NOTIFY => {
                    if !cap.has_rights(Rights::SEND) {
                        Err(Error::PermissionDenied)
                    } else {
                        ipc::notify(ep, fast_badge)
                    }
                }
                ipcmethod::PROXY => {
                    if !cap.has_rights(Rights::CUSTOM) {
                        Err(Error::PermissionDenied)
                    } else {
                        let cap_to_send = ipc::transfer_cap(tcb);
                        ipc::proxy(tcb, ep, cap_to_send)
                    }
                }
                _ => Err(Error::InvalidMethod),
            };

            Some(match ret {
                Ok(_) => Error::Success as usize,
                Err(e) => e as usize,
            })
        }
        CapType::Reply => {
            if method != replymethod::REPLY {
                return Some(Error::InvalidMethod as usize);
            }

            let target_tcb = unsafe { (*slot_ptr).cap.obj_ptr().as_mut::<TCB>() };
            let cap_to_send = ipc::transfer_cap(tcb);
            let ret = ipc::reply(tcb, target_tcb, cap_to_send);

            // Reply cap is one-shot; consume it regardless of delivery result.
            unsafe {
                (*slot_ptr).cap = Capability::empty();
            }

            Some(match ret {
                Ok(_) => Error::Success as usize,
                Err(e) => e as usize,
            })
        }
        _ => None,
    }
}

fn sync_ipc_regs_to_utcb(tcb: &mut TCB, ctx: &TrapFrame) -> Result<(), Error> {
    let utcb = tcb.get_utcb().ok_or(Error::MappingFailed)?;
    let (msgtag, badge, mrs) = ctx.get_syscall_ipc_args();
    utcb.msg_tag = MsgTag(msgtag);
    utcb.badge = Badge::from(badge);
    utcb.mrs_regs[0] = mrs[0];
    utcb.mrs_regs[1] = mrs[1];
    utcb.mrs_regs[2] = mrs[2];
    utcb.mrs_regs[3] = mrs[3];
    Ok(())
}

fn sync_ipc_regs_from_utcb(tcb: &mut TCB, ctx: &mut TrapFrame) -> Result<(), Error> {
    let utcb = tcb.get_utcb().ok_or(Error::MappingFailed)?;
    ctx.set_syscall_ipc_ret(
        utcb.msg_tag.as_usize(),
        utcb.badge.bits(),
        [utcb.mrs_regs[0], utcb.mrs_regs[1], utcb.mrs_regs[2], utcb.mrs_regs[3]],
    );
    Ok(())
}

#[inline(always)]
fn finish_syscall(ctx: &mut TrapFrame, ret: usize) -> bool {
    ctx.set_return_value(ret);
    ctx.advance_pc();
    ctx.set_fast_ipc_hint(false);
    true
}

fn syscall_handler(ctx: &mut TrapFrame) -> bool {
    let (syscall_no, cptr) = ctx.get_syscall_args();
    let Some(method) = crate::cap::method::decode_invoke(syscall_no) else {
        return false;
    };

    let tcb_ptr = scheduler::current().expect("Native syscall with no current thread");
    let tcb = unsafe { &mut *tcb_ptr };

    if cptr == 0 {
        let epc = ctx.get_epc();
        let ra = ctx.get_ra();
        let sp = ctx.get_sp();
        error!(
            "trap: Native syscall with null cptr at thread {:p}, syscall_no={}, method={:#x}, epc={:#x}, ra={:#x}, sp={:#x}",
            tcb_ptr, syscall_no, method, epc, ra, sp
        );
        ctx.set_return_value(Error::InvalidCapability as usize);
        ctx.advance_pc();
        return true;
    }
    let fast_slot = lookup_fast_ipc_slot(tcb, cptr);
    let is_fast_ipc = fast_slot.is_some();
    ctx.set_fast_ipc_hint(is_fast_ipc);

    if let Some(slot_ptr) = fast_slot {
        let cap_type = unsafe { (*slot_ptr).cap.cap_type() };
        let (fast_msgtag_bits, fast_badge_bits, fast_mrs) = ctx.get_syscall_ipc_args();
        let fast_msgtag = MsgTag(fast_msgtag_bits);
        let fast_badge = Badge::from(fast_badge_bits);

        if let Some((msgtag, badge)) = dispatch_fast_recv_notify_inline(slot_ptr, method) {
            ctx.set_syscall_ipc_ret(msgtag, badge.bits(), [0; 4]);
            return finish_syscall(ctx, Error::Success as usize);
        }

        if fast_ipc_can_inline_payload(fast_msgtag)
            && let Some(ret) =
                dispatch_fast_ipc_inline(tcb, slot_ptr, method, fast_msgtag, fast_mrs)
        {
            if ret == Error::InvalidSlot as usize || ret == Error::InvalidCapability as usize {
                error!(
                    "trap: native fast inline dispatch failed: thread={:p}, syscall_no={}, method={:#x}, cptr={:#x}, ret={:#x}, a7={:#x}, epc={:#x}",
                    tcb_ptr,
                    syscall_no,
                    method,
                    cptr,
                    ret,
                    ctx.get_syscall_registers()[7],
                    ctx.get_epc(),
                );
            }

            if fast_ipc_needs_sync_out(cap_type, method)
                && let Err(e) = sync_ipc_regs_from_utcb(tcb, ctx)
                && ret == Error::Success as usize
            {
                return finish_syscall(ctx, e as usize);
            }

            return finish_syscall(ctx, ret);
        }

        let needs_sync_in = fast_ipc_needs_sync_in(cap_type, method);
        let needs_sync_out = fast_ipc_needs_sync_out(cap_type, method);

        if needs_sync_in && let Err(e) = sync_ipc_regs_to_utcb(tcb, ctx) {
            return finish_syscall(ctx, e as usize);
        }

        if let Some(ret) = dispatch_fast_ipc(tcb, slot_ptr, method, fast_badge) {
            if ret == Error::InvalidSlot as usize || ret == Error::InvalidCapability as usize {
                error!(
                    "trap: native fast dispatch failed: thread={:p}, syscall_no={}, method={:#x}, cptr={:#x}, ret={:#x}, a7={:#x}, epc={:#x}",
                    tcb_ptr,
                    syscall_no,
                    method,
                    cptr,
                    ret,
                    ctx.get_syscall_registers()[7],
                    ctx.get_epc(),
                );
            }

            if needs_sync_out
                && let Err(e) = sync_ipc_regs_from_utcb(tcb, ctx)
                && ret == Error::Success as usize
            {
                return finish_syscall(ctx, e as usize);
            }

            return finish_syscall(ctx, ret);
        }
    }

    let ret = syscall::dispatch(cptr, method);

    if ret == Error::InvalidSlot as usize || ret == Error::InvalidCapability as usize {
        error!(
            "trap: native dispatch failed: thread={:p}, syscall_no={}, method={:#x}, cptr={:#x}, ret={:#x}, a7={:#x}, epc={:#x}",
            tcb_ptr,
            syscall_no,
            method,
            cptr,
            ret,
            ctx.get_syscall_registers()[7],
            ctx.get_epc(),
        );
    }

    if is_fast_ipc {
        if let Err(e) = sync_ipc_regs_from_utcb(tcb, ctx)
            && ret == Error::Success as usize
        {
            return finish_syscall(ctx, e as usize);
        }
    }

    finish_syscall(ctx, ret)
}

// 外设中断处理
fn external_handler() {
    let cpuid = hal::cpu::cpu_id();
    let id = hal::irq::claim(cpuid);
    match id {
        None => return,
        Some(id) => irq::handle_claimed(cpuid, id as usize)
            .unwrap_or_else(|e| error!("trap: Failed to handle external interrupt: {:?}\n", e)),
    }
}

fn timer_ssip(status: usize) {
    hal::irq::clear_soft();
    if hal::trap::is_user_mode(status) {
        scheduler::yield_proc();
    }
}

fn timer_stip(status: usize) {
    irq::timer::program_next_tick();
    if hal::trap::is_user_mode(status) {
        if let Some(tcb_ptr) = scheduler::current() {
            let tcb = unsafe { &*tcb_ptr };
            if tcb.timeslice == 0 {
                scheduler::yield_proc();
            }
        }
    }
}
