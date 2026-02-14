use crate::mem::PhysAddr;
use crate::platform::{BusType, DeviceDesc, DeviceKind, MemoryType, PlatformInfo};
use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use core::ptr::NonNull;

#[derive(Clone, Copy)]
pub struct GlendaAcpiHandler;

impl AcpiHandler for GlendaAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new_unchecked(physical_address as *mut T),
                size,
                size,
                *self,
            )
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        // Nothing to do for identity mapping
    }
}

pub fn parse(rsdp_addr: usize) -> PlatformInfo {
    let mut info = PlatformInfo::new();
    let handler = GlendaAcpiHandler;

    let tables =
        unsafe { AcpiTables::from_rsdp(handler, rsdp_addr).expect("Failed to parse ACPI tables") };

    // Parse SPCR for UART
    if let Ok(spcr) = tables.find_table::<acpi::spcr::Spcr>() {
        if let Some(Ok(gas)) = spcr.base_address() {
            let base = gas.address as usize;

            let mut dev = DeviceDesc {
                compatible: [0; 64],
                base_addr: PhysAddr::from(base),
                size: 0x1000,
                irq: 0,
                kind: DeviceKind::Uart,
                parent_index: u32::MAX,
                bus_type: BusType::Platform,
            };
            dev.compatible[..4].copy_from_slice(b"uart");
            info.add_device(dev);
            // SPCR usually points to MMIO
            info.add_memory(PhysAddr::from(base), 0x1000, MemoryType::Mmio);
        }
    }

    // Parse MADT for CPUs
    if let Ok(madt_mapping) = tables.find_table::<acpi::madt::Madt>() {
        let madt_ptr = madt_mapping.virtual_start().as_ptr();
        let madt_len = unsafe { (*madt_ptr).header.length } as usize;
        let madt_ptr_u8 = madt_ptr as *const u8;
        let mut offset = core::mem::size_of::<acpi::madt::Madt>();
        let mut cpu_count = 0;

        while offset < madt_len {
            unsafe {
                let entry_ptr = madt_ptr_u8.add(offset);
                let entry_type = *entry_ptr;
                let entry_len = *entry_ptr.add(1);

                if entry_len == 0 || offset + (entry_len as usize) > madt_len {
                    break;
                }

                // 0x00 = Processor Local APIC (x86)
                if entry_type == 0x00 {
                    let flags = entry_ptr.add(4) as *const u32;
                    if (flags.read_unaligned() & 1) != 0 {
                        cpu_count += 1;
                    }
                }
                // 0x18 = RISC-V Interrupt Controller (Hart)
                else if entry_type == 0x18 {
                    // Flags at offset 16
                    let flags = entry_ptr.add(16) as *const u32;
                    if (flags.read_unaligned() & 1) != 0 {
                        cpu_count += 1;
                    }
                }
                // 0x1B = RISC-V PLIC
                else if entry_type == 0x1B {
                    if entry_len >= 36 {
                        // Address at offset 24 (u64)
                        let base_addr = (entry_ptr.add(24) as *const u64).read_unaligned() as usize;
                        // Size at offset 20 (u32)
                        let size = (entry_ptr.add(20) as *const u32).read_unaligned() as usize;
                        // Sources at offset 12 (u16)
                        let sources = (entry_ptr.add(12) as *const u16).read_unaligned() as usize;
                        info.irq_count = sources;

                        let mut dev = DeviceDesc {
                            compatible: [0; 64],
                            base_addr: PhysAddr::from(base_addr),
                            size,
                            irq: 0,
                            kind: DeviceKind::Intc,
                            parent_index: u32::MAX,
                            bus_type: BusType::Platform,
                        };
                        dev.compatible[..4].copy_from_slice(b"plic");
                        info.add_device(dev);
                        info.add_memory(PhysAddr::from(base_addr), size, MemoryType::Mmio);
                    }
                }

                offset += entry_len as usize;
            }
        }

        info.cpu_count = if cpu_count > 0 { cpu_count } else { 1 };
    }

    crate::hal::platform::parse_acpi(&tables, &mut info);

    info
}
