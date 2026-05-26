pub mod cause;
pub mod handler;
pub mod syscall;

pub use cause::*;

use crate::hal;

pub fn init_cpu() {
    let cpuid = hal::cpu::cpu_id();
    unsafe {
        hal::trap::vector_init();
    }
    log!("trap: Initialized for cpu {}", cpuid);
}
