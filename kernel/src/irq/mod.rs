pub mod timer;

use crate::cap;
use crate::cap::Capability;
use crate::error::Error;
use crate::hal;
use crate::hal::irq::MAX_IRQS;
use crate::ipc;
use crate::sync::RwLock;

pub fn init() {
    // 初始化 IRQ 表与定时器
    hal::irq::init();
    log!("irq: Initialized global IRQs");
}

pub fn init_cpu() {
    hal::irq::init_cpu();
    hal::timer::init();
    timer::program_next_tick();
    let cpuid = hal::cpu::cpu_id();
    log!("irq: Initialized for cpu {}", cpuid);
}

#[derive(Clone)]
pub struct IrqSlot {
    pub notification: Option<Capability>,
    pub enabled: bool,
}

impl IrqSlot {
    const fn new() -> Self {
        Self { notification: None, enabled: false }
    }
}

#[derive(Clone, Copy)]
pub struct IRQ(usize);

impl IRQ {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }
    pub const fn id(&self) -> usize {
        self.0
    }
}

static IRQ_TABLE: RwLock<[IrqSlot; MAX_IRQS]> = RwLock::new([const { IrqSlot::new() }; MAX_IRQS]);

/// 绑定通知对象到 IRQ（通常是 Endpoint Cap）
pub fn bind_notification(irq: usize, cap: &Capability) -> Result<(), Error> {
    log!("irq: Binding irq: {} to cap: {:p}", irq, cap);
    let mut tbl = IRQ_TABLE.write();
    if irq >= MAX_IRQS {
        return Err(Error::InvalidAddress);
    }
    tbl[irq].notification = Some(cap.clone());
    tbl[irq].enabled = true;

    // Enable IRQ in PLIC
    let cpuid = hal::cpu::cpu_id();
    hal::irq::unmask(irq, cpuid);

    Ok(())
}

pub fn clear_notification(irq: usize) -> Result<(), Error> {
    log!("irq: Clearing irq: {}", irq);
    let mut tbl = IRQ_TABLE.write();
    if irq >= MAX_IRQS {
        return Err(Error::InvalidAddress);
    }
    tbl[irq].notification = None;
    tbl[irq].enabled = false;
    Ok(())
}

/// 内核在 trap 中调用：处理 claim 到的 IRQ（mask + notify + complete）
pub fn handle_claimed(cpuid: usize, id: usize) -> Result<(), Error> {
    // 1. Mask interrupt
    hal::irq::mask(id, cpuid);
    hal::irq::complete(id, cpuid);

    let tbl = IRQ_TABLE.read();
    if id >= MAX_IRQS {
        return Err(Error::InvalidAddress);
    }

    if let Some(cap) = &tbl[id].notification {
        if cap.cap_type() == cap::CapType::Endpoint {
            let ep_ptr = cap.obj_ptr();
            let badge = cap.get_badge();
            let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
            if let Err(e) = ipc::notify(ep, badge) {
                error!("irq: Notify failed for irq {}: {:?}", id, e);
                return Err(e);
            }
            return Ok(());
        }
        Err(Error::InvalidCapability)
    } else {
        warn!("irq: IRQ {} has no bound notification, completing directly", id);
        hal::irq::unmask(id, cpuid);
        Ok(())
    }
}

pub fn ack_irq(cpuid: usize, irq: usize) -> Result<(), Error> {
    // Only unmask. Completion was done in handle_claimed.
    hal::irq::unmask(irq, cpuid);
    Ok(())
}
