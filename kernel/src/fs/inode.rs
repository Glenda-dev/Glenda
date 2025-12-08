use crate::fs::buffer;
use crate::fs::buffer::BLOCK_SIZE;
use crate::fs::fs::get_sb;

pub fn alloc() -> u32 {
    let sb = get_sb();
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
