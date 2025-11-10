use crate::mem::vm::{self, init_kernel_vm};
use spin::Once;

static VM_INIT: Once<()> = Once::new();

pub fn init(hartid: usize, _dtb: *const u8) {
    VM_INIT.call_once(|| {
        init_kernel_vm(hartid);
    });
    vm::switch_to_kernel(hartid);
}
