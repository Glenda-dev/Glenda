use super::asm;

const PSCI_SYSTEM_OFF: u32 = 0x84000008;
const PSCI_SYSTEM_RESET: u32 = 0x84000009;
const PSCI_CPU_ON_64: u32 = 0xC4000003;

#[inline(always)]
fn psci_call(func: u32, arg0: usize, arg1: usize, arg2: usize) -> usize {
    unsafe { asm::psci_hvc_call(func as usize, arg0, arg1, arg2) }
}

pub fn cpu_on(target_cpu: usize, entry_point: usize, context_id: usize) -> usize {
    psci_call(PSCI_CPU_ON_64, target_cpu, entry_point, context_id)
}

pub fn system_off() -> ! {
    psci_call(PSCI_SYSTEM_OFF, 0, 0, 0);
    loop {
        asm::wfi()
    }
}

pub fn system_reset() -> ! {
    psci_call(PSCI_SYSTEM_RESET, 0, 0, 0);
    loop {
        asm::wfi()
    }
}
