#![allow(dead_code)]

use crate::mem::pmem;
use crate::mem::{PGSIZE, PhysAddr};
use crate::printk;
use core::ptr::{read_volatile, write_volatile};
use spin::Mutex;
use riscv::register::sstatus;

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

#[repr(C)]
#[repr(align(16))]
struct VRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[repr(align(4))]
struct VRingUsedElem {
    id: u32,
    len: u32,
}

// Descriptor flags
const VRING_DESC_F_NEXT: u16 = 1;
const VRING_DESC_F_WRITE: u16 = 2;

struct Disk {
    pages: Option<usize>,
    init_done: bool,
}

static DISK: Mutex<Disk> = Mutex::new(Disk {
    pages: None,
    init_done: false,
});

#[repr(C)]
#[derive(Clone, Copy)]
struct BlkOutHdr {
    _type: u32,
    reserved: u32,
    sector: u64,
}

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

struct DiskState {
    headers: [BlkOutHdr; NUM_DESCS],
    status: [u8; NUM_DESCS],
}

static DISK_STATE: Mutex<DiskState> = Mutex::new(DiskState {
    headers: [BlkOutHdr { _type: 0, reserved: 0, sector: 0 }; NUM_DESCS],
    status: [0; NUM_DESCS],
});

fn reg_read(offset: usize) -> u32 {
    unsafe { read_volatile((VIRTIO0 + offset) as *const u32) }
}

fn reg_write(offset: usize, val: u32) {
    unsafe { write_volatile((VIRTIO0 + offset) as *mut u32, val) }
}

pub fn init() {
    let mut disk = DISK.lock();
    if disk.init_done {
        return;
    }

    if reg_read(VIRTIO_MMIO_MAGIC_VALUE) != 0x74726976
        || reg_read(VIRTIO_MMIO_VERSION) != 1
        || reg_read(VIRTIO_MMIO_DEVICE_ID) != 2
        || reg_read(VIRTIO_MMIO_VENDOR_ID) != 0x554d4551
    {
        panic!("virtio: invalid device");
    }

    let mut status: u32 = 0;
    status |= VIRTIO_CONFIG_S_ACKNOWLEDGE;
    reg_write(VIRTIO_MMIO_STATUS, status);

    status |= VIRTIO_CONFIG_S_DRIVER;
    reg_write(VIRTIO_MMIO_STATUS, status);

    // Features
    let mut features = reg_read(VIRTIO_MMIO_DEVICE_FEATURES) as u64;
    features &= !VIRTIO_BLK_F_RO;
    features &= !VIRTIO_BLK_F_SCSI;
    features &= !VIRTIO_BLK_F_CONFIG_WCE;
    features &= !VIRTIO_BLK_F_MQ;
    features &= !VIRTIO_F_ANY_LAYOUT;
    features &= !VIRTIO_RING_F_EVENT_IDX;
    features &= !VIRTIO_RING_F_INDIRECT_DESC;

    reg_write(VIRTIO_MMIO_DEVICE_FEATURES, features as u32);

    status |= VIRTIO_CONFIG_S_FEATURES_OK;
    reg_write(VIRTIO_MMIO_STATUS, status);

    if (reg_read(VIRTIO_MMIO_STATUS) & VIRTIO_CONFIG_S_FEATURES_OK) == 0 {
        panic!("virtio: features not ok");
    }

    reg_write(VIRTIO_MMIO_QUEUE_SEL, 0);

    let max = reg_read(VIRTIO_MMIO_QUEUE_NUM_MAX);
    if max == 0 {
        panic!("virtio: queue num max 0");
    }
    if max < NUM_DESCS as u32 {
        panic!("virtio: queue num max too small");
    }

    reg_write(VIRTIO_MMIO_QUEUE_NUM, NUM_DESCS as u32);

    // Desc (16*8=128) + Avail (6+2*8=22) + Pad -> 4096 -> Used (6+8*8=70)
    let p = pmem::alloc_contiguous(2, true);
    let page = p as usize;

    disk.pages = Some(page);

    // Setup Legacy Registers
    reg_write(VIRTIO_MMIO_GUEST_PAGE_SIZE, PGSIZE as u32);
    reg_write(VIRTIO_MMIO_QUEUE_PFN, (page / PGSIZE) as u32);

    status |= VIRTIO_CONFIG_S_DRIVER_OK;
    reg_write(VIRTIO_MMIO_STATUS, status);

    disk.init_done = true;
    printk!("virtio: disk initialized (Legacy)");
}

pub fn virtio_disk_rw(buf: *mut u8, blockno: u32, write: bool) {
    // Disable interrupts to avoid deadlock with ISR
    let sstatus_val = sstatus::read();
    let sie_enabled = sstatus_val.sie();
    unsafe { sstatus::clear_sie(); }

    let disk = DISK.lock();
    let idx = 0;

    let sector = blockno as u64 * (PGSIZE as u64 / 512);

    let mut state = DISK_STATE.lock();
    state.headers[idx].sector = sector;
    state.headers[idx]._type = if write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };
    state.headers[idx].reserved = 0;

    let head_pa = &state.headers[idx] as *const BlkOutHdr as u64;
    let data_pa = buf as u64;
    let status_pa = &state.status[idx] as *const u8 as u64;

    let page = disk.pages.expect("virtio not initialized");

    let desc_ptr = page as *mut VRingDesc;

    unsafe {
        (*desc_ptr.add(0)).addr = head_pa;
        (*desc_ptr.add(0)).len = 16;
        (*desc_ptr.add(0)).flags = VRING_DESC_F_NEXT;
        (*desc_ptr.add(0)).next = 1;

        (*desc_ptr.add(1)).addr = data_pa;
        (*desc_ptr.add(1)).len = PGSIZE as u32;
        (*desc_ptr.add(1)).flags = VRING_DESC_F_NEXT | (if !write { VRING_DESC_F_WRITE } else { 0 });
        (*desc_ptr.add(1)).next = 2;

        (*desc_ptr.add(2)).addr = status_pa;
        (*desc_ptr.add(2)).len = 1;
        (*desc_ptr.add(2)).flags = VRING_DESC_F_WRITE;
        (*desc_ptr.add(2)).next = 0;

        let avail_ptr = (page + 128) as *mut u8;
        let avail_idx_ptr = avail_ptr.add(2) as *mut u16;
        let avail_ring_ptr = avail_ptr.add(4) as *mut u16;

        let idx_val = read_volatile(avail_idx_ptr);
        write_volatile(avail_ring_ptr.add((idx_val % 8) as usize), 0);

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        write_volatile(avail_idx_ptr, idx_val.wrapping_add(1));

        reg_write(VIRTIO_MMIO_QUEUE_NOTIFY, 0);
    }

    state.status[idx] = 0xFF; // In-progress
    drop(state);
    drop(disk);

    if sie_enabled { unsafe { sstatus::set_sie(); } }

    loop {
        let state = DISK_STATE.lock();
        if state.status[idx] != 0xFF {
            break;
        }
        drop(state);
    }
}

pub fn virtio_disk_intr() {
    let _disk = DISK.lock();
    let status = reg_read(VIRTIO_MMIO_INTERRUPT_STATUS);
    reg_write(VIRTIO_MMIO_INTERRUPT_ACK, status & 0x3);
}
