mod hart;

use spin::Once;

// Global initialization guards - ensure each subsystem is initialized only once
static DTB_INIT: Once<()> = Once::new();
static UART_INIT: Once<()> = Once::new();
static PMEM_INIT: Once<()> = Once::new();
static IRQ_INIT: Once<()> = Once::new();
static VM_INIT: Once<()> = Once::new();
static FS_INIT: Once<()> = Once::new();

pub fn init(hartid: usize, dtb: *const u8) {
    // Device tree - global, once
    DTB_INIT.call_once(|| {
        crate::dtb::init(dtb);
    });

    // UART - global, once
    UART_INIT.call_once(|| {
        crate::drivers::uart::initialize_from_dtb(dtb);
    });

    // Physical memory - global, once
    PMEM_INIT.call_once(|| {
        crate::mem::pmem::initialize_regions(hartid);
    });

    // IRQ - global init once, then per-hart init
    IRQ_INIT.call_once(|| {
        crate::irq::init();
    });
    crate::irq::init_hart(hartid);

    // Virtual memory - global init once, then per-hart switch
    VM_INIT.call_once(|| {
        crate::mem::vm::init_kernel_vm(hartid);
    });
    crate::mem::vm::switch_to_kernel(hartid);

    // File system - global, once
    FS_INIT.call_once(|| {
        crate::drivers::virtio::init();
        crate::fs::buffer::init();
    });

    // Hart management - per-hart
    hart::init(hartid, dtb);
}
