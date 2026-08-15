#![allow(dead_code)]

use crate::drivers::uart::Config as UartConfig;
use super::types::{DeviceTreeInfo, MemoryRange};
use fdt::Fdt;

pub fn parse_uart(fdt: &Fdt) -> Option<UartConfig> {
    let chosen = fdt.find_node("/chosen")?;
    let stdout_path = chosen.property("stdout-path")?.as_str()?;
    let node_path = stdout_path.split(':').next().unwrap_or(stdout_path);
    let node = fdt.find_node(node_path)?;

    UartConfig::from_fdt(&node)
}

pub fn parse_hart_count(fdt: &Fdt) -> usize {
    let mut count = 0;
    for cpu in fdt.cpus() {
        let disabled = cpu
            .property("status")
            .and_then(|prop| prop.as_str())
            .map(|status| status == "disabled")
            .unwrap_or(false);

        if !disabled {
            count += 1;
        }
    }

    core::cmp::max(count, 1)
}

pub fn parse_memory(fdt: &Fdt) -> Option<MemoryRange> {
    let memory = fdt.memory();
    let mut regions = memory.regions();
    regions.find_map(|region| {
        let start = region.starting_address as usize;
        region.size.map(|size| MemoryRange { start, size })
    })
}

pub fn parse_plic_base(fdt: &Fdt) -> Option<usize> {
    for node in fdt.all_nodes() {
        let is_plic = node
            .compatible()
            .map(|c| c.all().any(|s| s.contains("riscv,plic0") || s.contains("sifive,plic-1")))
            .unwrap_or(false);
        if !is_plic {
            continue;
        }
        if let Some(mut regs) = node.reg() {
            if let Some(region) = regs.next() {
                return Some(region.starting_address as usize);
            }
        }
    }
    None
}

pub fn parse_device_tree(fdt: &Fdt) -> DeviceTreeInfo {
    let hart_count = parse_hart_count(fdt);
    let uart = parse_uart(fdt);
    let memory = parse_memory(fdt);
    let plic_base = parse_plic_base(fdt);

    DeviceTreeInfo::new(uart, hart_count, memory, plic_base)
}
