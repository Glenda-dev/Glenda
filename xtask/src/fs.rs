use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

pub fn mkfs() -> anyhow::Result<()> {
    // Parameters
    const BLOCK_SIZE: usize = 4096;
    const N_INODES: usize = 200;
    const N_DATA_BLOCKS: usize = 1000;
    const MAGIC: u32 = 0x10203040;

    // Sizes
    let sb_size = 1;
    let inode_bitmap_size = 1;

    // Inode size 64 bytes
    const IPB: usize = BLOCK_SIZE / 64;
    let inode_blocks = (N_INODES + IPB - 1) / IPB;

    let data_bitmap_size = 1;

    let total_blocks =
        sb_size + inode_bitmap_size + inode_blocks + data_bitmap_size + N_DATA_BLOCKS;

    let inode_region_start = sb_size + inode_bitmap_size;
    let data_bitmap_start = inode_region_start + inode_blocks;

    println!(
        "[ INFO ] Generating disk.img (Size: {} blocks / {} bytes)",
        total_blocks,
        total_blocks * BLOCK_SIZE
    );
    println!(
        "[ INFO ] Layout: SB:0, IBMap:1, IRegions:{}-{}, DBMap:{}, Data:{}...",
        inode_region_start,
        inode_region_start + inode_blocks - 1,
        data_bitmap_start,
        data_bitmap_start + 1
    );

    let mut file = File::create("disk.img")?;

    file.set_len((total_blocks * BLOCK_SIZE) as u64)?;

    let mut sb_buf = [0u8; BLOCK_SIZE];
    let magic_bytes = MAGIC.to_le_bytes();
    let size_bytes = (total_blocks as u32).to_le_bytes();
    let nblocks_bytes = (N_DATA_BLOCKS as u32).to_le_bytes();
    let ninodes_bytes = (N_INODES as u32).to_le_bytes();
    let inode_start_bytes = (inode_region_start as u32).to_le_bytes();
    let bmap_start_bytes = (data_bitmap_start as u32).to_le_bytes();

    sb_buf[0..4].copy_from_slice(&magic_bytes);
    sb_buf[4..8].copy_from_slice(&size_bytes);
    sb_buf[8..12].copy_from_slice(&nblocks_bytes);
    sb_buf[12..16].copy_from_slice(&ninodes_bytes);
    sb_buf[16..20].copy_from_slice(&inode_start_bytes);
    sb_buf[20..24].copy_from_slice(&bmap_start_bytes);

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&sb_buf)?;

    Ok(())
}
