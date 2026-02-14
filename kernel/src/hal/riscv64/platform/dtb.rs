use crate::drivers as generic_drivers;
use crate::hal::riscv64::drivers::{INTC, IntcDriver, UART, UartDriver};
use crate::platform::PlatformInfo;

pub fn parse(fdt: &fdt::Fdt, _info: &mut PlatformInfo) {
    for node in fdt.all_nodes() {
        if let Some(compatibles) = node.compatible() {
            for compat in compatibles.all() {
                if compat == "riscv,plic0" || compat == "sifive,plic-1.0.0" {
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x400_0000);

                        INTC.call_once(|| {
                            IntcDriver::Plic(generic_drivers::intr::plic::Plic::new(addr, size))
                        });
                        log!("hal: plic initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "riscv,aplic" {
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x4000);

                        INTC.call_once(|| {
                            IntcDriver::Aplic(generic_drivers::intr::aplic::Aplic::new(addr, size))
                        });
                        log!("hal: aplic initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "ns16550a" || compat == "snps,dw-apb-uart" {
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x100);

                        let cfg = generic_drivers::uart::ns16550a::Config::new(addr, 0, 5, 0x20);
                        UART.call_once(|| {
                            UartDriver::Ns16550a(
                                generic_drivers::uart::ns16550a::Uart::from_config(cfg, size),
                            )
                        });
                        log!("hal: ns16550a initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "arm,pl011" || compat == "arm,primecell" {
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x1000);

                        UART.call_once(|| {
                            UartDriver::Pl011(generic_drivers::uart::pl011::Pl011::new(addr, size))
                        });
                        log!("hal: pl011 initialized at {:#x}", addr);
                    }
                    break;
                }
            }
        }
    }
}
