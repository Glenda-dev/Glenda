#![allow(dead_code)]

use crate::drivers::uart::Config as UartConfig;
use core::cmp;

#[derive(Debug, Clone, Copy)]
pub struct MemoryRange {
    pub start: usize,
    pub size: usize,
}

impl MemoryRange {
    pub fn end(&self) -> usize {
        self.start + self.size
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceTreeInfo {
    uart: Option<UartConfig>,
    hart_count: usize,
    memory: Option<MemoryRange>,
    plic_base: Option<usize>,
}

impl DeviceTreeInfo {
    pub(crate) fn new(
        uart: Option<UartConfig>,
        hart_count: usize,
        memory: Option<MemoryRange>,
        plic_base: Option<usize>,
    ) -> Self {
        Self { uart, hart_count, memory, plic_base }
    }

    pub fn uart(&self) -> Option<UartConfig> {
        self.uart
    }

    pub fn hart_count(&self) -> usize {
        cmp::max(self.hart_count, 1)
    }

    pub fn memory(&self) -> Option<MemoryRange> {
        self.memory
    }

    pub fn plic_base(&self) -> Option<usize> {
        self.plic_base
    }
}
