use crate::mem::pmem;
use spin::Once;

static PMEM_INIT: Once<()> = Once::new();
pub fn init() {
    PMEM_INIT.call_once(|| pmem::initialize_regions());
}
