pub mod cause;
pub mod handler;
pub mod info;
pub mod syscall;

pub use cause::{TrapCause, TrapException, TrapInterrupt};

use crate::hal;
use crate::printk;

pub fn init_cpu() {
    let cpuid = hal::cpu::cpu_id();
    unsafe {
        hal::trap::vector_init();
    }
    printk!("trap: Initialized for cpu {}\n", cpuid);
}
