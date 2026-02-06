use super::asm;

pub const MAX_CPUS: usize = 8;

pub fn cpu_id() -> usize {
    asm::read_tp()
}
pub fn read_cycle() -> usize {
    asm::rdcycle()
}
