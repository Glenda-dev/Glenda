pub mod boot;
pub mod console;
pub mod cpu;
pub mod irq;
pub mod mem;
pub mod platform;
pub mod proc;
pub mod runtime;
pub mod timer;
pub mod trap;
pub mod virt;

pub const ARCH: &str = "x86_64";
