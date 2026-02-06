use super::asm;
use super::sbi;

pub const MAX_CPUS: usize = 8;

pub fn cpu_id() -> usize {
    unsafe { asm::read_tp() }
}
pub fn read_cycle() -> usize {
    unsafe { asm::rdcycle() }
}
