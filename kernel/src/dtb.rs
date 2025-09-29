#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::cmp;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicU8, Ordering};

use driver_uart::Config as UartConfig;
use fdt::Fdt;

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
}

impl DeviceTreeInfo {
    fn new(fdt: &Fdt) -> Self {
        let hart_count = parse_hart_count(fdt);
        let uart = parse_uart(fdt);
        let memory = parse_memory(fdt);

        Self { uart, hart_count, memory }
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

pub fn init(dtb: *const u8) -> Result<&'static DeviceTreeInfo, fdt::FdtError> {
    DEVICE_TREE
        .get_or_try_init(|| unsafe { Fdt::from_ptr(dtb) }.map(|fdt| DeviceTreeInfo::new(&fdt)))
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
        region.size.map(|size| MemoryRange { start, size })
    })
}
