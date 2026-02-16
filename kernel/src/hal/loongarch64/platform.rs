use crate::platform::{PlatformInfo, MemoryRegion, MemoryType, DeviceDesc, DeviceKind, BusType};
use crate::mem::PhysAddr;

pub fn info() -> PlatformInfo {
    let mut info = PlatformInfo::new();
    info.cpu_count = 1;

    info.memory_regions[0] = MemoryRegion {
        start: PhysAddr::from(0),
        size: 0x10000000,
        region_type: MemoryType::Ram,
    };
    info.memory_region_count = 1;

    info.initrd = MemoryRegion {
        start: PhysAddr::from(0x04000000),
        size: 176 * 1024,
        region_type: MemoryType::Reserved,
    };

    info
}

pub fn bootstrap_cpus() {}
pub fn send_ipi(_mask: usize, _base: usize) -> Result<(), crate::error::Error> {
    Ok(())
}
pub fn shutdown() -> ! {
    loop {
        unsafe { core::arch::asm!("idle 0"); }
    }
}
