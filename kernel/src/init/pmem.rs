use crate::mem::addr;
use crate::mem::pmem;
use crate::sync::Once;

static PMEM_INIT: Once<()> = Once::new();
pub fn init() {
    PMEM_INIT.call_once(|| {
        addr::init();
        pmem::initialize_regions();
    });
}
