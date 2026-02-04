use super::console::Config as UartConfig;
use crate::mem::MemoryRange;
use crate::mem::PhysAddr;
use crate::platform::{BusType, DeviceDesc, DeviceKind, MemoryRegion, MemoryType, PlatformInfo};
use fdt::Fdt;
use fdt::node::FdtNode;
use spin::Once;

const MAX_MMIO_REGIONS: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct DeviceTreeInfo {
    pub uart: Option<UartConfig>,
    pub plic: Option<MemoryRange>,
    pub dtb_paddr: usize,
    pub dtb_size: usize,
    pub hart_count: usize,
}

impl DeviceTreeInfo {
    fn new(fdt: &Fdt, dtb_paddr: usize) -> Self {
        let uart = parse_uart(fdt);
        let plic = parse_plic(fdt);
        let dtb_size = fdt.total_size();
        let hart_count = parse_hart_count(fdt);
        Self { uart, plic, dtb_paddr, dtb_size, hart_count }
    }

    fn uart(&self) -> Option<UartConfig> {
        self.uart
    }

    fn plic(&self) -> Option<MemoryRange> {
        self.plic
    }
}
static DEVICE_TREE_INFO: Once<DeviceTreeInfo> = Once::new();

pub fn init(dtb: *const u8) {
    let dtb = dtb as *const u8;
    let fdt = unsafe { Fdt::from_ptr(dtb).expect("Failed to parse FDT") };
    DEVICE_TREE_INFO.call_once(|| DeviceTreeInfo::new(&fdt, dtb as usize));
}

pub fn uart_config() -> Option<UartConfig> {
    DEVICE_TREE_INFO.get().and_then(DeviceTreeInfo::uart)
}

pub fn plic() -> Option<MemoryRange> {
    DEVICE_TREE_INFO.get().and_then(DeviceTreeInfo::plic)
}

pub fn dtb_addr() -> usize {
    let info = DEVICE_TREE_INFO.get().expect("Device Tree Not Initialzed");
    info.dtb_paddr
}

pub fn hart_count() -> usize {
    let info = DEVICE_TREE_INFO.get().expect("Device Tree Not Initialzed");
    info.hart_count
}

fn parse_u64(data: &[u8]) -> u64 {
    let mut res = 0 as u64;
    for &b in data {
        res = (res << 8) | (b as u64);
    }
    res
}

fn parse_uart(fdt: &Fdt) -> Option<UartConfig> {
    let chosen = fdt.find_node("/chosen")?;
    let stdout_path = chosen.property("stdout-path")?.as_str()?;
    let node_path = stdout_path.split(':').next().unwrap_or(stdout_path);
    let node = fdt.find_node(node_path)?;

    UartConfig::from_fdt(&node)
}

fn parse_hart_count(fdt: &Fdt) -> usize {
    match fdt.find_node("/cpus") {
        None => 1,
        Some(cpus_node) => {
            let mut count = 0;
            for cpu_node in cpus_node.children() {
                if cpu_node.name.starts_with("cpu@") {
                    count += 1;
                }
            }
            count
        }
    }
}

fn parse_plic(fdt: &Fdt) -> Option<MemoryRange> {
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
                return Some(MemoryRange {
                    start: PhysAddr::from(region.starting_address as usize),
                    size: region.size.unwrap_or(0),
                });
            }
        }
    }
    None
}

fn parse_initrd(fdt: &Fdt) -> Option<MemoryRange> {
    let chosen = fdt.find_node("/chosen")?;
    let initrd_start = parse_u64(chosen.property("linux,initrd-start")?.value) as usize;
    let initrd_end = parse_u64(chosen.property("linux,initrd-end")?.value) as usize;
    if initrd_end > initrd_start {
        Some(MemoryRange { start: PhysAddr::from(initrd_start), size: initrd_end - initrd_start })
    } else {
        None
    }
}

static mut BOOTARGS_BUF: [u8; 256] = [0; 256];

fn parse_bootargs(fdt: &Fdt) -> Option<&'static str> {
    let chosen = fdt.find_node("/chosen")?;
    let s = chosen.property("bootargs")?.as_str()?;

    let bytes = s.as_bytes();
    let len = core::cmp::min(bytes.len(), 255);
    unsafe {
        BOOTARGS_BUF[..len].copy_from_slice(&bytes[..len]);
        BOOTARGS_BUF[len] = 0; // Null terminator for safety if needed
        Some(core::str::from_utf8_unchecked(&BOOTARGS_BUF[..len]))
    }
}

