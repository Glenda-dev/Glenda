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
    plic: Option<MemoryRange>,
    initrd: Option<MemoryRange>,
    bootargs: Option<&'static str>,
    pub dtb_paddr: usize,
    pub dtb_size: usize,
}

impl DeviceTreeInfo {
    fn new(fdt: &Fdt, dtb_paddr: usize) -> Self {
        let hart_count = parse_hart_count(fdt);
        let uart = parse_uart(fdt);
        let memory = parse_memory(fdt);
        let plic = parse_plic(fdt);
        let initrd = parse_initrd(fdt);
        let bootargs = parse_bootargs(fdt);
        let dtb_size = fdt.total_size();
        Self { uart, hart_count, memory, plic, dtb_paddr, dtb_size, initrd, bootargs }
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

    fn plic(&self) -> Option<MemoryRange> {
        self.plic
    }

    fn initrd(&self) -> Option<MemoryRange> {
        self.initrd
    }

    fn bootargs(&self) -> Option<&'static str> {
        self.bootargs
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

pub fn plic() -> Option<MemoryRange> {
    DEVICE_TREE.get().and_then(DeviceTreeInfo::plic)
}

pub fn initrd_range() -> Option<MemoryRange> {
    DEVICE_TREE.get().and_then(DeviceTreeInfo::initrd)
}

pub fn bootargs() -> Option<&'static str> {
    DEVICE_TREE.get().and_then(|info| info.bootargs)
}

fn parse_u64(data: &[u8]) -> u64 {
    let mut res = 0;
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
