use super::super::super::plic;
use super::uart;
use crate::hart;

// 外设中断处理 (基于PLIC，lab-3只需要识别和处理UART中断)
pub fn interrupt_handler() {
    let hartid = hart::getid();
    let id = plic::claim(hartid);
    if id == 0 {
        return;
    }

    if id == plic::UART_IRQ {
        uart::interrupt_handler();
    }

    plic::complete(hartid, id);
}
