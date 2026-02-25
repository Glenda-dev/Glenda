use super::super::method::*;
use crate::cap::{Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::proc::scheduler;

pub fn invoke_console(cap: &mut Capability, method: usize, _cptr: usize) -> Result<(), Error> {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("Console::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        consolemethod::CONSOLE_PUT_STR => {
            if cap.has_rights(Rights::WRITE) == false {
                error!("Console::ConsolePutStr failed: permission denied");
                return Err(Error::PermissionDenied);
            }

            // Try to print as UTF-8 string for better display
            let len = utcb.available_data();
            if len > 0 {
                let start = utcb.head;
                if let Some(slice) = utcb.ipc_buffer.get(start..start + len) {
                    match core::str::from_utf8(slice) {
                        Ok(s) => printk!("{}", s),
                        Err(_) => {
                            warn!(
                                "Console::ConsolePutStr warning: invalid UTF-8, printing as bytes"
                            );
                            for &b in slice {
                                printk!("{}", b as char);
                            }
                        }
                    }
                    utcb.head += len;
                }
            }
            Ok(())
        }
        consolemethod::CONSOLE_GET_CHAR => {
            if cap.has_rights(Rights::READ) == false {
                error!("Console::ConsoleGetChar failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let c = hal::console::read() as usize;
            utcb.mrs_regs[0] = c;
            Ok(())
        }
        consolemethod::CONSOLE_GET_STR => {
            if cap.has_rights(Rights::READ) == false {
                error!("Console::ConsoleGetStr failed: permission denied");
                return Err(Error::PermissionDenied);
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
            Ok(())
        }
        _ => Err(Error::InvalidMethod),
    }
}
