pub mod boot;
pub mod console;
pub mod cpu;
mod csr;
pub mod irq;
pub mod mem;
pub mod platform;
pub mod proc;
pub mod runtime;
pub mod timer;
pub mod trap;

pub use boot::*;
pub use console::*;
pub use cpu::*;
pub use irq::*;
pub use mem::*;
pub use platform::*;
pub use proc::*;
pub use runtime::*;
pub use timer::*;
pub use trap::*;

pub const ARCH: &str = "loongarch64";
