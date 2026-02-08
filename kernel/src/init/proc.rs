use crate::proc;
use crate::sync::Once;

static PROC_INIT: Once<()> = Once::new();

pub fn init() {
    PROC_INIT.call_once(|| {
        proc::init();
    });
}
