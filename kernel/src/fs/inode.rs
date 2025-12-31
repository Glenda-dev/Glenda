use crate::fs::buffer;
use crate::fs::buffer::BLOCK_SIZE;
use crate::fs::fs::get_sb;
use crate::fs::bitmap;
use crate::printk;
use spin::Mutex;
use core::mem::size_of;
use core::ptr;

// Constants
pub const ROOT_INODE: u32 = 0;
pub const INODE_TYPE_DIR: u16 = 1;
pub const INODE_TYPE_DATA: u16 = 2;

// Index layout
pub const INODE_INDEX_1: usize = 10; // Direct
pub const INODE_INDEX_2: usize = 12; // +2 Indirect Level 1
pub const INODE_INDEX_3: usize = 13; // +1 Indirect Level 2
pub const NINDIRECT: usize = BLOCK_SIZE / 4;
pub const MAXLEN_FILENAME: usize = 60;

// Disk Structures
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InodeDisk {
    pub type_: u16,
    pub major: u16,
    pub minor: u16,
    pub nlink: u16,
    pub size: u32,
    pub index: [u32; INODE_INDEX_3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DentryDisk {
    pub name: [u8; MAXLEN_FILENAME],
    pub inode_num: u32,
}

// Memory Structure
pub struct Inode {
    pub disk: InodeDisk,
    pub valid: bool,
    pub inode_num: u32,
    pub refcnt: u32,
    pub lock: Mutex<()>,
}

impl Inode {
    pub const fn new() -> Self {
        Self {
            disk: InodeDisk {
                type_: 0,
                major: 0,
                minor: 0,
                nlink: 0,
                size: 0,
                index: [0; INODE_INDEX_3],
            },
            valid: false,
            inode_num: 0,
            refcnt: 0,
            lock: Mutex::new(()),
        }
    }
}

pub const N_INODE: usize = 50;

pub struct InodeCache {
    pub inodes: [Inode; N_INODE],
}

pub static INODE_CACHE: Mutex<InodeCache> = Mutex::new(InodeCache {
    inodes: [const { Inode::new() }; N_INODE],
});

fn locate_or_add_block(inode: &mut Inode, mut lbn: u32, grow: bool) -> Option<u32> {
    // 1. Direct Blocks
    if (lbn as usize) < INODE_INDEX_1 {
        let idx = lbn as usize;
        let mut blk = inode.disk.index[idx];
        if blk == 0 {
            if !grow {
                return None;
            }
            blk = bitmap::alloc();
            inode.disk.index[idx] = blk;
            inode_rw(inode, true);
        }
        return Some(blk);
    }
    lbn -= INODE_INDEX_1 as u32;

    // 2. Indirect Level 1
    if (lbn as usize) < NINDIRECT {
        let idx = INODE_INDEX_1;
        let mut indirect_blk = inode.disk.index[idx];
        if indirect_blk == 0 {
            if !grow {
                return None;
            }
            indirect_blk = bitmap::alloc();
            inode.disk.index[idx] = indirect_blk;
            inode_rw(inode, true);
        }

        let b = buffer::read(0, indirect_blk);
        let data = buffer::get_data_ptr(b) as *mut u32;
        let mut blk = unsafe { *data.add(lbn as usize) };
        if blk == 0 {
            if !grow {
                buffer::release(b);
                return None;
            }
            blk = bitmap::alloc();
            unsafe { *data.add(lbn as usize) = blk };
            buffer::write(b);
        }
        buffer::release(b);
        return Some(blk);
    }
    lbn -= NINDIRECT as u32;

    // 3. Indirect Level 2
    if (lbn as usize) < NINDIRECT * NINDIRECT {
        let idx = INODE_INDEX_2;
        let mut l1_blk = inode.disk.index[idx];
        if l1_blk == 0 {
            if !grow {
                return None;
            }
            l1_blk = bitmap::alloc();
            inode.disk.index[idx] = l1_blk;
            inode_rw(inode, true);
        }

        let l1_idx = (lbn as usize) / NINDIRECT;
        let l2_idx = (lbn as usize) % NINDIRECT;

        let b_l1 = buffer::read(0, l1_blk);
        let data_l1 = buffer::get_data_ptr(b_l1) as *mut u32;
        let mut l2_blk = unsafe { *data_l1.add(l1_idx) };

        if l2_blk == 0 {
            if !grow {
                buffer::release(b_l1);
                return None;
            }
            l2_blk = bitmap::alloc();
            unsafe { *data_l1.add(l1_idx) = l2_blk };
            buffer::write(b_l1);
        }
        buffer::release(b_l1);

        let b_l2 = buffer::read(0, l2_blk);
        let data_l2 = buffer::get_data_ptr(b_l2) as *mut u32;
        let mut blk = unsafe { *data_l2.add(l2_idx) };

        if blk == 0 {
            if !grow {
                buffer::release(b_l2);
                return None;
            }
            blk = bitmap::alloc();
            unsafe { *data_l2.add(l2_idx) = blk };
            buffer::write(b_l2);
        }
        buffer::release(b_l2);
        return Some(blk);
    }

    panic!("locate_or_add_block: block index out of range");
}

fn free_data_blocks(inode: &mut Inode) {
    // 1. Direct Blocks
    for i in 0..INODE_INDEX_1 {
        if inode.disk.index[i] != 0 {
            bitmap::free(inode.disk.index[i]);
            inode.disk.index[i] = 0;
        }
    }

    // 2. Indirect Level 1
    if inode.disk.index[INODE_INDEX_1] != 0 {
        let indirect_blk = inode.disk.index[INODE_INDEX_1];
        let b = buffer::read(0, indirect_blk);
        let data = buffer::get_data_ptr(b) as *const u32;
        for i in 0..NINDIRECT {
            let blk = unsafe { *data.add(i) };
            if blk != 0 {
                bitmap::free(blk);
            }
        }
        buffer::release(b);
        bitmap::free(indirect_blk);
        inode.disk.index[INODE_INDEX_1] = 0;
    }

    // 3. Indirect Level 2
    if inode.disk.index[INODE_INDEX_2] != 0 {
        let l1_blk = inode.disk.index[INODE_INDEX_2];
        let b_l1 = buffer::read(0, l1_blk);
        let data_l1 = buffer::get_data_ptr(b_l1) as *const u32;

        for i in 0..NINDIRECT {
            let l2_blk = unsafe { *data_l1.add(i) };
            if l2_blk != 0 {
                let b_l2 = buffer::read(0, l2_blk);
                let data_l2 = buffer::get_data_ptr(b_l2) as *const u32;
                for j in 0..NINDIRECT {
                    let blk = unsafe { *data_l2.add(j) };
                    if blk != 0 {
                        bitmap::free(blk);
                    }
                }
                buffer::release(b_l2);
                bitmap::free(l2_blk);
            }
        }
        buffer::release(b_l1);
        bitmap::free(l1_blk);
        inode.disk.index[INODE_INDEX_2] = 0;
    }
}

pub fn inode_init() {
    let _cache = INODE_CACHE.lock();
    printk!("Inode cache initialized with {} inodes\n", N_INODE);
}


pub fn inode_rw(inode: &mut Inode, write: bool) {
    let sb = get_sb();
    let ipb = (BLOCK_SIZE / size_of::<InodeDisk>()) as u32; // Inodes per block
    let block = sb.inode_start + (inode.inode_num / ipb);
    let offset = (inode.inode_num % ipb) as usize * size_of::<InodeDisk>();

    let b = buffer::read(0, block); // Assuming dev 0 for now
    let data_ptr = buffer::get_data_ptr(b);

    unsafe {
        if write {
            // Copy from Inode to disk buffer
            let inode_disk_ptr = &inode.disk as *const InodeDisk;
            ptr::copy_nonoverlapping(inode_disk_ptr, (data_ptr as *mut u8).add(offset) as *mut InodeDisk, 1);
            buffer::write(b);
        } else {
            // Copy from disk buffer to Inode
            let inode_disk_ptr = &mut inode.disk as *mut InodeDisk;
            ptr::copy_nonoverlapping((data_ptr as *const u8).add(offset) as *const InodeDisk, inode_disk_ptr, 1);
        }
    }
    buffer::release(b);
}


pub fn inode_get(inum: u32) -> &'static mut Inode {
    let mut cache_guard = INODE_CACHE.lock();

    // Search active cache for inode
    for i in 0..N_INODE {
        let inode = unsafe { &mut *(&raw mut cache_guard.inodes[i] as *mut Inode) };
        if inode.refcnt > 0 && inode.inode_num == inum && inode.valid {
            // Found in cache, increment refcnt
            inode.refcnt += 1;
            drop(cache_guard); // Release global cache lock
            return inode;
        }
    }

    // Not in cache, find a free slot (refcnt == 0)
    for i in 0..N_INODE {
        let inode = unsafe { &mut *(&raw mut cache_guard.inodes[i] as *mut Inode) };
        if inode.refcnt == 0 {
            // Found a free slot
            inode.inode_num = inum;
            inode.valid = false; // Mark as invalid until data is read
            inode.refcnt = 1;

            drop(cache_guard); // Release global cache lock

            // Read InodeDisk from disk into inode.disk
            inode_rw(inode, false);
            inode.valid = true; // Mark as valid after reading
            return inode;
        }
    }

    // No free slot found
    drop(cache_guard);
    panic!("inode_get: no free inode in cache");
}

pub fn inode_dup(inode: &mut Inode) {
    let _guard = inode.lock.lock(); // Acquire individual inode lock
    inode.refcnt += 1;
    // _guard is dropped here, releasing the lock
}

pub fn inode_put(inode: &mut Inode) {
    let guard = inode.lock.lock(); // Acquire individual inode lock
    if inode.refcnt == 0 {
        panic!("inode_put: refcnt is already zero for inode {}", inode.inode_num);
    }
    inode.refcnt -= 1;
    let should_delete = inode.refcnt == 0 && inode.disk.nlink == 0;
    drop(guard); // Release lock before potential deletion logic

    if should_delete {
        // Fully implemented inode_delete logic
        // Preserve current inode number during on-disk clear.
        let current_inum = inode.inode_num;

        free_data_blocks(inode); // Free all data blocks
        free(current_inum); // Free the inode bitmap entry

        // Clear on-disk inode content at the correct slot
        inode.disk.size = 0;
        inode.disk.type_ = 0;
        inode_rw(inode, true);

        // Invalidate cache entry afterwards
        inode.valid = false;
        inode.inode_num = 0;
    }
}

pub fn inode_read_data(inode: &mut Inode, off: u32, len: u32, dst: &mut [u8]) -> u32 {
    let mut off = off;
    let mut len = len;
    let mut dst_off = 0;

    if off >= inode.disk.size {
        return 0;
    }
    if off + len > inode.disk.size {
        len = inode.disk.size - off;
    }

    let end = off + len;
    while off < end {
        let lbn = off / BLOCK_SIZE as u32;
        let off_in_block = (off % BLOCK_SIZE as u32) as usize;
        let mut copy_len = BLOCK_SIZE - off_in_block;
        if off + copy_len as u32 > end {
            copy_len = (end - off) as usize;
        }

        match locate_or_add_block(inode, lbn, false) {
            Some(block_no) => {
                let b = buffer::read(0, block_no);
                let data = buffer::get_data_ptr(b);
                unsafe {
                    ptr::copy_nonoverlapping(
                        data.add(off_in_block),
                        dst.as_mut_ptr().add(dst_off),
                        copy_len,
                    );
                }
                buffer::release(b);
            }
            None => {
                // Sparse file hole, fill with zeros
                unsafe {
                    ptr::write_bytes(dst.as_mut_ptr().add(dst_off), 0, copy_len);
                }
            }
        }

        off += copy_len as u32;
        dst_off += copy_len;
    }

    len
}

pub fn inode_write_data(inode: &mut Inode, off: u32, len: u32, src: &[u8]) -> u32 {
    let mut off = off;
    let end = off + len;
    let mut src_off = 0;

    // TODO: Check max file size limit if necessary

    while off < end {
        let lbn = off / BLOCK_SIZE as u32;
        let off_in_block = (off % BLOCK_SIZE as u32) as usize;
        let mut copy_len = BLOCK_SIZE - off_in_block;
        if off + copy_len as u32 > end {
            copy_len = (end - off) as usize;
        }

        let block_no = locate_or_add_block(inode, lbn, true).expect("inode_write_data: out of blocks");

        let b = buffer::read(0, block_no);
        let data = buffer::get_data_ptr(b);

        unsafe {
            ptr::copy_nonoverlapping(
                src.as_ptr().add(src_off),
                data.add(off_in_block),
                copy_len,
            );
        }

        buffer::write(b);
        buffer::release(b);

        off += copy_len as u32;
        src_off += copy_len;
    }

    if end > inode.disk.size {
        inode.disk.size = end;
        inode_rw(inode, true);
    }

    len
}

pub fn inode_create(type_: u16, major: u16, minor: u16) -> &'static mut Inode {
    let inum = alloc(); // Allocate a new inode number
    let inode = inode_get(inum); // Get the inode from cache or disk

    // Acquire individual inode lock
    // This guard ensures that 'inode' is exclusively accessed during initialization.
    let guard = inode.lock.lock();

    inode.disk.type_ = type_;
    inode.disk.major = major;
    inode.disk.minor = minor;
    inode.disk.nlink = 1;
    inode.disk.size = 0;
    // Initialize index array to zeros
    for i in 0..INODE_INDEX_3 {
        inode.disk.index[i] = 0;
    }

    drop(guard); // Explicitly drop the guard here to release the lock on 'inode'

    inode_rw(inode, true); // Write the initialized inode to disk

    inode
}

