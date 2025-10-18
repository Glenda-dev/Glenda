use crate::mem::pmem::initialize_regions;
use spin::Once;

static PMEM_INIT: Once<()> = Once::new();
static PMEM_LOCK: spin::Mutex<()> = spin::Mutex::new(());
pub fn pmem_init(hartid: usize) {
    let _lock = PMEM_LOCK.lock();
    PMEM_INIT.call_once(|| initialize_regions(hartid));
}
