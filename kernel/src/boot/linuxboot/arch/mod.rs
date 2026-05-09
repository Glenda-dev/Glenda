#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
pub mod arm;
#[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
pub mod riscv;
