use super::super::method::*;
use crate::cap::{Badge, CapPtr, CapType, Capability};
use crate::error::Error;
use crate::hal;
use crate::irq;
use crate::proc::scheduler;

pub fn invoke_irq_handler(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let irq = if cap.cap_type() == CapType::IrqHandler {
        let badge = cap.get_badge().get();
        if badge == 0 {
            // 这是 Master IRQ Capability (IRQ Control)
            // 只有 mint 操作有意义，这里应返回错误
            error!("IRQ::invoke failed: IRQ control capability cannot be invoked directly");
            return Err(Error::InvalidCapability);
        }
        badge
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
            let ep_cptr = CapPtr::from(utcb.mrs_regs[0]);

            if let Some(ep_cap) = tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if ep_cap.cap_type() == CapType::Endpoint {
                    // 使用 IrqHandler 能力携带的 Badge 作为通知时的标识
                    let mut badged_ep = ep_cap.clone();
                    badged_ep.set_badge(Badge::from(irq));
                    irq::bind_notification(irq, &badged_ep)
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
            let cpuid = hal::cpu::cpu_id();
            irq::ack_irq(cpuid, irq)
        }
        irqmethod::CLEAR_NOTIFICATION => {
            // Clear binding
            irq::clear_notification(irq)
        }
        irqmethod::SET_PRIORITY => {
            // SetPriority: args[0] = priority
            let priority = utcb.mrs_regs[0];
            hal::irq::set_priority(irq, priority);
            Ok(())
        }
        _ => {
            error!("IRQ::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
