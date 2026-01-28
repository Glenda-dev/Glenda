pub mod cause;
pub mod handler;
pub mod info;
pub mod syscall;

pub use cause::{TrapCause, TrapException, TrapInterrupt};

use crate::hal;
use crate::printk;

pub fn init_hart(hartid: usize) {
    unsafe {
        hal::trap::vector_init();
    }
    printk!("trap: Initialized for hart {}\n", hartid);
}
