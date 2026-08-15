#![allow(dead_code)]

mod lru;

use crate::drivers::virtio;
use crate::printk;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use lru::{BufferId, LRUCache};

pub const BLOCK_SIZE: usize = 4096;
pub const N_BUFFER: usize = 32;

pub type BlockNo = u32;

pub struct Buffer {
    pub data: [u8; BLOCK_SIZE], // Data buffer
    pub block_no: BlockNo,      // Block number on disk
    pub dev: u32,               // Device ID
    pub refcnt: u32,            // Reference count
    pub valid: bool,            // Is data valid?
    pub dirty: bool,            // Does data need writing to disk?
    pub locked: bool,           // SleepLock equivalent
}

impl Buffer {
    pub const fn new() -> Self {
        Self {
            data: [0; BLOCK_SIZE],
            block_no: 0,
            dev: 0,
            refcnt: 0,
            valid: false,
            dirty: false,
            locked: false,
        }
    }
}

static CACHE: Mutex<LRUCache> = Mutex::new(LRUCache::new());

static STATE_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub fn debug_state() {
    let c = CACHE.lock();
    let state_num = STATE_COUNTER.fetch_add(1, Ordering::Relaxed);
    
    crate::printk!("state-{} buffer cache information:\n", state_num);

    crate::printk!("1.active list:\n");
    for id in c.iter_active() {
        let b = c.get_buffer(id);
        crate::printk!(
            "buffer {}(ref ={}): page(pa = 0x{:x})-> block[{}]\n",
            id.as_usize(),
            b.refcnt,
            b.data.as_ptr() as usize,
            b.block_no
        );
    }
    crate::printk!("over!\n");

    crate::printk!("2.inactive list:\n");
    for id in c.iter_inactive() {
        let b = c.get_buffer(id);
        crate::printk!(
            "buffer {}(ref ={}): page(pa = 0x{:x})-> block[{}]\n",
            id.as_usize(),
            b.refcnt,
            b.data.as_ptr() as usize,
            b.block_no
        );
    }
    crate::printk!("over!\n");
}

pub fn init() {
    let mut c = CACHE.lock();
    c.init();
    printk!("Buffer: cache initialized with {} buffers\n", N_BUFFER);
}

fn get(dev: u32, blockno: u32) -> BufferId {
    let mut c = CACHE.lock();

    // Search Active List
    if let Some(id) = c.find_active(dev, blockno) {
        let buf = c.get_buffer_mut(id);
        if buf.locked {
            // TODO: Implement sleep waiting for buffer lock
            // For now, we assume no contention or handle it higher up
        }
        buf.locked = true;
        return id;
    }

    // Search Inactive List
    if let Some(id) = c.find_inactive(dev, blockno) {
        c.promote_to_active(id);
        c.get_buffer_mut(id).locked = true;
        return id;
    }

    // Not cached - recycle LRU buffer
    c.recycle_lru(dev, blockno)
}

pub fn read(dev: u32, blockno: u32) -> usize {
    let id = get(dev, blockno);
    let valid = {
        let c = CACHE.lock();
        c.get_buffer(id).valid
    };

    if !valid {
        let buf_ptr = {
            let c = CACHE.lock();
            c.get_buffer(id).data.as_ptr() as *mut u8
        };

        virtio::disk::rw(buf_ptr, blockno, false);

        let mut c = CACHE.lock();
        c.get_buffer_mut(id).valid = true;
    }
    id.as_usize()
}

pub fn write(idx: usize) {
    let id = BufferId::new(idx).expect("Invalid buffer index");
    let (buf_ptr, blockno) = {
        let c = CACHE.lock();
        let buf = c.get_buffer(id);
        (buf.data.as_ptr() as *mut u8, buf.block_no)
    };
    virtio::disk::rw(buf_ptr, blockno, true);

    let mut c = CACHE.lock();
    c.get_buffer_mut(id).dirty = false;
}

pub fn release(idx: usize) {
    let id = BufferId::new(idx).expect("Invalid buffer index");
    let mut c = CACHE.lock();
    let buf = c.get_buffer_mut(id);
    buf.refcnt -= 1;
    buf.locked = false;

    if buf.refcnt == 0 {
        // Move from Active to Inactive Head (MRU)
        c.demote_to_inactive(id);
    }
}

pub fn get_data_ptr(idx: usize) -> *mut u8 {
    let id = BufferId::new(idx).expect("Invalid buffer index");
    let c = CACHE.lock();
    c.get_buffer(id).data.as_ptr() as *mut u8
}