fn parse_mmio(fdt: &Fdt) -> ([MemoryRange; MAX_MMIO_REGIONS], usize) {
    let mut regions = [MemoryRange::empty(); MAX_MMIO_REGIONS];
    let mut count = 0;

    for node in fdt.all_nodes() {
        if let Some(device_type) = node.property("device_type").and_then(|p| p.as_str()) {
            if device_type == "memory" || device_type == "cpu" {
                continue;
            }
        }

        if node.name == "chosen" || node.name == "aliases" {
            continue;
        }

        if let Some(mut regs) = node.reg() {
            while let Some(region) = regs.next() {
                if count < MAX_MMIO_REGIONS {
                    let start = region.starting_address as usize;
                    let size = region.size.unwrap_or(0);
                    if size > 0 {
                        regions[count] = MemoryRange { start: PhysAddr::from(start), size };
                        count += 1;
                    }
                }
            }
        }
    }
    (regions, count)
}

pub fn get_platform_info() -> PlatformInfo {
    let mut info = PlatformInfo::new();
    let dtb = dtb_addr() as *const u8;
    if let Ok(fdt) = unsafe { Fdt::from_ptr(dtb) } {
        fill_platform_info(&fdt, &mut info);
    }
    info
}

fn fill_platform_info(fdt: &Fdt, info: &mut PlatformInfo) {
    // Fill model name
    if let Some(root) = fdt.find_node("/") {
        if let Some(model) = root.property("model").and_then(|p| p.as_str()) {
            let bytes = model.as_bytes();
            let len = core::cmp::min(bytes.len(), 63);
            info.model_name[..len].copy_from_slice(&bytes[..len]);
        }
    }

    info.cpu_count = parse_hart_count(fdt);
    let initrd = parse_initrd(fdt).expect("Initrd range not found");
    info.initrd =
        MemoryRegion { start: initrd.start, size: initrd.size, region_type: MemoryType::Ram };

    // Fill memory
    let memory = fdt.memory();
    for region in memory.regions() {
        info.add_memory(
            PhysAddr::from(region.starting_address as usize),
            region.size.unwrap_or(0),
            MemoryType::Ram,
        );
    }

    // Fill bootargs
    if let Some(args) = parse_bootargs(fdt) {
        let bytes = args.as_bytes();
        let len = core::cmp::min(bytes.len(), 255);
        info.bootargs[..len].copy_from_slice(&bytes[..len]);
    }

    // Fill MMIO regions
    let (mmio_regions, mmio_count) = parse_mmio(fdt);
    for i in 0..mmio_count {
        let region = mmio_regions[i];
        info.add_memory(region.start, region.size, MemoryType::Mmio);
    }

    // Walk for devices
    if let Some(root) = fdt.find_node("/") {
        walk_device_tree(&root, u32::MAX, info);
    }
}

fn walk_device_tree(node: &FdtNode, parent_idx: u32, info: &mut PlatformInfo) {
    for child in node.children() {
        let mut dev_idx = parent_idx;
        let is_memory = child
            .property("device_type")
            .and_then(|p| p.as_str())
            .map(|s| s == "memory")
            .unwrap_or(false);
        let is_cpu = child
            .property("device_type")
            .and_then(|p| p.as_str())
            .map(|s| s == "cpu")
            .unwrap_or(false);

        if !is_memory && !is_cpu {
            if let Some(mut regs) = child.reg() {
                if let Some(region) = regs.next() {
                    let mut desc = DeviceDesc {
                        compatible: [0; 64],
                        base_addr: PhysAddr::from(region.starting_address as usize),
                        size: region.size.unwrap_or(0),
                        irq: 0,
                        kind: DeviceKind::Unknown,
                        parent_index: parent_idx,
                        bus_type: BusType::System,
                    };

                    if let Some(compat) = child.compatible() {
                        if let Some(first) = compat.all().next() {
                            let bytes = first.as_bytes();
                            let len = core::cmp::min(bytes.len(), 63);
                            desc.compatible[..len].copy_from_slice(&bytes[..len]);

                            if first.contains("uart") || first.contains("serial") {
                                desc.kind = DeviceKind::Uart;
                            } else if first.contains("plic") {
                                desc.kind = DeviceKind::Intc;
                            } else if first.contains("virtio") {
                                desc.kind = DeviceKind::Virtio;
                            }
                        }
                    }
                    dev_idx = info.add_device(desc);
                }
            }
        }
        walk_device_tree(&child, dev_idx, info);
    }
}
