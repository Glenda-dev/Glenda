#![allow(dead_code)]

use crate::printk;
use spin::Mutex;

pub const BLOCK_SIZE: usize = 4096;
pub const N_BUFFER: usize = 32;

pub type BlockNo = u32;

pub struct Buffer {
    pub data: [u8; BLOCK_SIZE], // Data buffer
    pub block_no: BlockNo,      // Block number on disk
    pub dev: u32,               // Device ID
    pub refcnt: u32,            // Reference count
    pub valid: bool,
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
    next: [usize; N_BUFFER + 1],
    prev: [usize; N_BUFFER + 1],
}

static CACHE: Mutex<LRUCache> = Mutex::new(LRUCache {
    bufs: [const { Buffer::new() }; N_BUFFER],
    next: [0; N_BUFFER + 1],
    prev: [0; N_BUFFER + 1],
});

pub fn init() {
    let mut c = CACHE.lock();
    let head = N_BUFFER;

    // Head <-> Buf0 <-> Buf1 <-> ... <-> BufN-1 <-> Head
    c.next[head] = 0;
    c.prev[head] = N_BUFFER - 1;

    for i in 0..N_BUFFER {
        c.next[i] = if i == N_BUFFER - 1 { head } else { i + 1 };
        c.prev[i] = if i == 0 { head } else { i - 1 };
    }

    printk!("buffer: cache initialized with {} buffers", N_BUFFER);
}

fn bget(dev: u32, blockno: u32) -> usize {
    let mut c = CACHE.lock();

    // Search for cached
    let head = N_BUFFER;
    let mut b = c.next[head];
    while b != head {
        if c.bufs[b].dev == dev && c.bufs[b].block_no == blockno {
            c.bufs[b].refcnt += 1;
            // Move to head (MRU)
            // unlink
            let p = c.prev[b];
            let n = c.next[b];
            c.next[p] = n;
            c.prev[n] = p;

            // link at head
            let first = c.next[head];
            c.next[head] = b;
            c.prev[b] = head;
            c.next[b] = first;
            c.prev[first] = b;

            if c.bufs[b].locked {
                // TODO: Implement sleep waiting for buffer lock
                // For now, we assume no contention or handle it higher up
            }
            c.bufs[b].locked = true;
            return b;
        }
        b = c.next[b];
    }

    // Not cached. Recycle LRU.
    let mut b = c.prev[head];
    while b != head {
        if c.bufs[b].refcnt == 0 {
            c.bufs[b].dev = dev;
            c.bufs[b].block_no = blockno;
            c.bufs[b].valid = false;
            c.bufs[b].refcnt = 1;
            c.bufs[b].locked = true;

            // Move to head
            let p = c.prev[b];
            let n = c.next[b];
            c.next[p] = n;
            c.prev[n] = p;

            let first = c.next[head];
            c.next[head] = b;
            c.prev[b] = head;
            c.next[b] = first;
            c.prev[first] = b;

            return b;
        }
        b = c.prev[b];
    }

    panic!("bget: no buffers");
}

pub fn bread(dev: u32, blockno: u32) -> usize {
    let idx = bget(dev, blockno);
    let valid = {
        let c = CACHE.lock();
        c.bufs[idx].valid
    };

    if !valid {
        let buf_ptr = {
            let mut c = CACHE.lock();
            c.bufs[idx].data.as_mut_ptr()
        };

        crate::fs::virtio::virtio_disk_rw(buf_ptr, blockno, false);

        let mut c = CACHE.lock();
        c.bufs[idx].valid = true;
    }
    idx
}

pub fn bwrite(idx: usize) {
    let (buf_ptr, blockno) = {
        let mut c = CACHE.lock();
        (c.bufs[idx].data.as_mut_ptr(), c.bufs[idx].block_no)
    };
    crate::fs::virtio::virtio_disk_rw(buf_ptr, blockno, true);

    let mut c = CACHE.lock();
    c.bufs[idx].dirty = false;
}

pub fn brelse(idx: usize) {
    let mut c = CACHE.lock();
    c.bufs[idx].refcnt -= 1;
    c.bufs[idx].locked = false;

    // Move to head (MRU)
    let b = idx;
    let head = N_BUFFER;
    let p = c.prev[b];
    let n = c.next[b];
    c.next[p] = n;
    c.prev[n] = p;

    let first = c.next[head];
    c.next[head] = b;
    c.prev[b] = head;
    c.next[b] = first;
    c.prev[first] = b;
}

pub fn get_data_ptr(idx: usize) -> *mut u8 {
    let mut c = CACHE.lock();
    c.bufs[idx].data.as_mut_ptr()
}
