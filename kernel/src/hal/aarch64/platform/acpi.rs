use crate::drivers as generic_drivers;
use crate::hal::aarch64::drivers::{FB, FbDriver, INTC, IntcDriver, UART, UartDriver};
use crate::platform::acpi::GlendaAcpiHandler;
use acpi::AcpiTables;
use acpi::sdt::spcr::SpcrInterfaceType;

pub fn parse_acpi(tables: &AcpiTables<GlendaAcpiHandler>) {
    crate::hal::aarch64::timer::init(0);

    if let Some(spcr_mapping) = tables.find_table::<acpi::sdt::spcr::Spcr>() {
        let spcr = spcr_mapping.get();
        if let Some(Ok(gas)) = spcr.base_address() {
            let addr = gas.address as usize;
            match spcr.interface_type() {
                SpcrInterfaceType::Full16550 => {
                    let cfg = generic_drivers::uart::ns16550a::Config::new(addr, 0, 5, 0x20);
                    UART.call_once(|| {
                        UartDriver::Ns16550a(generic_drivers::uart::ns16550a::Ns16550a::from_config(
                            cfg, 0x100,
                        ))
                    });
                }
                SpcrInterfaceType::ArmPL011 => {
                    UART.call_once(|| {
                        UartDriver::Pl011(generic_drivers::uart::pl011::Pl011::new(addr, 0x1000))
                    });
                }
                _ => {}
            }
        }
    }

    if let Some(madt_mapping) = tables.find_table::<acpi::sdt::madt::Madt>() {
        let madt = madt_mapping.get();
        let madt_ptr = (&*madt) as *const acpi::sdt::madt::Madt as *const u8;
        let mut offset = core::mem::size_of::<acpi::sdt::madt::Madt>();
        let length = madt.header.length as usize;

        let mut gicd_base = None;
        let mut gicc_base = None;

        while offset < length {
            unsafe {
                let entry_header = *(madt_ptr.add(offset) as *const acpi::sdt::madt::EntryHeader);
                let entry_ptr = madt_ptr.add(offset);
                match entry_header.entry_type {
                    // GIC CPU interface
                    11 => {
                        #[repr(C, packed)]
                        struct Gicc {
                            _h: acpi::sdt::madt::EntryHeader,
                            _cpu_interface_number: u16,
                            _acpi_processor_uid: u16,
                            _flags: u32,
                            _parking_version: u32,
                            _performance_gsiv: u32,
                            parked_address: u64,
                            phys_base_addr: u64,
                            _gicv: u64,
                            _gich: u64,
                            _vgic_maint_irq: u32,
                            _gicr_base_address: u64,
                            _mpidr: u64,
                            _efficiency_class: u8,
                            _reserved: [u8; 3],
                            _spe_overflow_interrupt: u16,
                            _trbe_interrupt: u16,
                        }
                        let gicc = &*(entry_ptr as *const Gicc);
                        let cpu_base = if gicc.phys_base_addr != 0 {
                            gicc.phys_base_addr as usize
                        } else {
                            gicc.parked_address as usize
                        };
                        if cpu_base != 0 && gicc_base.is_none() {
                            gicc_base = Some(cpu_base);
                        }
                    }
                    // GIC distributor
                    12 => {
                        #[repr(C, packed)]
                        struct Gicd {
                            _h: acpi::sdt::madt::EntryHeader,
                            _gic_id: u16,
                            phys_base_addr: u64,
                            _global_irq_base: u32,
                            _version: u8,
                            _reserved: [u8; 3],
                        }
                        let gicd = &*(entry_ptr as *const Gicd);
                        if gicd_base.is_none() {
                            gicd_base = Some(gicd.phys_base_addr as usize);
                        }
                    }
                    _ => {}
                }
                offset += entry_header.length as usize;
            }
        }

        if let (Some(dist), Some(cpu)) = (gicd_base, gicc_base) {
            INTC.call_once(|| IntcDriver::GicV2(generic_drivers::intr::gicv2::GicV2::new(dist, cpu, 0x10000, 0x2000)));
            log!("hal: gicv2 (acpi) initialized, dist={:#x}, cpu={:#x}", dist, cpu);
        }
    }

    if let Some(info) = crate::boot::get_framebuffer() {
        let writer = crate::drivers::fb::FramebufferWriter::new(info);
        writer.clear();
        FB.call_once(|| FbDriver::Framebuffer(writer));
    }
}
