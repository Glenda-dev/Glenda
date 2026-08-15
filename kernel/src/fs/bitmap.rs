#![allow(dead_code)]

use crate::fs::buffer;
use crate::fs::buffer::BLOCK_SIZE;
use crate::fs::fs::get_sb;

// Allocate a block from the data bitmap
pub fn alloc() -> u32 {
    let sb = get_sb();
    let bmap_start = sb.bmap_start;

    let b = buffer::read(0, bmap_start);
    let data = buffer::get_data_ptr(b);

    let total_data_blocks = sb.nblocks;

    for i in 0..BLOCK_SIZE {
        let byte = unsafe { *data.add(i) };
        if byte != 0xFF {
            for j in 0..8 {
                if (byte & (1 << j)) == 0 {
                    // Found free bit
                    let bit_idx = i * 8 + j;
                    if bit_idx as u32 >= total_data_blocks {
                        buffer::release(b);
                        panic!("buffer_alloc: out of blocks");
                    }

                    unsafe {
                        *data.add(i) |= 1 << j;
                    }

                    buffer::write(b);
                    buffer::release(b);

                    // Zero the allocated block
                    let data_start = bmap_start + 1;
                    let abs_block = data_start + bit_idx as u32;

                    let zero_buf = buffer::read(0, abs_block);
                    let zero_ptr = buffer::get_data_ptr(zero_buf);
                    unsafe {
                        core::ptr::write_bytes(zero_ptr, 0, BLOCK_SIZE);
                    }
                    buffer::write(zero_buf);
                    buffer::release(zero_buf);

                    return abs_block;
                }
            }
        }
    }

    buffer::release(b);
    panic!("buffer_alloc: out of blocks");
}

pub fn free(block_no: u32) {
    let sb = get_sb();
    let bmap_start = sb.bmap_start;
    let data_start = bmap_start + 1;

    if block_no < data_start || block_no >= data_start + sb.nblocks {
        panic!("buffer_free: block out of data range");
    }

    let bit_idx = (block_no - data_start) as usize;

    let b = buffer::read(0, bmap_start);
    let data = buffer::get_data_ptr(b);

    let byte_idx = bit_idx / 8;
    let bit = bit_idx % 8;

    unsafe {
        let val = *data.add(byte_idx);
        if (val & (1 << bit)) == 0 {
            panic!("buffer_free: block already free");
        }
        *data.add(byte_idx) &= !(1 << bit);
    }

    buffer::write(b);
    buffer::release(b);
}
