use super::NUM_DESCS;
use super::VRingDesc;
use super::vring::{VRING_DESC_F_NEXT, VRING_DESC_F_WRITE};
use super::{VIRTIO_BLK_F_CONFIG_WCE, VIRTIO_BLK_F_MQ, VIRTIO_BLK_F_RO, VIRTIO_BLK_F_SCSI};
use super::{
    VIRTIO_CONFIG_S_ACKNOWLEDGE, VIRTIO_CONFIG_S_DRIVER, VIRTIO_CONFIG_S_DRIVER_OK,
    VIRTIO_CONFIG_S_FEATURES_OK,
};
use super::{VIRTIO_F_ANY_LAYOUT, VIRTIO_RING_F_EVENT_IDX, VIRTIO_RING_F_INDIRECT_DESC};
use super::{
    VIRTIO_MMIO_DEVICE_FEATURES, VIRTIO_MMIO_DEVICE_ID, VIRTIO_MMIO_GUEST_PAGE_SIZE,
    VIRTIO_MMIO_INTERRUPT_ACK, VIRTIO_MMIO_INTERRUPT_STATUS, VIRTIO_MMIO_MAGIC_VALUE,
    VIRTIO_MMIO_QUEUE_NOTIFY, VIRTIO_MMIO_QUEUE_NUM, VIRTIO_MMIO_QUEUE_NUM_MAX,
    VIRTIO_MMIO_QUEUE_PFN, VIRTIO_MMIO_QUEUE_SEL, VIRTIO_MMIO_STATUS, VIRTIO_MMIO_VENDOR_ID,
    VIRTIO_MMIO_VERSION,
};
use super::{reg_read, reg_write};
use crate::mem::PGSIZE;
use crate::mem::pmem;
use crate::printk;
use core::ptr::{read_volatile, write_volatile};
use riscv::register::sstatus;
use spin::Mutex;

struct Disk {
    pub pages: Option<usize>,
    pub init_done: bool,
}

static DISK: Mutex<Disk> = Mutex::new(Disk { pages: None, init_done: false });

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

pub fn rw(buf: *mut u8, blockno: u32, write: bool) {
    // Disable interrupts to avoid deadlock with ISR
    let sstatus_val = sstatus::read();
    let sie_enabled = sstatus_val.sie();
    unsafe {
        sstatus::clear_sie();
    }

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
        (*desc_ptr.add(1)).flags =
            VRING_DESC_F_NEXT | (if !write { VRING_DESC_F_WRITE } else { 0 });
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

    if sie_enabled {
        unsafe {
            sstatus::set_sie();
        }
    }

    loop {
        let state = DISK_STATE.lock();
        if state.status[idx] != 0xFF {
            break;
        }
        drop(state);
    }
}

pub fn intr() {
    let _disk = DISK.lock();
    let status = reg_read(VIRTIO_MMIO_INTERRUPT_STATUS);
    reg_write(VIRTIO_MMIO_INTERRUPT_ACK, status & 0x3);
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
        panic!("VirtIO: invalid device");
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
        panic!("VirtIO: features not ok");
    }

    reg_write(VIRTIO_MMIO_QUEUE_SEL, 0);

    let max = reg_read(VIRTIO_MMIO_QUEUE_NUM_MAX);
    if max == 0 {
        panic!("VirtIO: queue num max 0");
    }
    if max < NUM_DESCS as u32 {
        panic!("VirtIO: queue num max too small");
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
    printk!("VirtIO: Disk initialized (Legacy)\n");
}
