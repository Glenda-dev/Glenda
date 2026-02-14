use limine::mp::Cpu;
use limine::response::MpResponse;

pub fn cpuid(cpu: &Cpu) -> usize {
    cpu.hartid as usize
}

pub fn bspid(mp: &MpResponse) -> usize {
    mp.bsp_hartid() as usize
}
