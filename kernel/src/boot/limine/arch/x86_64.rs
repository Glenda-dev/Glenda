use limine::mp::Cpu;
use limine::response::MpResponse;

pub fn cpuid(cpu: &Cpu) -> usize {
    cpu.lapic_id as usize
}

pub fn bspid(mp: &MpResponse) -> usize {
    mp.bsp_lapic_id() as usize
}
