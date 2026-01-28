use crate::hal::platform::PlatformInfo;
use crate::mem::pmem::initialize_regions;
use spin::Once;

static PMEM_INIT: Once<()> = Once::new();
pub fn init(cpuid: usize, _info: PlatformInfo) {
    PMEM_INIT.call_once(|| initialize_regions(cpuid));
}
