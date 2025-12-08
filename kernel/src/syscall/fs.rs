use crate::fs::{bitmap, buffer, inode};
use crate::irq::TrapContext;
use crate::mem::{PageTable, uvm};
use crate::proc::current_proc;

pub fn sys_alloc_block() -> usize {
    bitmap::alloc() as usize
}

pub fn sys_free_block(ctx: &mut TrapContext) -> usize {
    let block_no = ctx.a0 as u32;
    bitmap::free(block_no);
    0
}

pub fn sys_alloc_inode() -> usize {
    inode::alloc() as usize
}

pub fn sys_free_inode(ctx: &mut TrapContext) -> usize {
    let inode_idx = ctx.a0 as u32;
    inode::free(inode_idx);
    0
}

pub fn sys_show_bitmap(ctx: &mut TrapContext) -> usize {
    let which = ctx.a0;
    // TODO: Implement bitmap dump logic if needed for debugging
    0
}

pub fn sys_get_block(ctx: &mut TrapContext) -> usize {
    let block_no = ctx.a0 as u32;
    buffer::read(0, block_no) // return buffer index
}

pub fn sys_read_block(ctx: &mut TrapContext) -> usize {
    let buf_idx = ctx.a0;
    let u_dst = ctx.a1;

    // Copy buffer data to user
    let data_ptr = buffer::get_data_ptr(buf_idx);
    let data_slice = unsafe { core::slice::from_raw_parts(data_ptr, buffer::BLOCK_SIZE) };

    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    match uvm::copyout(pt, u_dst, data_slice) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_write_block(ctx: &mut TrapContext) -> usize {
    let buf_idx = ctx.a0;
    let u_src = ctx.a1;

    let data_ptr = buffer::get_data_ptr(buf_idx);
    let data_slice = unsafe { core::slice::from_raw_parts_mut(data_ptr, buffer::BLOCK_SIZE) };

    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    match uvm::copyin(pt, data_slice, u_src) {
        Ok(_) => {
            buffer::write(buf_idx);
            0
        }
        Err(_) => usize::MAX,
    }
}

pub fn sys_put_block(ctx: &mut TrapContext) -> usize {
    let buf_idx = ctx.a0;
    buffer::release(buf_idx);
    0
}

pub fn sys_show_buffer() -> usize {
    buffer::debug_state();
    0
}

pub fn sys_flush_buffer(ctx: &mut TrapContext) -> usize {
    let _count = ctx.a0;
    // In current static allocation model, we don't really free memory.
    // This hook is kept for compatibility with tests.
    0
}
