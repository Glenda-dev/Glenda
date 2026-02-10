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

            // Try to print as UTF-8 string for better display
            let len = utcb.available_data();
            if len > 0 {
                let start = utcb.head;
                if let Some(slice) = utcb.ipc_buffer.get(start..start + len) {
                    match core::str::from_utf8(slice) {
                        Ok(s) => printk!("{}", s),
                        Err(_) => {
                            printk!(
                                "Kernel::ConsolePutStr warning: invalid UTF-8, printing as bytes"
                            );
                            for &b in slice {
                                printk!("{}", b as char);
                            }
                        }
                    }
                    utcb.head += len;
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

            // Implementation: loop read char until \n or \r
            let mut buf = [0u8; 256];
            let mut count = 0;
            loop {
                let c = hal::console::read();
                let b = c as u8;

                // Echo back
                printk!("{}", c as char);

                if b == b'\r' || b == b'\n' || count >= buf.len() {
                    break;
                }

                buf[count] = b;
                count += 1;
            }

            utcb.write(&buf[..count]);
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
