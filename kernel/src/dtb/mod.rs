#![allow(dead_code)]

mod parser;
mod types;

use crate::drivers::uart::Config as UartConfig;
use crate::printk;
use fdt::Fdt;
use spin::Once;

pub use types::{DeviceTreeInfo, MemoryRange};

static DEVICE_TREE: Once<DeviceTreeInfo> = Once::new();

fn _init(dtb: *const u8) -> Result<&'static DeviceTreeInfo, fdt::FdtError> {
    let fdt = unsafe { Fdt::from_ptr(dtb)? };
    let info = parser::parse_device_tree(&fdt);
    
    Ok(DEVICE_TREE.call_once(|| info))
}

pub fn hart_count() -> usize {
    DEVICE_TREE.get().map(DeviceTreeInfo::hart_count).unwrap_or(1)
}

pub fn uart_config() -> Option<UartConfig> {
    DEVICE_TREE.get().and_then(DeviceTreeInfo::uart)
}

pub fn memory_range() -> Option<MemoryRange> {
    DEVICE_TREE.get().and_then(DeviceTreeInfo::memory)
}

pub fn plic_base() -> Option<usize> {
    DEVICE_TREE.get().and_then(DeviceTreeInfo::plic_base)
}

pub fn init(dtb: *const u8) {
    // 解析设备树
    let dtb_result = _init(dtb);
    match dtb_result {
        Ok(_) => {
            printk!("Device tree blob at {:p}\n", dtb);
            printk!("{} harts detected\n", hart_count());
        }
        Err(err) => {
            panic!("Device tree parsing failed: {:?}\n", err);
        }
    }
}
