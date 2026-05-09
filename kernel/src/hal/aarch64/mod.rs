pub mod asm;
pub mod console;
pub mod cpu;
pub mod drivers;
pub mod irq;
pub mod mem;
pub mod platform;
pub mod proc;
pub mod psci;
pub mod runtime;
pub mod timer;
pub mod trap;
pub mod virt;

pub const ARCH: &str = "aarch64";
