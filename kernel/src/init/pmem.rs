use crate::mem::pmem::initialize_regions;
use spin::Once;

static PMEM_INIT: Once<()> = Once::new();
pub fn init(hartid: usize, _dtb: *const u8) {
    PMEM_INIT.call_once(|| initialize_regions(hartid));
}
