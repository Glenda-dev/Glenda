pub const MAGIC_VALUE: u32 = 0x74726976; // "virt"
pub const VERSION_1: u32 = 1;
pub const VERSION_2: u32 = 2;

// Device IDs
pub const DEV_ID_NET: u32 = 1;
pub const DEV_ID_BLOCK: u32 = 2;
pub const DEV_ID_CONSOLE: u32 = 3;
pub const DEV_ID_ENTROPY: u32 = 4;
pub const DEV_ID_GPU: u32 = 16;
pub const DEV_ID_INPUT: u32 = 18;

// MMIO Offsets
pub const OFF_MAGIC: usize = 0x000;
pub const OFF_VERSION: usize = 0x004;
pub const OFF_DEVICE_ID: usize = 0x008;
pub const OFF_VENDOR_ID: usize = 0x00c;
pub const OFF_DEVICE_FEATURES: usize = 0x010;
pub const OFF_DEVICE_FEATURES_SEL: usize = 0x014;
pub const OFF_DRIVER_FEATURES: usize = 0x020;
pub const OFF_DRIVER_FEATURES_SEL: usize = 0x024;
pub const OFF_QUEUE_SEL: usize = 0x030;
pub const OFF_QUEUE_NUM_MAX: usize = 0x034;
pub const OFF_QUEUE_NUM: usize = 0x038;
pub const OFF_QUEUE_READY: usize = 0x044;
pub const OFF_QUEUE_NOTIFY: usize = 0x050;
pub const OFF_INTERRUPT_STATUS: usize = 0x060;
pub const OFF_INTERRUPT_ACK: usize = 0x064;
pub const OFF_STATUS: usize = 0x070;
pub const OFF_QUEUE_DESC_LOW: usize = 0x080;
pub const OFF_QUEUE_DESC_HIGH: usize = 0x084;
pub const OFF_QUEUE_DRIVER_LOW: usize = 0x090;
pub const OFF_QUEUE_DRIVER_HIGH: usize = 0x094;
pub const OFF_QUEUE_DEVICE_LOW: usize = 0x0a0;
pub const OFF_QUEUE_DEVICE_HIGH: usize = 0x0a4;
pub const OFF_CONFIG_GENERATION: usize = 0x0fc;
pub const OFF_CONFIG: usize = 0x100;

// Status bits
pub const STATUS_ACKNOWLEDGE: u32 = 1;
pub const STATUS_DRIVER: u32 = 2;
pub const STATUS_DRIVER_OK: u32 = 4;
pub const STATUS_FEATURES_OK: u32 = 8;
pub const STATUS_DEVICE_NEEDS_RESET: u32 = 64;
pub const STATUS_FAILED: u32 = 128;
