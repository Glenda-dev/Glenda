#![allow(dead_code)]

use crate::drivers::virtio;
use crate::printk;
use spin::Mutex;

pub const BLOCK_SIZE: usize = 4096;
pub const N_BUFFER: usize = 32;

// Two lists:
// HEAD_INACTIVE: Unused buffers (refcnt=0), ordered by LRU.
// HEAD_ACTIVE: Used buffers (refcnt>0).
pub const HEAD_INACTIVE: usize = N_BUFFER;
pub const HEAD_ACTIVE: usize = N_BUFFER + 1;

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

struct LRUCache {
    bufs: [Buffer; N_BUFFER],
    next: [usize; N_BUFFER + 2],
    prev: [usize; N_BUFFER + 2],
}

static CACHE: Mutex<LRUCache> = Mutex::new(LRUCache {
    bufs: [const { Buffer::new() }; N_BUFFER],
    next: [0; N_BUFFER + 2],
    prev: [0; N_BUFFER + 2],
});

impl LRUCache {
    fn debug_print_list(&self) {
        let head = HEAD_INACTIVE;
        let mut cur = self.next[head];
        crate::printk!("LRU: [ ");
        while cur != head {
            crate::printk!("{} ", cur);
            cur = self.next[cur];
        }
        crate::printk!("]\n");
    }

    fn insert_head(&mut self, head: usize, idx: usize) {
        let first = self.next[head];
        self.next[head] = idx;
        self.prev[idx] = head;
        self.next[idx] = first;
        self.prev[first] = idx;
    }

    fn remove(&mut self, idx: usize) {
        let p = self.prev[idx];
        let n = self.next[idx];
        self.next[p] = n;
        self.prev[n] = p;
    }

    fn debug_print(&self, state_num: usize) {
        crate::printk!("state-{} buffer cache information:\n", state_num);

        crate::printk!("1.active list:\n");
        let mut cur = self.next[HEAD_ACTIVE];
        while cur != HEAD_ACTIVE {
            let b = &self.bufs[cur];
            crate::printk!(
                "buffer {} (ref = {}): page(pa = 0x{:x})-> block[{}]\n",
                cur,
                b.refcnt,
                b.data.as_ptr() as usize,
                b.block_no
            );
            cur = self.next[cur];
        }
        crate::printk!("over!\n");

        crate::printk!("2.inactive list:\n");
        let mut cur = self.next[HEAD_INACTIVE];
        while cur != HEAD_INACTIVE {
            let b = &self.bufs[cur];
            crate::printk!(
                "buffer {} (ref = {}): page(pa = 0x{:x})-> block[{}]\n",
                cur,
                b.refcnt,
                b.data.as_ptr() as usize,
                b.block_no
            );
            cur = self.next[cur];
        }
        crate::printk!("over!\n");
    }
}

static mut STATE_COUNTER: usize = 1;

pub fn debug_state() {
    let c = CACHE.lock();
    unsafe {
        c.debug_print(STATE_COUNTER);
        STATE_COUNTER += 1;
    }
}

pub fn init() {
    let mut c = CACHE.lock();

    c.next[HEAD_ACTIVE] = HEAD_ACTIVE;
    c.prev[HEAD_ACTIVE] = HEAD_ACTIVE;
    c.next[HEAD_INACTIVE] = 0;
    c.prev[HEAD_INACTIVE] = N_BUFFER - 1;

    for i in 0..N_BUFFER {
        c.next[i] = if i == N_BUFFER - 1 { HEAD_INACTIVE } else { i + 1 };
        c.prev[i] = if i == 0 { HEAD_INACTIVE } else { i - 1 };
    }

    printk!("Buffer: cache initialized with {} buffers\n", N_BUFFER);
}

fn get(dev: u32, blockno: u32) -> usize {
    let mut c = CACHE.lock();

    // Search Active List
    let mut b = c.next[HEAD_ACTIVE];
    while b != HEAD_ACTIVE {
        if c.bufs[b].dev == dev && c.bufs[b].block_no == blockno {
            if c.bufs[b].locked {
                // TODO: Implement sleep waiting for buffer lock
                // For now, we assume no contention or handle it higher up
            }
            c.bufs[b].locked = true;
            c.debug_print_list();
            return b;
        }
        b = c.next[b];
    }

    // Search Inactive List
    b = c.next[HEAD_INACTIVE];
    while b != HEAD_INACTIVE {
        if c.bufs[b].dev == dev && c.bufs[b].block_no == blockno {
            c.bufs[b].refcnt += 1;
            c.bufs[b].locked = true;
            c.remove(b);
            c.insert_head(HEAD_ACTIVE, b);
            c.debug_print_list();
            return b;
        }
        b = c.next[b];
    }

    // Not cached.
    let lru = c.prev[HEAD_INACTIVE];
    if lru == HEAD_INACTIVE {
        panic!("buffer_get: no buffers");
    }

    if c.bufs[lru].refcnt != 0 {
        panic!("buffer_get: inactive list has refcnt != 0");
    }

    // Recycle lru
    c.bufs[lru].dev = dev;
    c.bufs[lru].block_no = blockno;
    c.bufs[lru].valid = false;
    c.bufs[lru].refcnt = 1;
    c.bufs[lru].locked = true;

    c.remove(lru);
    c.insert_head(HEAD_ACTIVE, lru);

    c.debug_print_list();
    return lru;
}

pub fn read(dev: u32, blockno: u32) -> usize {
    let idx = get(dev, blockno);
    let valid = {
        let c = CACHE.lock();
        c.bufs[idx].valid
    };

    if !valid {
        let buf_ptr = {
            let mut c = CACHE.lock();
            c.bufs[idx].data.as_mut_ptr()
        };

        virtio::disk::rw(buf_ptr, blockno, false);

        let mut c = CACHE.lock();
        c.bufs[idx].valid = true;
    }
    idx
}

pub fn write(idx: usize) {
    let (buf_ptr, blockno) = {
        let mut c = CACHE.lock();
        (c.bufs[idx].data.as_mut_ptr(), c.bufs[idx].block_no)
    };
    virtio::disk::rw(buf_ptr, blockno, true);

    let mut c = CACHE.lock();
    c.bufs[idx].dirty = false;
}

pub fn release(idx: usize) {
    let mut c = CACHE.lock();
    c.bufs[idx].refcnt -= 1;
    c.bufs[idx].locked = false;

    if c.bufs[idx].refcnt == 0 {
        // Move from Active to Inactive Head (MRU)
        c.remove(idx);
        c.insert_head(HEAD_INACTIVE, idx);
    }
    c.debug_print_list();
}

pub fn get_data_ptr(idx: usize) -> *mut u8 {
    let mut c = CACHE.lock();
    c.bufs[idx].data.as_mut_ptr()
}
