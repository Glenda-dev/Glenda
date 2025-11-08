use crate::mem::vm::{self, init_kernel_vm};
use spin::{Mutex, Once};

static VM_INIT: Once<()> = Once::new();
static VM_LOCK: Mutex<()> = Mutex::new(());

pub fn init(hartid: usize, _dtb: *const u8) {
    let _lock = VM_LOCK.lock();
    VM_INIT.call_once(|| {
        init_kernel_vm(hartid);
    });
    vm::switch_to_kernel(hartid);
}
