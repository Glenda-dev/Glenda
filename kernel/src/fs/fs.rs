#![allow(dead_code)]

use crate::fs::buffer;
use crate::fs::inode;
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

static mut SB: SuperBlock =
    SuperBlock { magic: 0, size: 0, nblocks: 0, ninodes: 0, inode_start: 0, bmap_start: 0 };

pub fn fs_init() {
    // Read superblock (block 0)
    let b = buffer::read(0, 0);
    let data = buffer::get_data_ptr(b);

    unsafe {
        ptr::copy_nonoverlapping(data as *const SuperBlock, addr_of_mut!(SB), 1);
    }

    buffer::release(b);

    unsafe {
        if (*addr_of!(SB)).magic != MAGIC {
            panic!(
                "fs_init: invalid file system magic {:#x} (expected {:#x})",
                (*addr_of!(SB)).magic,
                MAGIC
            );
        }
        printk!(
            "FS: Superblock read: size={} blocks, inodes={}, bmap_start={}\n",
            (*addr_of!(SB)).size,
            (*addr_of!(SB)).ninodes,
            (*addr_of!(SB)).bmap_start
        );
    }
    inode::inode_init();
    fs_test();
}

fn fs_test() {
    printk!("FS: Starting self-tests...\n");

    // Test 1: Inode allocation and manipulation
    printk!("Test 1: Inode alloc/free...\n");
    let inode = inode::inode_create(inode::INODE_TYPE_DATA, 0, 0);
    let inum = inode.inode_num;
    printk!("  Allocated inode {}\n", inum);
    inode::inode_print(inode, "Created");
    inode::inode_dup(inode);
    printk!("  Dup refcnt: {}\n", inode.refcnt); // Should be 2
    inode::inode_put(inode);
    printk!("  Put refcnt: {}\n", inode.refcnt); // Should be 1

    // Manually simulate unlink for deletion test
    inode.disk.nlink = 0;
    inode::inode_rw(inode, true);
    inode::inode_put(inode); // Should trigger free logic
    printk!("  Inode {} freed.\n", inum);

    // Test 2: Data R/W
    printk!("Test 2: Data R/W...\n");
    let inode = inode::inode_create(inode::INODE_TYPE_DATA, 0, 0);
    let mut buf = [0u8; 100];
    for i in 0..100 { buf[i] = i as u8; }
    inode::inode_write_data(inode, 0, 100, &buf);
    let mut read_buf = [0u8; 100];
    inode::inode_read_data(inode, 0, 100, &mut read_buf);
    for i in 0..100 {
        if read_buf[i] != buf[i] {
            panic!("Test 2 failed: byte {} mismatch", i);
        }
    }
    printk!("  Data R/W passed.\n");
    // Cleanup
    inode.disk.nlink = 0;
    inode::inode_rw(inode, true);
    inode::inode_put(inode);

    // Prepare Root Inode for Test 3 & 4
    // In minimal mkfs, inode 0 is free. In rich mkfs, inode 0 is already allocated.
    // Handle both cases gracefully.
    let is_inum_set = |inum: u32| -> bool {
        let sb = get_sb();
        let ibmap_block = sb.inode_start - 1;
        let b = buffer::read(0, ibmap_block);
        let data = buffer::get_data_ptr(b);
        let byte_idx = (inum / 8) as usize;
        let bit = (inum % 8) as u8;
        let val = unsafe { *data.add(byte_idx) };
        buffer::release(b);
        (val & (1 << bit)) != 0
    };

    if !is_inum_set(inode::ROOT_INODE) {
        let root_inum = inode::alloc();
        if root_inum != inode::ROOT_INODE {
            panic!("fs_test: expected to allocate inode 0 for root, got {}", root_inum);
        }
        let root_init = inode::inode_get(inode::ROOT_INODE);
        root_init.disk.type_ = inode::INODE_TYPE_DIR;
        root_init.disk.nlink = 2; // . and ..
        root_init.disk.size = 0;
        inode::inode_rw(root_init, true);
        inode::inode_put(root_init);
    } else {
        // If already allocated, ensure it's sane and not accidentally deletable
        let root_init = inode::inode_get(inode::ROOT_INODE);
        let mut changed = false;
        if root_init.disk.type_ == 0 {
            root_init.disk.type_ = inode::INODE_TYPE_DIR;
            changed = true;
        }
        if root_init.disk.nlink == 0 {
            // Ensure root is not deleted by inode_put
            root_init.disk.nlink = 2; // '.' and '..' minimal semantic
            changed = true;
        }
        if changed { inode::inode_rw(root_init, true); }
        inode::inode_put(root_init);
    }

    printk!("FS: All self-tests passed!\n");
}

pub fn get_sb() -> &'static SuperBlock {
    unsafe { &*addr_of!(SB) }
}
