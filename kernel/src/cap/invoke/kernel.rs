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
                if !utcb.write_byte(b) {
                    break; // Buffer full
                }
                count += 1;
            }

            utcb.mrs_regs[0] = count;
            errcode::SUCCESS
        }
        kernelmethod::SHELL => {
            if !cap.has_rights(Rights::EXECUTE) {
                log!("Kernel::Shell failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            #[cfg(feature = "shell")]
            crate::shell::run();
            errcode::SUCCESS
        }
        kernelmethod::GET_TIME => {
            if !cap.has_rights(Rights::READ) {
                log!("Kernel::TimeNow failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            let now = hal::timer::get_time();
            utcb.mrs_regs[0] = now;
            errcode::SUCCESS
        }
        _ => {
            log!("Kernel::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}
