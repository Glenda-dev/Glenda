#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(not(target_arch = "riscv64"))]
pub mod api;

#[cfg(not(target_arch = "riscv64"))]
pub use api::*;
