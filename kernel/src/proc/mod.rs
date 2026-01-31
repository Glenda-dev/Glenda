pub mod asid;
pub mod elf;
pub mod roottask;
pub mod scheduler;
pub mod thread;

pub use elf::ElfFile;
pub use thread::{TCB, ThreadState};

pub fn init() {
    roottask::init();
}
