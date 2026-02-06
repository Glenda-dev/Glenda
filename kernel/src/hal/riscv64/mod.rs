mod asm;
pub mod boot;
pub mod console;
pub mod cpu;
mod dtb;
pub mod irq;
pub mod mem;
pub mod platform;
mod plic;
pub mod proc;
pub mod runtime;
mod sbi;
pub mod timer;
pub mod trap;

pub const ARCH: &'static str = "riscv64";
