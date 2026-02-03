pub mod timer;

use crate::cap;
use crate::cap::Capability;
use crate::cpu;
use crate::hal;
use crate::hal::irq::MAX_IRQS;
use crate::ipc;

use spin::Mutex;

pub fn init() {
    // 初始化 IRQ 表与定时器
    hal::irq::init();
    log!("irq: Initialized global IRQs");
}

pub fn init_cpu() {
    hal::irq::init_cpu();
    log!("irq: Initialized for cpu {}", hal::cpu::cpu_id());
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

static IRQ_TABLE: Mutex<[IrqSlot; MAX_IRQS]> = Mutex::new([const { IrqSlot::new() }; MAX_IRQS]);

/// 绑定通知对象到 IRQ（通常是 Endpoint Cap）
pub fn bind_notification(irq: usize, cap: Capability) -> bool {
    let mut tbl = IRQ_TABLE.lock();
    if irq >= MAX_IRQS {
        return false;
    }
    tbl[irq].notification = Some(cap);
    tbl[irq].enabled = true;
    true
}

pub fn clear_notification(irq: usize) -> bool {
    let mut tbl = IRQ_TABLE.lock();
    if irq >= MAX_IRQS {
        return false;
    }
    tbl[irq].notification = None;
    tbl[irq].enabled = false;
    true
}

/// 内核在 trap 中调用：处理 claim 到的 IRQ（mask + notify + complete）
pub fn handle_claimed(cpuid: usize, id: usize) {
    // 先屏蔽该 IRQ，交给驱动通过 Ack 重新打开
    hal::irq::mask(id as u32, cpuid);
    let tbl = IRQ_TABLE.lock();
    if id >= MAX_IRQS {
        // still complete the IRQ
        panic!("IRQ {} out of range of MAX_IRQS {}", id, MAX_IRQS);
    }

    if let Some(cap) = &tbl[id].notification {
        // 如果绑定了 Endpoint，直接通知（使用 badge，如果没有则 0）
        if cap.cap_type() == cap::CapType::Endpoint {
            let ep_ptr = cap.obj_ptr();
            let badge = cap.get_badge();
            let ep = ep_ptr.as_mut::<ipc::Endpoint>();
            ipc::notify(ep, badge);
        }
    } else {
        // 未绑定通知对象，直接完成
        log!("irq: IRQ {} has no bound notification, completing directly", id);
        // 对 PLIC 做 Complete（claim/complete 寄存器写入）
        hal::irq::complete(id as u32, cpuid);
        // 重新打开该 IRQ
        hal::irq::unmask(id as u32, cpuid);
    }
}

pub fn ack_irq(cpuid: usize, irq: usize) {
    // 对 PLIC 做 Complete（claim/complete 寄存器写入）
    hal::irq::complete(irq as u32, cpuid);
    // 重新打开该 IRQ
    hal::irq::unmask(irq as u32, cpuid);
}

/// 进入中断上下文
pub fn enter() {
    let cpu = cpu::get();
    cpu.nest_count += 1;
}
/// 退出中断上下文
pub fn exit() {
    let cpu = cpu::get();
    if cpu.nest_count > 0 {
        cpu.nest_count -= 1;
    }
}
