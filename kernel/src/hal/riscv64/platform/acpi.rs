use crate::drivers as generic_drivers;
use crate::hal::riscv64::drivers::{FB, FbDriver, INTC, IntcDriver, UART, UartDriver};
use crate::platform::acpi::GlendaAcpiHandler;
use acpi::AcpiTables;
use acpi::sdt::spcr::SpcrInterfaceType;

pub fn parse(tables: &AcpiTables<GlendaAcpiHandler>) {
    // 0. Initialize platform info
    // Default RISC-V frequency is often 10MHz in QEMU.
    crate::hal::riscv64::timer::init(10_000_000);

    // 1. Parse SPCR for UART
    // Serial Port Console Redirection Table
    if let Some(spcr_mapping) = tables.find_table::<acpi::sdt::spcr::Spcr>() {
        let spcr = spcr_mapping.get();
        if let Some(Ok(gas)) = spcr.base_address() {
            let addr = gas.address as usize;

            match spcr.interface_type() {
                SpcrInterfaceType::Full16550 => {
                    let size = 0x100;
                    let cfg = generic_drivers::uart::ns16550a::Config::new(addr, 0, 5, 0x20);
                    UART.call_once(|| {
                        UartDriver::Ns16550a(generic_drivers::uart::ns16550a::Uart::from_config(
                            cfg, size,
                        ))
                    });
                    log!("hal: ns16550a (acpi) initialized at {:#x}", addr);
                }
                SpcrInterfaceType::ArmPL011 => {
                    let size = 0x1000;
                    UART.call_once(|| {
                        UartDriver::Pl011(generic_drivers::uart::pl011::Pl011::new(addr, size))
                    });
                    log!("hal: pl011 (acpi) initialized at {:#x}", addr);
                }
                _ => {}
            }
        }
    }

    // 2. Parse MADT for Interrupt Controller
    // Multiple APIC Description Table
    if let Some(madt_mapping) = tables.find_table::<acpi::sdt::madt::Madt>() {
        let madt = madt_mapping.get();
        log!("hal: madt found at {:#x}", madt_mapping.physical_start);

        let madt_ptr = (&*madt) as *const acpi::sdt::madt::Madt as *const u8;
        let mut offset = core::mem::size_of::<acpi::sdt::madt::Madt>();
        let length = madt.header.length as usize;

        while offset < length {
            unsafe {
                let entry_header = *(madt_ptr.add(offset) as *const acpi::sdt::madt::EntryHeader);
                let entry_ptr = madt_ptr.add(offset);

                match entry_header.entry_type {
                    27 => {
                        // RISC-V PLIC
                        #[repr(C, packed)]
                        struct RiscvPlic {
                            header: acpi::sdt::madt::EntryHeader,
                            version: u8,
                            id: u8,
                            hardware_id: u64,
                            total_irq: u16,
                            max_priority: u16,
                            flags: u32,
                            size: u32,
                            base_addr: u64,
                            gsi_base: u32,
                        }
                        let plic = &*(entry_ptr as *const RiscvPlic);
                        let base_addr = plic.base_addr as usize;
                        let size = plic.size as usize;

                        INTC.call_once(|| {
                            IntcDriver::Plic(generic_drivers::intr::plic::Plic::new(
                                base_addr, size,
                            ))
                        });
                        log!("hal: plic (acpi) initialized at {:#x}, size: {:#x}", base_addr, size);
                    }
                    _ => {}
                }
                offset += entry_header.length as usize;
            }
        }
    }

    if let Some(info) = crate::boot::get_framebuffer() {
        let writer = crate::drivers::fb::FramebufferWriter::new(info);
        writer.clear();
        FB.call_once(|| FbDriver::Framebuffer(writer));
        log!("hal: framebuffer initialized at {:#x}", info.address.as_usize());
    }
}
