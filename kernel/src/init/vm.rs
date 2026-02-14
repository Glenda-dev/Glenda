use crate::mem::vm;
use crate::sync::Once;

static VM_INIT: Once<()> = Once::new();

pub fn init() {
    VM_INIT.call_once(|| {
        vm::init_kernel_vm();
    });
}
