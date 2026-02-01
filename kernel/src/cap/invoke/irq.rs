use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::hal;
use crate::irq;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_irq_handler(cap: &mut Capability, method: usize) -> usize {
    let irq = if cap.cap_type() == CapType::IrqHandler {
        cap.value()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        irqmethod::SET_NOTIFICATION => {
            // SetNotification: args[0] = ep_cptr
            let ep_cptr = CapPtr::from(utcb.mrs_regs[0]);

            if let Some(ep_cap) = tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if ep_cap.cap_type() == CapType::Endpoint {
                    irq::bind_notification(irq, ep_cap.clone());
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_OBJ_TYPE
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        irqmethod::ACK => {
            // Ack: acknowledge handled IRQ and unmask
            let cpuid = hal::cpu::cpu_id();
            irq::ack_irq(cpuid, irq);
            errcode::SUCCESS
        }
        irqmethod::CLEAR_NOTIFICATION => {
            // Clear binding
            irq::clear_notification(irq);
            errcode::SUCCESS
        }
        irqmethod::SET_PRIORITY => {
            // SetPriority: args[0] = priority
            let priority = utcb.mrs_regs[0];
            hal::irq::set_priority(irq as u32, priority as u8);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}
