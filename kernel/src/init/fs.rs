use crate::drivers::virtio;
use crate::fs::buffer;
use spin::Once;

static FS_INIT: Once<()> = Once::new();

pub fn init(_hartid: usize, _dtb: *const u8) {
    FS_INIT.call_once(|| {
        virtio::init();
        buffer::init();
    });
}
