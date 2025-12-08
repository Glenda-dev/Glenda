pub mod disk;
pub mod vring;

pub use vring::{VRingDesc, VRingUsedElem};

use crate::mem::PGSIZE;
use crate::mem::pmem;
use crate::printk;
use core::ptr::{read_volatile, write_volatile};

// VirtIO MMIO register offsets
const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
const VIRTIO_MMIO_VERSION: usize = 0x004;
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;
const VIRTIO_MMIO_STATUS: usize = 0x070;
const VIRTIO_MMIO_QUEUE_DESC_LOW: usize = 0x080;
const VIRTIO_MMIO_QUEUE_DESC_HIGH: usize = 0x084;
const VIRTIO_MMIO_QUEUE_DRIVER_LOW: usize = 0x090;
const VIRTIO_MMIO_QUEUE_DRIVER_HIGH: usize = 0x094;
const VIRTIO_MMIO_QUEUE_DEVICE_LOW: usize = 0x0a0;
const VIRTIO_MMIO_QUEUE_DEVICE_HIGH: usize = 0x0a4;
const VIRTIO_MMIO_CONFIG: usize = 0x100;

const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028;
const VIRTIO_MMIO_QUEUE_PFN: usize = 0x040;

// VirtIO Status bits
const VIRTIO_CONFIG_S_ACKNOWLEDGE: u32 = 1;
const VIRTIO_CONFIG_S_DRIVER: u32 = 2;
const VIRTIO_CONFIG_S_DRIVER_OK: u32 = 4;
const VIRTIO_CONFIG_S_FEATURES_OK: u32 = 8;

// Feature bits
const VIRTIO_BLK_F_RO: u64 = 1 << 5;
const VIRTIO_BLK_F_SCSI: u64 = 1 << 7;
const VIRTIO_BLK_F_CONFIG_WCE: u64 = 1 << 11;
const VIRTIO_BLK_F_MQ: u64 = 1 << 12;
const VIRTIO_F_ANY_LAYOUT: u64 = 1 << 27;
const VIRTIO_RING_F_INDIRECT_DESC: u64 = 1 << 28;
const VIRTIO_RING_F_EVENT_IDX: u64 = 1 << 29;

const NUM_DESCS: usize = 8; // Ring size

// MMIO Base Address
const VIRTIO0: usize = 0x10001000;

fn reg_read(offset: usize) -> u32 {
    unsafe { read_volatile((VIRTIO0 + offset) as *const u32) }
}

fn reg_write(offset: usize, val: u32) {
    unsafe { write_volatile((VIRTIO0 + offset) as *mut u32, val) }
}

pub fn init() {
    disk::init();
}
