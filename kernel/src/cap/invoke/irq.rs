use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::error::Error;
use crate::hal;
use crate::irq;
use crate::proc::scheduler;

pub fn invoke_irq_handler(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let irq = if cap.cap_type() == CapType::IrqHandler {
        let irq_num = cap.value();
        irq_num
    } else {
        error!("IRQ::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("IRQ::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        irqmethod::SET_NOTIFICATION => {
            // SetNotification: args[0] = ep_cptr
            if irq == 0 {
                error!("IRQ::SetNotification: Root IRQ (0) is a control channel.");
                return Err(Error::PermissionDenied);
            }
            let ep_cptr = CapPtr::from(utcb.mrs_regs[0]);

            if let Some(ep_cap) = tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if ep_cap.cap_type() == CapType::Endpoint {
                    irq::bind_notification(irq, &ep_cap)
                } else {
                    error!(
                        "IRQ::SetNotification failed: invalid target cap type {:?}",
                        ep_cap.cap_type()
                    );
                    Err(Error::InvalidType)
                }
            } else {
                error!("IRQ::SetNotification failed: cap not found {:?}", ep_cptr);
                Err(Error::InvalidCapability)
            }
        }
        irqmethod::ACK => {
            // Ack: acknowledge handled IRQ and unmask
            if irq == 0 {
                error!("IRQ::Ack: Root IRQ (0) is a control channel.");
                return Err(Error::PermissionDenied);
            }
            let cpuid = hal::cpu::cpu_id();
            irq::ack_irq(cpuid, irq)
        }
        irqmethod::CLEAR_NOTIFICATION => {
            // Clear binding
            if irq == 0 {
                error!("IRQ::ClearNotification: Root IRQ (0) is a control channel.");
                return Err(Error::PermissionDenied);
            }
            irq::clear_notification(irq)
        }
        irqmethod::SET_PRIORITY => {
            if irq != 0 {
                error!(
                    "IRQ::SetPriority: only Root IRQ (0) can perform this action, current irq={}",
                    irq
                );
                return Err(Error::PermissionDenied);
            }
            let irq = utcb.mrs_regs[0];
            let priority = utcb.mrs_regs[1];
            hal::irq::set_priority(irq, priority);
            Ok(())
        }
        irqmethod::SET_THRESHOLD => {
            // SetThreshold: [cpu,threshold]
            if irq != 0 {
                error!(
                    "IRQ::SetThreshold: only Root IRQ (0) can perform this action, current irq={}",
                    irq
                );
                return Err(Error::PermissionDenied);
            }
            let cpuid = utcb.mrs_regs[0];
            let threshold = utcb.mrs_regs[1];
            hal::irq::set_threshold(threshold, cpuid);
            Ok(())
        }
        _ => {
            error!("IRQ::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
