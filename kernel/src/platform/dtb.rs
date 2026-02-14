use crate::hal;
use crate::mem::PhysAddr;
use crate::platform::{BusType, DeviceDesc, DeviceKind, MemoryType, PlatformInfo};
use fdt::Fdt;

pub fn parse(dtb_ptr: usize) -> PlatformInfo {
    let mut info = PlatformInfo::new();
    let fdt = unsafe { Fdt::from_ptr(dtb_ptr as *const u8).expect("Failed to parse FDT") };

    info.cpu_count = fdt.cpus().count();

    // Timebase frequency
    info.clock_freq = fdt
        .find_node("/cpus")
        .and_then(|cpus| cpus.property("timebase-frequency"))
        .and_then(|prop| {
            let mut val = 0u64;
            for &b in prop.value {
                val = (val << 8) | (b as u64);
            }
            Some(val as usize)
        })
        .unwrap_or(10000000);

    // Model name
    let model = fdt.root().model();
    let model_bytes = model.as_bytes();
    let len = model_bytes.len().min(63);
    info.model_name[..len].copy_from_slice(&model_bytes[..len]);

    // Memory
    for region in fdt.memory().regions() {
        if let Some(r_size) = region.size {
            info.add_memory(
                PhysAddr::from(region.starting_address as usize),
                r_size,
                MemoryType::Ram,
            );
        }
    }

    // Devices (simple search for UART and Interrupt Controller)
    for node in fdt.all_nodes() {
        if let Some(compatible) = node.compatible() {
            let mut kind = DeviceKind::Unknown;
            if compatible.all().any(|c| c.contains("uart") || c.contains("16550")) {
                kind = DeviceKind::Uart;
            } else if compatible.all().any(|c| c.contains("plic")) {
                kind = DeviceKind::Intc;
                if let Some(prop) = node.property("riscv,ndev") {
                    let mut val = 0usize;
                    for &b in prop.value {
                        val = (val << 8) | (b as usize);
                    }
                    info.irq_count = val;
                }
            }

            if kind != DeviceKind::Unknown {
                if let Some(mut reg_iter) = node.reg() {
                    if let Some(reg) = reg_iter.next() {
                        let mut dev = DeviceDesc {
                            compatible: [0; 64],
                            base_addr: PhysAddr::from(reg.starting_address as usize),
                            size: reg.size.unwrap_or(0x1000),
                            irq: 0,
                            kind,
                            parent_index: u32::MAX,
                            bus_type: BusType::Platform,
                        };
                        let comp_bytes = compatible.first().as_bytes();
                        let c_len = comp_bytes.len().min(63);
                        dev.compatible[..c_len].copy_from_slice(&comp_bytes[..c_len]);
                        info.add_device(dev);
                        // Register MMIO region
                        if let Some(r_size) = reg.size {
                            info.add_memory(
                                PhysAddr::from(reg.starting_address as usize),
                                r_size,
                                MemoryType::Mmio,
                            );
                        }
                    }
                }
            }
        }
    }
    hal::platform::parse_dtb(&fdt, &mut info);
    info
}
