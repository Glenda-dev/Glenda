use crate::drivers as generic_drivers;
use crate::hal::aarch64::drivers::{FB, FbDriver, INTC, IntcDriver, UART, UartDriver};

pub fn parse_dtb(fdt: &fdt::Fdt) {
    log!("hal: Parsing aarch64 fdt...");

    crate::hal::aarch64::timer::init(0);

    let preferred_uart_addr = {
        let mut addr = None;
        if let Some(chosen) = fdt.find_node("/chosen")
            && let Some(raw_stdout) = chosen.property("stdout-path").and_then(|p| p.as_str())
        {
            let raw_stdout = raw_stdout.trim_end_matches('\0');
            let mut stdout_path = raw_stdout.split(':').next().unwrap_or(raw_stdout);
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
            }
        }
        addr
    };

    for node in fdt.all_nodes() {
        if let Some(compatibles) = node.compatible() {
            for compat in compatibles.all() {
                if compat == "arm,gic-400" || compat == "arm,cortex-a15-gic" {
                    if INTC.get().is_some() {
                        break;
                    }
                    if let Some(mut reg) = node.reg() {
                        let dist = reg.next();
                        let cpu = reg.next();
                        if let (Some(dist), Some(cpu)) = (dist, cpu) {
                            let dist_base = dist.starting_address as usize;
                            let cpu_base = cpu.starting_address as usize;
                            let dist_size = dist.size.unwrap_or(0x10000);
                            let cpu_size = cpu.size.unwrap_or(0x2000);
                            INTC.call_once(|| {
                                IntcDriver::GicV2(generic_drivers::intr::gicv2::GicV2::new(
                                    dist_base, cpu_base, dist_size, cpu_size,
                                ))
                            });
                            log!(
                                "hal: GICv2 initialized, dist={:#x}, cpu={:#x}",
                                dist_base,
                                cpu_base
                            );
                        }
                    }
                    break;
                } else if compat == "ns16550a" || compat == "snps,dw-apb-uart" {
                    if UART.get().is_some() {
                        break;
                    }
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        if let Some(preferred) = preferred_uart_addr
                            && preferred != addr
                        {
                            break;
                        }
                        let size = reg.size.unwrap_or(0x100);
                        let cfg = generic_drivers::uart::ns16550a::Config::new(addr, 0, 5, 0x20);
                        UART.call_once(|| {
                            UartDriver::Ns16550a(
                                generic_drivers::uart::ns16550a::Ns16550a::from_config(cfg, size),
                            )
                        });
                        log!("hal: ns16550a initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "arm,pl011" || compat == "arm,primecell" {
                    if UART.get().is_some() {
                        break;
                    }
                    if let Some(reg) = node.reg().and_then(|mut r| r.next()) {
                        let addr = reg.starting_address as usize;
                        if let Some(preferred) = preferred_uart_addr
                            && preferred != addr
                        {
                            break;
                        }
                        let size = reg.size.unwrap_or(0x1000);
                        UART.call_once(|| {
                            UartDriver::Pl011(generic_drivers::uart::pl011::Pl011::new(addr, size))
                        });
                        log!("hal: pl011 initialized at {:#x}", addr);
                    }
                    break;
                } else if compat == "riscv,plic0" || compat == "sifive,plic-1.0.0" {
                    if INTC.get().is_none()
                        && let Some(reg) = node.reg().and_then(|mut r| r.next())
                    {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x400_0000);
                        INTC.call_once(|| {
                            IntcDriver::Plic(generic_drivers::intr::plic::Plic::new(addr, size))
                        });
                    }
                    break;
                } else if compat == "riscv,aplic" {
                    if INTC.get().is_none()
                        && let Some(reg) = node.reg().and_then(|mut r| r.next())
                    {
                        let addr = reg.starting_address as usize;
                        let size = reg.size.unwrap_or(0x4000);
                        INTC.call_once(|| {
                            IntcDriver::Aplic(generic_drivers::intr::aplic::Aplic::new(addr, size))
                        });
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
    }
}
