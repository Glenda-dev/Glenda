use crate::cap::CNode;
use crate::cap::Capability;
use crate::hal;
use crate::mem;
use crate::mem::{PageTable, VirtAddr};
use crate::platform;
use crate::printk;
use crate::proc;
use crate::proc::TCB;

pub fn execute_command(cmd_line: &str) -> bool {
    let mut parts = cmd_line.trim().split_whitespace();
    let cmd = match parts.next() {
        Some(c) => c,
        None => return false, // Empty line
    };

    match cmd {
        "help" => print_help(),
        "info" => print_platform_info(),
        "mem" => mem::pmem::debug_info(),
        "ps" => proc::scheduler::ps(),
        "ls" => proc::roottask::initrd::print_files(),
        "exec" => {
            if let Some(name) = parts.next() {
                proc::roottask::spawn(name);
            } else {
                printk!("Usage: exec <name>\n");
            }
        }
        "cat" => {
            if let Some(name) = parts.next() {
                proc::roottask::initrd::cat_file(name);
            } else {
                printk!("Usage: cat <name>\n");
            }
        }
        "boot" | "exit" => return true, // Signal to exit shell
        "kpt" => print_kpt(),
        "debug" => {
            if let (Some(addr_str), Some(type_str)) = (parts.next(), parts.next()) {
                debug_struct(addr_str, type_str);
            } else {
                printk!("Usage: debug <addr> <type>\n");
            }
        }
        "inspect" => {
            if let (Some(addr_str), Some(len_str)) = (parts.next(), parts.next()) {
                inspect(addr_str, len_str);
            } else {
                printk!("Usage: inspect <addr> <len>\n");
            }
        }
        "shutdown" => {
            hal::platform::shutdown();
        }
        "reboot" => {
            hal::platform::reboot();
        }
        _ => printk!("Unknown command: {}\n", cmd),
    }
    false
}

fn print_kpt() {
    let kpt = mem::vm::KERNEL_PAGE_TABLE.get();
    match kpt {
        None => return,
        Some(pt) => pt.debug_print(),
    }
}

fn print_help() {
    printk!("Available commands:\n");
    printk!("  help                - Show this help message\n");
    printk!("  info                - Show platform information\n");
    printk!("  mem                 - Show memory status\n");
    printk!("  kpt                 - Show kernel pagetable\n");
    printk!("  inspect             - Inspect memory\n");
    printk!("  sched               - Show scheduler status\n");
    printk!("  ps                  - List processes/threads (scheduler status)\n");
    printk!("  ls                  - List initrd files\n");
    printk!("  exec <name>         - Spawn process from initrd\n");
    printk!("  cat <name>          - Concatenate and print file content\n");
    printk!("  boot                - Boot system (exit shell)\n");
    printk!("  shutdown            - Shutdown machine\n");
    printk!("  reboot              - Reboot machine\n");
    printk!("  debug <addr> <type> - Debug print struct at address\n");
    printk!("       types: tcb, pagetable, cnode, capability\n");
}

fn print_platform_info() {
    printk!("Platform: {}\n", hal::ARCH);
    let info = platform::get();
    printk!("{:?}", info);
}

fn debug_struct(addr_str: &str, type_str: &str) {
    let addr = if addr_str.starts_with("0x") {
        usize::from_str_radix(&addr_str[2..], 16)
    } else {
        usize::from_str_radix(addr_str, 10)
    };

    let addr = match addr {
        Ok(a) => a,
        Err(_) => {
            printk!("Invalid address format\n");
            return;
        }
    };

    match type_str {
        "tcb" => {
            let tcb = unsafe { VirtAddr::from(addr).as_ref::<TCB>() };
            tcb.debug_print();
        }
        "pagetable" => {
            let pt = unsafe { VirtAddr::from(addr).as_ref::<PageTable>() };
            pt.debug_print();
        }
        "cnode" => {
            let cnode = unsafe { VirtAddr::from(addr).as_ref::<CNode>() };
            cnode.debug_print();
        }
        "capability" => {
            let cap = unsafe { VirtAddr::from(addr).as_ref::<Capability>() };
            printk!("{}", cap)
        }
        _ => printk!("Unknown type: {}\n", type_str),
    }
}

fn inspect(addr_str: &str, len_str: &str) {
    let addr = if addr_str.starts_with("0x") {
        usize::from_str_radix(&addr_str[2..], 16)
    } else {
        usize::from_str_radix(addr_str, 10)
    };

    let len = if len_str.starts_with("0x") {
        usize::from_str_radix(&len_str[2..], 16)
    } else {
        usize::from_str_radix(len_str, 10)
    };

    let (addr, len) = match (addr, len) {
        (Ok(a), Ok(l)) => (a, l),
        _ => {
            printk!("Invalid address or length format\n");
            return;
        }
    };

    for i in (0..len).step_by(16) {
        printk!("{:016x}:  ", addr + i);
        for j in 0..16 {
            if i + j < len {
                let byte = unsafe { *VirtAddr::from(addr + i + j).as_ptr::<u8>() };
                printk!("{:02x} ", byte);
            } else {
                printk!("   ");
            }
        }

        printk!(" |");
        for j in 0..16 {
            if i + j < len {
                let byte = unsafe { *VirtAddr::from(addr + i + j).as_ptr::<u8>() };
                if byte >= 32 && byte <= 126 {
                    printk!("{}", byte as char);
                } else {
                    printk!(".");
                }
            } else {
                printk!(" ");
            }
        }
        printk!("|\n");
    }
}
