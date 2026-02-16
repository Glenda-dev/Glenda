use crate::drivers as generic_drivers;
use crate::hal::riscv64::drivers::{INTC, IntcDriver, UART, UartDriver};

pub fn parse(fdt: &fdt::Fdt) {
    log!("hal: Parsing fdt...");
    let timer_frequency = fdt
        .find_node("/cpus")
        .and_then(|node| node.property("timebase-frequency"))
        .and_then(|prop| {
            if prop.value.len() == 4 {
                Some(u32::from_be_bytes(prop.value.try_into().unwrap()) as usize)
            } else if prop.value.len() == 8 {
                Some(u64::from_be_bytes(prop.value.try_into().unwrap()) as usize)
            } else {
                None
            }
        })
        .unwrap_or(10_000_000);

    crate::hal::timer::set_frequency(timer_frequency);

    for node in fdt.all_nodes() {
        if let Some(compatibles) = node.compatible() {
            for compat in compatibles.all() {
                if compat == "riscv,plic0" || compat == "sifive,plic-1.0.0" {
                    log!("hal: Found PLIC at {}", node.name);
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x400_0000);

                        INTC.call_once(|| {
                            IntcDriver::Plic(generic_drivers::intr::plic::Plic::new(addr, size))
                        });
                        log!("hal: PLIC initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "riscv,aplic" {
                    log!("hal: Found APLIC at {}", node.name);
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x4000);

                        INTC.call_once(|| {
                            IntcDriver::Aplic(generic_drivers::intr::aplic::Aplic::new(addr, size))
                        });
                        log!("hal: APLIC initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "ns16550a" || compat == "snps,dw-apb-uart" {
                    log!("hal: Found NS16550A-compatible UART at {}", node.name);
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x100);

                        let cfg = generic_drivers::uart::ns16550a::Config::new(addr, 0, 5, 0x20);
                        UART.call_once(|| {
                            UartDriver::Ns16550a(
                                generic_drivers::uart::ns16550a::Uart::from_config(cfg, size),
                            )
                        });
                        log!("hal: NS16550A initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "arm,pl011" || compat == "arm,primecell" {
                    log!("hal: Found PL011-compatible UART at {}", node.name);
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x1000);

                        UART.call_once(|| {
                            UartDriver::Pl011(generic_drivers::uart::pl011::Pl011::new(addr, size))
                        });
                        log!("hal: PL011 initialized at {:#x}", addr);
                    }
                    break;
                }
            }
        }
    }
}
