use crate::hal::platform::PlatformInfo;
use crate::mem::vm::{self, init_kernel_vm};
use spin::Once;

static VM_INIT: Once<()> = Once::new();

pub fn init(cpuid: usize, _info: PlatformInfo) {
    VM_INIT.call_once(|| {
        init_kernel_vm(cpuid);
    });
    vm::switch_to_kernel(cpuid);
}
