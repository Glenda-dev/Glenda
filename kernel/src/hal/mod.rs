#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32")))]
pub mod api;

#[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32")))]
pub use api::*;

#[cfg(target_arch = "riscv32")]
pub mod riscv32;

#[cfg(target_arch = "riscv32")]
pub use riscv32::*;
