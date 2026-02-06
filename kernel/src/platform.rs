use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::mem::PhysAddr;
use core::fmt;
use core::mem::size_of;
use spin::Once;

static PLATFORM_INFO: Once<PlatformInfo> = Once::new();
pub const PLATFORM_SIZE: usize = size_of::<PlatformInfo>();
pub const PLATFORM_PAGES: usize = (PLATFORM_SIZE + PGSIZE - 1) / PGSIZE;
pub const MAX_MEMORY_REGIONS: usize = 16;
pub const MAX_DEVICES: usize = 64;

/// 平台硬件信息摘要
/// 这个结构体设计为架构无关，可以从 DTB (RISC-V/ARM) 或 ACPI (x86) 转换而来
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PlatformInfo {
    /// 平台名称 (例如 "QEMU Virt", "SiFive Unleashed")
    pub model_name: [u8; 64],

    /// 平台架构 (例如 "riscv64", "x86_64")
    pub arch: [u8; 16],

    /// 引导参数字符串
    pub bootargs: [u8; 256],

    /// initrd 物理内存区域
    pub initrd: MemoryRegion,

    /// CPU 核心数量
    pub cpu_count: usize,

    /// 时钟频率
    pub clock_freq: usize,

    /// 平台支持的最大 IRQ 数量
    pub irq_count: usize,

    /// 物理内存区域列表
    pub memory_regions: [MemoryRegion; MAX_MEMORY_REGIONS],
    pub memory_region_count: usize,

    /// 关键设备列表 (中断控制器, 串口等)
    pub devices: [DeviceDesc; MAX_DEVICES],
    pub device_count: usize,
}

impl fmt::Debug for PlatformInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PlatformInfo {{")?;
        writeln!(f, "  model_name: {:?},", self.model())?;
        writeln!(f, "  cpu_count: {},", self.cpu_count)?;
        writeln!(f, "  clock_freq: {},", self.clock_freq)?;
        writeln!(f, "  memory_regions: [")?;
        for i in 0..self.memory_region_count {
            writeln!(f, "    {:?},", self.memory_regions[i])?;
        }
        writeln!(f, "  ],")?;
        writeln!(f, "  devices: [")?;
        for i in 0..self.device_count {
            writeln!(f, "    {:?},", self.devices[i])?;
        }
        writeln!(f, "  ],")?;
        writeln!(f, "}}")
    }
}

/// 描述一块物理内存区域
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: PhysAddr,
    pub size: usize,
    pub region_type: MemoryType,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Ram = 1,
    Mmio = 2,
    Reserved = 3,
}

/// 描述一个硬件设备资源
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DeviceDesc {
    /// 设备兼容性字符串 (例如 "ns16550a", "virtio,mmio")
    /// 用于驱动匹配
    pub compatible: [u8; 64],

    /// MMIO 物理基地址
    pub base_addr: PhysAddr,

    /// MMIO 区域大小
    pub size: usize,

    /// 中断号 (全局中断号)
    pub irq: u32,

    /// 设备类型提示
    pub kind: DeviceKind,

    /// 拓扑结构支持：父节点在 `devices` 数组中的索引
    /// 如果是根节点，可以使用 u32::MAX (0xFFFFFFFF)
    pub parent_index: u32,

    /// 拓扑结构支持：该设备连接到的总线类型
    pub bus_type: BusType,
}

impl fmt::Debug for DeviceDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let compatible_len =
            self.compatible.iter().position(|&c| c == 0).unwrap_or(self.compatible.len());
        let compatible_str =
            core::str::from_utf8(&self.compatible[..compatible_len]).unwrap_or("???");

        f.debug_struct("DeviceDesc")
            .field("compatible", &compatible_str)
            .field("base_addr", &format_args!("{}", self.base_addr))
            .field("size", &format_args!("{:#x}", self.size))
            .field("irq", &self.irq)
            .field("kind", &self.kind)
            .field("parent", &mut DisplayOptionIdx(self.parent_index))
            .field("bus", &self.bus_type)
            .finish()
    }
}

struct DisplayOptionIdx(u32);
impl fmt::Debug for DisplayOptionIdx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == u32::MAX { write!(f, "None") } else { write!(f, "{}", self.0) }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Unknown = 0,
    Uart = 1,
    Intc = 2, // 中断控制器 (PLIC/GIC)
    Timer = 3,
    Virtio = 4,
    PciHost = 5,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    System = 0, // 系统主总线 (System Bus)
    Pci = 1,
    Usb = 2,
    Platform = 3, // 简单的内存映射设备
}

impl PlatformInfo {
    pub fn new() -> Self {
        Self {
            model_name: [0; 64],
            cpu_count: 0,
            clock_freq: 0,
            arch: [0; 16],
            bootargs: [0; 256],
            initrd: MemoryRegion {
                start: PhysAddr::null(),
                size: 0,
                region_type: MemoryType::Reserved,
            },
            irq_count: 0,
            memory_regions: [MemoryRegion {
                start: PhysAddr::null(),
                size: 0,
                region_type: MemoryType::Reserved,
            }; MAX_MEMORY_REGIONS],
            memory_region_count: 0,
            devices: [DeviceDesc {
                compatible: [0; 64],
                base_addr: PhysAddr::null(),
                size: 0,
                irq: 0,
                kind: DeviceKind::Unknown,
                parent_index: u32::MAX,
                bus_type: BusType::System,
            }; MAX_DEVICES],
            device_count: 0,
        }
    }

    /// 获取平台型号字符串
    pub fn model(&self) -> &str {
        let len = self.model_name.iter().position(|&c| c == 0).unwrap_or(self.model_name.len());
        core::str::from_utf8(&self.model_name[..len]).unwrap_or("Unknown")
    }

    pub fn add_memory(&mut self, start: PhysAddr, size: usize, memtype: MemoryType) {
        if self.memory_region_count < self.memory_regions.len() {
            self.memory_regions[self.memory_region_count] =
                MemoryRegion { start, size, region_type: memtype };
            self.memory_region_count += 1;
        }
    }

    pub fn add_device(&mut self, dev: DeviceDesc) -> u32 {
        if self.device_count < self.devices.len() {
            let idx = self.device_count;
            self.devices[idx] = dev;
            self.device_count += 1;
            idx as u32
        } else {
            u32::MAX
        }
    }
}

pub fn init() {
    PLATFORM_INFO.call_once(|| hal::platform::info());
    log!("platform: Initialized via HAL");
}

pub fn get() -> &'static PlatformInfo {
    PLATFORM_INFO.get().expect("PlatformInfo is not initialized")
}