pub fn inode_print(inode: &Inode, tag: &str) {
    printk!(
        "[{}] Inode {} (ref: {}, valid: {}): type={}, major={}, minor={}, nlink={}, size={}, index={:?}\n",
        tag,
        inode.inode_num,
        inode.refcnt,
        inode.valid,
        inode.disk.type_,
        inode.disk.major,
        inode.disk.minor,
        inode.disk.nlink,
        inode.disk.size,
        inode.disk.index
    );
}

pub fn alloc() -> u32 {    let sb = get_sb();
    let ibmap_block = sb.inode_start - 1;

    let b = buffer::read(0, ibmap_block);
    let data = buffer::get_data_ptr(b);

    let total_inodes = sb.ninodes;

    for i in 0..BLOCK_SIZE {
        let byte = unsafe { *data.add(i) };
        if byte != 0xFF {
            for j in 0..8 {
                if (byte & (1 << j)) == 0 {
                    let bit_idx = i * 8 + j;
                    if bit_idx as u32 >= total_inodes {
                        buffer::release(b);
                        panic!("inode_alloc: out of inodes");
                    }

                    unsafe {
                        *data.add(i) |= 1 << j;
                    }
                    buffer::write(b);
                    buffer::release(b);

                    return bit_idx as u32;
                }
            }
        }
    }
    buffer::release(b);
    panic!("inode_alloc: out of inodes");
}

pub fn free(inode_idx: u32) {
    let sb = get_sb();
    let ibmap_block = sb.inode_start - 1;

    let b = buffer::read(0, ibmap_block);
    let data = buffer::get_data_ptr(b);

    let byte_idx = (inode_idx / 8) as usize;
    let bit = (inode_idx % 8) as u8;

    unsafe {
        if (*data.add(byte_idx) & (1 << bit)) == 0 {
            panic!("inode_free: inode already free");
        }
        *data.add(byte_idx) &= !(1 << bit);
    }
    buffer::write(b);
    buffer::release(b);
}
