use crate::mem::vm::{init_kernel_vm, vm_switch_to_kernel};
use spin::Mutex;
use spin::Once;

static VM_INIT: Once<()> = Once::new();
static VM_LOCK: Mutex<()> = Mutex::new(());

pub fn vm_init(hartid: usize) {
    let _lock = VM_LOCK.lock();
    VM_INIT.call_once(|| {
        init_kernel_vm(hartid);
    });
    vm_switch_to_kernel(hartid);
}
