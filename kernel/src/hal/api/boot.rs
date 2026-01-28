use core::arch::global_asm;

use super::platform::PlatformInfo;

global_asm!("");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootLoaderType {
    NULL,
}

pub static mut BOOT_LOADER_TYPE: BootLoaderType = BootLoaderType::NULL;

pub fn detect() -> (usize, PlatformInfo) {
    unimplemented!()
}
