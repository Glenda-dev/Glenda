use super::super::method::*;
use crate::cap::{Capability, Rights};
use crate::hal;
use crate::printk;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_kernel(cap: &mut Capability, method: usize) -> usize {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("Kernel::invoke failed: no UTCB");
            return errcode::MAPPING_FAILED;
        }
    };

    match method {
        kernelmethod::CONSOLE_PUT_STR => {
            if cap.has_rights(Rights::WRITE) == false {
                log!("Kernel::ConsolePutStr failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }

            let mut buf = [0u8; 128];
            loop {
                let n = utcb.read(&mut buf);
                if n == 0 {
                    break;
                }
                // 简单的 best-effort 处理：尝试作为 UTF-8 打印，失败则逐字节打印
                if let Ok(s) = core::str::from_utf8(&buf[..n]) {
                    printk!("{}", s);
                } else {
                    for &b in &buf[..n] {
                        printk!("{}", b as char);
                    }
                }
            }
            errcode::SUCCESS
        }
        kernelmethod::CONSOLE_GET_CHAR => {
            if cap.has_rights(Rights::READ) == false {
                log!("Kernel::ConsoleGetChar failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            let c = hal::console::read() as usize;
            utcb.mrs_regs[0] = c;
            errcode::SUCCESS
        }
        kernelmethod::CONSOLE_GET_STR => {
            if cap.has_rights(Rights::READ) == false {
                log!("Kernel::ConsoleGetStr failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            // 清空 UTCB 缓冲区以便写入
            utcb.head = 0;
            utcb.tail = 0;

            // Implementation: loop read char until \n or \r
            let mut count = 0;
            loop {
                let c = hal::console::read();
                let b = c as u8;

                // Echo back
                crate::printk!("{}", c as char);

                if b == b'\r' || b == b'\n' {
                    break;
                }

                // Write to UTCB buffer
                if utcb.available_space() > 0 {
                    utcb.write(&[b]);
                    count += 1;
                } else {
                    break; // Buffer full
                }
            }

            utcb.mrs_regs[0] = count;
            errcode::SUCCESS
        }
        kernelmethod::SHELL => {
            if cap.has_rights(Rights::EXECUTE) == false {
                log!("Kernel::Shell failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            crate::shell::run();
            errcode::SUCCESS
        }
        _ => {
            log!("Kernel::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}
