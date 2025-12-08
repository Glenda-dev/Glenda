#![allow(dead_code)]

use crate::fs::buffer::{bread, brelse, get_data_ptr};
use crate::printk;
use core::ptr::{self, addr_of, addr_of_mut};

// Filesystem constants
pub const MAGIC: u32 = 0x10203040;
pub const BSIZE: usize = 4096; // Block size = Page size

#[repr(C)]
pub struct SuperBlock {
    pub magic: u32,
    pub size: u32,
    pub nblocks: u32,
    pub ninodes: u32,
    pub inode_start: u32,
    pub bmap_start: u32,
}

static mut SB: SuperBlock = SuperBlock {
    magic: 0,
    size: 0,
    nblocks: 0,
    ninodes: 0,
    inode_start: 0,
    bmap_start: 0,
};

pub fn fs_init() {
    // Read superblock (block 0)
    let b = bread(0, 0);
    let data = get_data_ptr(b);

    unsafe {
        ptr::copy_nonoverlapping(data as *const SuperBlock, addr_of_mut!(SB), 1);
    }

    brelse(b);

    unsafe {
        if (*addr_of!(SB)).magic != MAGIC {
            panic!("fs_init: invalid file system magic {:#x} (expected {:#x})", (*addr_of!(SB)).magic, MAGIC);
        }
        printk!("fs: superblock read. size={} blocks, inodes={}, bmap_start={}",
            (*addr_of!(SB)).size, (*addr_of!(SB)).ninodes, (*addr_of!(SB)).bmap_start);
    }
}

pub fn get_sb() -> &'static SuperBlock {
    unsafe { &*addr_of!(SB) }
}
