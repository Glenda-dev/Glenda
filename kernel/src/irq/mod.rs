pub mod plic;

use crate::cap;
use crate::cap::Capability;
use crate::ipc;
use crate::printk;
use spin::Mutex;

const MAX_IRQS: usize = 64;

pub fn init() {
    // 初始化 IRQ 表与定时器
    // init_table(); // No longer needed
    printk!("irq: Initialized global IRQs\n");
}

pub fn init_hart(hartid: usize) {
    // 设置 PLIC 阈值为 0，允许所有优先级 > 0 的中断
    plic::set_threshold_s(hartid, 0);
    printk!("irq: Initialized for hart {}\n", hartid);
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
pub fn handle_claimed(hartid: usize, id: usize) {
    // 先屏蔽该 IRQ，交给驱动通过 Ack 重新打开
    plic::set_enable_s(hartid, id, false);
    let tbl = IRQ_TABLE.lock();
    if id >= MAX_IRQS {
        // still complete the IRQ
        plic::set_claim_s(hartid, id);
        return;
    }

    if let Some(cap) = &tbl[id].notification {
        // 如果绑定了 Endpoint，直接通知（使用 badge，如果没有则 0）
        if let cap::CapType::Endpoint { ep_ptr } = cap.object {
            let badge = cap.badge.unwrap_or(0usize);
            let ep = ep_ptr.as_mut::<ipc::Endpoint>();
            ipc::notify(ep, badge);
        }
    }

    // 对 PLIC 做 Complete（claim/complete 寄存器写入）
    plic::set_claim_s(hartid, id);
}

/// 驱动调用：处理 IRQ Ack（解除屏蔽）
pub fn ack_irq(hartid: usize, irq: usize) {
    plic::set_enable_s(hartid, irq, true);
}
