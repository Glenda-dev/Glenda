use crate::trap::{inittraps, inittraps_hart};
use spin::{Mutex, Once};

static TRAP_INIT: Once<()> = Once::new();
static TRAP_LOCK: Mutex<()> = Mutex::new(());
pub fn init(hartid: usize, _dtb: *const u8) {
    let _lock = TRAP_LOCK.lock();
    TRAP_INIT.call_once(|| {
        inittraps();
    });
    inittraps_hart(hartid);
}
