#![allow(dead_code)]

use crate::fs::buffer::{bread, brelse, bwrite, get_data_ptr, BLOCK_SIZE};
use crate::fs::fs::get_sb;

// Allocate a block from the data bitmap
pub fn balloc() -> u32 {
    let sb = get_sb();
    let bmap_start = sb.bmap_start;

    let b = bread(0, bmap_start);
    let data = get_data_ptr(b);

    let total_data_blocks = sb.nblocks;

    for i in 0..BLOCK_SIZE {
        let byte = unsafe { *data.add(i) };
        if byte != 0xFF {
            for j in 0..8 {
                if (byte & (1 << j)) == 0 {
                    // Found free bit
                    let bit_idx = i * 8 + j;
                    if bit_idx as u32 >= total_data_blocks {
                        brelse(b);
                        panic!("balloc: out of blocks");
                    }

                    unsafe {
                        *data.add(i) |= 1 << j;
                    }

                    bwrite(b);
                    brelse(b);

                    // Zero the allocated block
                    let data_start = bmap_start + 1;
                    let abs_block = data_start + bit_idx as u32;

                    let zero_buf = bread(0, abs_block);
                    let zero_ptr = get_data_ptr(zero_buf);
                    unsafe { core::ptr::write_bytes(zero_ptr, 0, BLOCK_SIZE); }
                    bwrite(zero_buf);
                    brelse(zero_buf);

                    return abs_block;
                }
            }
        }
    }

    brelse(b);
    panic!("balloc: out of blocks");
}

pub fn bfree(block_no: u32) {
    let sb = get_sb();
    let bmap_start = sb.bmap_start;
    let data_start = bmap_start + 1;

    if block_no < data_start || block_no >= data_start + sb.nblocks {
        panic!("bfree: block out of data range");
    }

    let bit_idx = (block_no - data_start) as usize;

    let b = bread(0, bmap_start);
    let data = get_data_ptr(b);

    let byte_idx = bit_idx / 8;
    let bit = bit_idx % 8;

    unsafe {
        let val = *data.add(byte_idx);
        if (val & (1 << bit)) == 0 {
            panic!("bfree: block already free");
        }
        *data.add(byte_idx) &= !(1 << bit);
    }

    bwrite(b);
    brelse(b);
}

pub fn ialloc() -> u32 {
    let sb = get_sb();
    let ibmap_block = sb.inode_start - 1;

    let b = bread(0, ibmap_block);
    let data = get_data_ptr(b);

    let total_inodes = sb.ninodes;

    for i in 0..BLOCK_SIZE {
        let byte = unsafe { *data.add(i) };
        if byte != 0xFF {
            for j in 0..8 {
                if (byte & (1 << j)) == 0 {
                    let bit_idx = i * 8 + j;
                    if bit_idx as u32 >= total_inodes {
                        brelse(b);
                        panic!("ialloc: out of inodes");
                    }

                    unsafe {
                        *data.add(i) |= 1 << j;
                    }
                    bwrite(b);
                    brelse(b);

                    return bit_idx as u32;
                }
            }
        }
    }
    brelse(b);
    panic!("ialloc: out of inodes");
}

pub fn ifree(inode_idx: u32) {
    let sb = get_sb();
    let ibmap_block = sb.inode_start - 1;

    let b = bread(0, ibmap_block);
    let data = get_data_ptr(b);

    let byte_idx = (inode_idx / 8) as usize;
    let bit = (inode_idx % 8) as u8;

    unsafe {
        if (*data.add(byte_idx) & (1 << bit)) == 0 {
            panic!("ifree: inode already free");
        }
        *data.add(byte_idx) &= !(1 << bit);
    }
    bwrite(b);
    brelse(b);
}
