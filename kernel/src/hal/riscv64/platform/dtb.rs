use crate::drivers as generic_drivers;
use crate::hal::riscv64::drivers::{FB, FbDriver, INTC, IntcDriver, UART, UartDriver};

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

    crate::hal::riscv64::timer::init(timer_frequency);

    let preferred_uart_addr = {
        let mut addr = None;

        if let Some(chosen) = fdt.find_node("/chosen")
            && let Some(raw_stdout) = chosen.property("stdout-path").and_then(|p| p.as_str())
        {
            let raw_stdout = raw_stdout.trim_end_matches('\0');
            let mut stdout_path = raw_stdout.split(':').next().unwrap_or(raw_stdout);

            // stdout-path may be an alias name like "serial0" instead of an absolute path.
            if !stdout_path.starts_with('/')
                && let Some(aliases) = fdt.find_node("/aliases")
                && let Some(alias_target) = aliases.property(stdout_path).and_then(|p| p.as_str())
            {
                let alias_target = alias_target.trim_end_matches('\0');
                stdout_path = alias_target.split(':').next().unwrap_or(alias_target);
            }

            if let Some(stdout_node) = fdt.find_node(stdout_path)
                && let Some(reg) = stdout_node.reg().and_then(|mut r| r.next())
            {
                addr = Some(reg.starting_address as usize);
                log!(
                    "hal: /chosen/stdout-path resolved to {} @ {:#x}",
                    stdout_path,
                    reg.starting_address as usize
                );
            }
        }

        addr
    };

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

                        if let Some(preferred) = preferred_uart_addr
                            && preferred != addr
                        {
                            log!(
                                "hal: Skip UART {:#x} (stdout-path prefers {:#x})",
                                addr,
                                preferred
                            );
                            break;
                        }

                        if UART.get().is_some() {
                            break;
                        }

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

                        if let Some(preferred) = preferred_uart_addr
                            && preferred != addr
                        {
                            log!(
                                "hal: Skip UART {:#x} (stdout-path prefers {:#x})",
                                addr,
                                preferred
                            );
                            break;
                        }

                        if UART.get().is_some() {
                            break;
                        }

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

    if let Some(info) = crate::boot::get_framebuffer() {
        let writer = crate::drivers::fb::FramebufferWriter::new(info);
        writer.clear();
        FB.call_once(|| FbDriver::Framebuffer(writer));
        log!("hal: framebuffer initialized at {:#x}", info.address.as_usize());
    }
}
