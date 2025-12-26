use crate::mem::PhysAddr;
use crate::printk;
use crate::printk::uart::Config as UartConfig;
use core::cell::UnsafeCell;
use core::cmp;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicU8, Ordering};
use fdt::Fdt;

#[derive(Debug, Clone, Copy)]
pub struct MemoryRange {
    pub start: PhysAddr,
    pub size: usize,
}

impl MemoryRange {
    pub fn end(&self) -> PhysAddr {
        self.start + self.size
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceTreeInfo {
    uart: Option<UartConfig>,
    hart_count: usize,
    memory: Option<MemoryRange>,
    plic_base: Option<usize>,
    pub dtb_paddr: usize,
    pub dtb_size: usize,
}

impl DeviceTreeInfo {
    fn new(fdt: &Fdt, dtb_paddr: usize) -> Self {
        let hart_count = parse_hart_count(fdt);
        let uart = parse_uart(fdt);
        let memory = parse_memory(fdt);
        let plic_base = parse_plic_base(fdt);
        let dtb_size = fdt.total_size();
        Self { uart, hart_count, memory, plic_base, dtb_paddr, dtb_size }
    }

    fn uart(&self) -> Option<UartConfig> {
        self.uart
    }

    fn hart_count(&self) -> usize {
        cmp::max(self.hart_count, 1)
    }

    fn memory(&self) -> Option<MemoryRange> {
        self.memory
    }

    fn plic_base(&self) -> Option<usize> {
        self.plic_base
    }
}

const UNINITIALIZED: u8 = 0;
const INITIALIZING: u8 = 1;
const READY: u8 = 2;

struct DeviceTreeCell {
    state: AtomicU8,
    value: UnsafeCell<Option<DeviceTreeInfo>>,
}

impl DeviceTreeCell {
    const fn new() -> Self {
        Self { state: AtomicU8::new(UNINITIALIZED), value: UnsafeCell::new(None) }
    }

    fn get(&self) -> Option<&DeviceTreeInfo> {
        if self.state.load(Ordering::Acquire) == READY {
            unsafe { (*self.value.get()).as_ref() }
        } else {
            None
        }
    }

    fn get_or_try_init<F>(&self, init: F) -> Result<&DeviceTreeInfo, fdt::FdtError>
    where
        F: FnOnce() -> Result<DeviceTreeInfo, fdt::FdtError>,
    {
        loop {
            match self.state.load(Ordering::Acquire) {
                READY => return Ok(self.get_ready()),
                UNINITIALIZED => {
                    if self
                        .state
                        .compare_exchange(
                            UNINITIALIZED,
                            INITIALIZING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        break;
                    }
                }
                _ => {
                    while self.state.load(Ordering::Acquire) == INITIALIZING {
                        spin_loop();
                    }
                }
            }
        }

        match init() {
            Ok(info) => unsafe {
                *self.value.get() = Some(info);
                self.state.store(READY, Ordering::Release);
                Ok(self.get_ready())
            },
            Err(err) => {
                self.state.store(UNINITIALIZED, Ordering::Release);
                Err(err)
            }
        }
    }

    fn get_ready(&self) -> &DeviceTreeInfo {
        unsafe { (*self.value.get()).as_ref().unwrap() }
    }
}

unsafe impl Sync for DeviceTreeCell {}

static DEVICE_TREE: DeviceTreeCell = DeviceTreeCell::new();

fn _init(dtb: *const u8) -> Result<&'static DeviceTreeInfo, fdt::FdtError> {
    DEVICE_TREE.get_or_try_init(|| {
        unsafe { Fdt::from_ptr(dtb) }.map(|fdt| DeviceTreeInfo::new(&fdt, dtb as usize))
    })
}

pub fn dtb_info() -> Option<(usize, usize)> {
    DEVICE_TREE.get().map(|info| (info.dtb_paddr, info.dtb_size))
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

pub fn soc_mmio_range() -> Option<(usize, usize)> {
    let info = DEVICE_TREE.get()?;
    let fdt = unsafe { fdt::Fdt::from_ptr(info.dtb_paddr as *const u8) }.ok()?;
    let soc = fdt.find_node("/soc")?;

    // 获取根节点的 cells 信息 (用于解析 parent-bus-address 和 length)
    let root = fdt.find_node("/")?;
    let parent_addr_cells = root.property("#address-cells")?.as_usize()?;
    let parent_size_cells = root.property("#size-cells")?.as_usize()?;

    // 获取 soc 节点的 cells 信息 (用于解析 child-bus-address)
    let child_addr_cells = soc.property("#address-cells")?.as_usize()?;

    let ranges = soc.property("ranges")?;
    let value = ranges.value;

    // ranges 格式: (child_addr, parent_addr, size)
    let entry_size = (child_addr_cells + parent_addr_cells + parent_size_cells) * 4;
    if value.len() < entry_size {
        return None;
    }

    // 简单起见，我们只取第一个 entry
    let mut offset = 0;

    // 跳过 child_addr
    offset += child_addr_cells * 4;

    // 读取 parent_addr (CPU 物理地址)
    let mut parent_addr: usize = 0;
    for _ in 0..parent_addr_cells {
        parent_addr = (parent_addr << 32)
            | (u32::from_be_bytes(value[offset..offset + 4].try_into().unwrap()) as usize);
        offset += 4;
    }

    // 读取 size
    let mut size: usize = 0;
    for _ in 0..parent_size_cells {
        size = (size << 32)
            | (u32::from_be_bytes(value[offset..offset + 4].try_into().unwrap()) as usize);
        offset += 4;
    }

    Some((parent_addr, size))
}

fn parse_uart(fdt: &Fdt) -> Option<UartConfig> {
    let chosen = fdt.find_node("/chosen")?;
    let stdout_path = chosen.property("stdout-path")?.as_str()?;
    let node_path = stdout_path.split(':').next().unwrap_or(stdout_path);
    let node = fdt.find_node(node_path)?;

    UartConfig::from_fdt(&node)
}

fn parse_hart_count(fdt: &Fdt) -> usize {
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

    cmp::max(count, 1)
}

fn parse_memory(fdt: &Fdt) -> Option<MemoryRange> {
    let memory = fdt.memory();
    let mut regions = memory.regions();
    regions.find_map(|region| {
        let start = region.starting_address as usize;
        region.size.map(|size| MemoryRange { start: PhysAddr::from(start), size })
    })
}

fn parse_plic_base(fdt: &Fdt) -> Option<usize> {
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
