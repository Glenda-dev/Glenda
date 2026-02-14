use crate::drivers as generic_drivers;
use crate::hal::riscv64::drivers::{INTC, IntcDriver, UART, UartDriver};
use crate::platform::PlatformInfo;
use crate::platform::acpi::GlendaAcpiHandler;
use acpi::AcpiTables;
use acpi::spcr::SpcrInterfaceType;

pub fn parse(tables: &AcpiTables<GlendaAcpiHandler>, _info: &mut PlatformInfo) {
    // 1. Parse SPCR for UART
    // Serial Port Console Redirection Table
    if let Ok(spcr) = tables.find_table::<acpi::spcr::Spcr>() {
        // spcr.base_address() returns generic Address Structure (GAS)
        // Accessing it might return an invalid address error
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
    // Multiple APIC Description Table (checks for RISC-V extensions)
    if let Ok(madt_mapping) = tables.find_table::<acpi::madt::Madt>() {
        let madt_ptr = madt_mapping.virtual_start().as_ptr();
        let madt_len = unsafe { (*madt_ptr).header.length } as usize;
        let madt_ptr_u8 = madt_ptr as *const u8;
        let mut offset = core::mem::size_of::<acpi::madt::Madt>();

        // Log the MADT header info
        log!("hal: madt found at {:#x}, length: {}", madt_mapping.physical_start(), madt_len);

        while offset < madt_len {
            unsafe {
                let entry_ptr = madt_ptr_u8.add(offset);
                let entry_type = *entry_ptr;
                let entry_len = *entry_ptr.add(1);

                if entry_len == 0 {
                    log!("hal: madt parse error: zero length entry at offset {}", offset);
                    break;
                }

                // Check if we will overrun
                if offset + (entry_len as usize) > madt_len {
                    log!("hal: madt parse error: entry length overrun");
                    break;
                }

                // 0x1B (27) = RISC-V PLIC
                if entry_type == 0x1B {
                    // Parse PLIC
                    // Layout (Based on RISC-V ACPI Platform Interface):
                    // 0: Type (1)
                    // 1: Length (1)
                    // 2: Version (1)
                    // 3: Instance ID (1)
                    // 4: Hardware ID (8)
                    // 12: Total External Interrupt Sources (2)
                    // 14: Max Priority (2)
                    // 16: Flags (4)
                    // 20: PLIC Size (4)
                    // 24: PLIC Address (8)
                    // 32: GSI Base (4)

                    if entry_len >= 36 {
                        // Address is at offset 24
                        let base_addr_ptr = entry_ptr.add(24) as *const u64;
                        let base_addr = base_addr_ptr.read_unaligned() as usize;

                        // Size is at offset 20
                        let size_ptr = entry_ptr.add(20) as *const u32;
                        let size = size_ptr.read_unaligned() as usize;

                        INTC.call_once(|| {
                            IntcDriver::Plic(generic_drivers::intr::plic::Plic::new(
                                base_addr, size,
                            ))
                        });
                        log!("hal: plic (acpi) initialized at {:#x}, size: {:#x}", base_addr, size);
                    } else {
                        log!("hal: madt plic entry too short: {}", entry_len);
                    }
                }
                // 0x1A (26) = RISC-V APLIC
                else if entry_type == 0x1A {
                    // Parse APLIC
                    // Layout can be complex, assuming basic:
                    // ...
                    // 16: Base Address (8)
                    // 24: Length (4) ?

                    // Currently implemented PLIC support is priority.
                    // APLIC logic would go here.
                    log!("hal: madt aplic entry found (unimplemented)");
                }

                offset += entry_len as usize;
            }
        }
    }
}
