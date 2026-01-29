pub mod asid;
pub mod elf;
pub mod roottask;
pub mod scheduler;
pub mod thread;

use crate::hal::platform::PlatformInfo;
pub use elf::ElfFile;
pub use thread::KSTACK_PAGES;
pub use thread::{TCB, ThreadState};

pub fn init(cpuid: usize, _info: PlatformInfo) {
    if cpuid == 0 {
        roottask::init();
    }
}
