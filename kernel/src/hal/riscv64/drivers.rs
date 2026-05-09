use crate::drivers::intr::{InterruptController, aplic, plic};
use crate::drivers::uart::{Uart, ns16550a, pl011};
use crate::mem::PageTable;
use crate::sync::Once;

pub enum UartDriver {
    Ns16550a(ns16550a::Ns16550a),
    Pl011(pl011::Pl011),
}

impl UartDriver {
    pub fn map_mmio(&self, kpt: &mut PageTable) {
        match self {
            UartDriver::Ns16550a(uart) => uart.map_mmio(kpt),
            UartDriver::Pl011(uart) => uart.map_mmio(kpt),
        }
    }

    pub fn putc(&self, c: u8) {
        match self {
            UartDriver::Ns16550a(uart) => uart.putc(c),
            UartDriver::Pl011(uart) => uart.putc(c),
        }
    }

    pub fn getc(&self) -> Option<u8> {
        match self {
            UartDriver::Ns16550a(uart) => uart.getc(),
            UartDriver::Pl011(uart) => uart.getc(),
        }
    }
}

pub enum IntcDriver {
    Plic(plic::Plic),
    Aplic(aplic::Aplic),
}

impl IntcDriver {
    pub fn map_mmio(&self, kpt: &mut PageTable) {
        match self {
            IntcDriver::Plic(intc) => intc.map_mmio(kpt),
            IntcDriver::Aplic(intc) => intc.map_mmio(kpt),
        }
    }

    pub fn set_priority(&self, irq: usize, priority: usize) {
        match self {
            IntcDriver::Plic(intc) => intc.set_priority(irq, priority),
            IntcDriver::Aplic(intc) => intc.set_priority(irq, priority),
        }
    }

    pub fn set_enable(&self, hartid: usize, irq: usize, enable: bool) {
        match self {
            IntcDriver::Plic(intc) => intc.set_enable(hartid, irq, enable),
            IntcDriver::Aplic(intc) => intc.set_enable(hartid, irq, enable),
        }
    }

    pub fn set_threshold(&self, hartid: usize, threshold: usize) {
        match self {
            IntcDriver::Plic(intc) => intc.set_threshold(hartid, threshold),
            IntcDriver::Aplic(intc) => intc.set_threshold(hartid, threshold),
        }
    }

    pub fn claim(&self, hartid: usize) -> usize {
        match self {
            IntcDriver::Plic(intc) => intc.claim(hartid),
            IntcDriver::Aplic(intc) => intc.claim(hartid),
        }
    }

    pub fn complete(&self, hartid: usize, irq: usize) {
        match self {
            IntcDriver::Plic(intc) => intc.complete(hartid, irq),
            IntcDriver::Aplic(intc) => intc.complete(hartid, irq),
        }
    }
}

pub enum FbDriver {
    Framebuffer(crate::drivers::fb::FramebufferWriter),
}

impl FbDriver {
    pub fn map_mmio(&self, _kpt: &mut PageTable) {
        match self {
            FbDriver::Framebuffer(fb) => {
                let _info = fb.get_info();
            }
        }
    }

    pub fn write_fmt(&self, args: core::fmt::Arguments) -> core::fmt::Result {
        match self {
            FbDriver::Framebuffer(fb) => {
                use core::fmt::Write;
                crate::drivers::fb::FbWriter(fb).write_fmt(args)
            }
        }
    }
}

pub static UART: Once<UartDriver> = Once::new();
pub static INTC: Once<IntcDriver> = Once::new();
pub static FB: Once<FbDriver> = Once::new();
