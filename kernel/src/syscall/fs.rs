use crate::fs::{bitmap, buffer, inode, dentry, path};
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

pub fn sys_inode_create(ctx: &mut TrapContext) -> usize {
    let type_ = (ctx.a0 & 0xFFFF) as u16;
    let major = (ctx.a1 & 0xFFFF) as u16;
    let minor = (ctx.a2 & 0xFFFF) as u16;
    let inode_ref = inode::inode_create(type_, major, minor);
    inode_ref.inode_num as usize
}

pub fn sys_inode_dup(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let inode_ref = inode::inode_get(inum);
    inode::inode_dup(inode_ref);
    let rc = inode_ref.refcnt as usize;
    inode::inode_put(inode_ref);
    rc
}

pub fn sys_inode_put(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let inode_ref = inode::inode_get(inum);
    inode::inode_put(inode_ref);
    0
}

pub fn sys_inode_set_nlink(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let nlink = (ctx.a1 & 0xFFFF) as u16;
    let inode_ref = inode::inode_get(inum);
    inode_ref.disk.nlink = nlink;
    inode::inode_rw(inode_ref, true);
    inode::inode_put(inode_ref);
    0
}

pub fn sys_inode_get_refcnt(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let cache = inode::INODE_CACHE.lock();
    for i in 0..inode::N_INODE {
        let inode_ref = unsafe { &*(&cache.inodes[i] as *const inode::Inode) };
        if inode_ref.refcnt > 0 && inode_ref.inode_num == inum {
            return inode_ref.refcnt as usize;
        }
    }
    0
}

pub fn sys_inode_print(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let inode_ref = inode::inode_get(inum);
    inode::inode_print(inode_ref, "C");
    inode::inode_put(inode_ref);
    0
}

pub fn sys_inode_write_data(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let off = ctx.a1 as u32;
    let u_src = ctx.a2;
    let len = ctx.a3 as usize;

    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    let inode_ref = inode::inode_get(inum);
    let mut total_written = 0;
    let mut buf = [0u8; 512];
    while total_written < len {
        let chunk_len = core::cmp::min(len - total_written, buf.len());
        if let Err(_) = uvm::copyin(pt, &mut buf[..chunk_len], u_src + total_written) {
            inode::inode_put(inode_ref);
            return if total_written > 0 { total_written } else { usize::MAX };
        }
        let written = inode::inode_write_data(inode_ref, off + total_written as u32, chunk_len as u32, &buf[..chunk_len]);
        total_written += written as usize;
        if written < chunk_len as u32 { break; }
    }
    inode::inode_put(inode_ref);
    total_written
}

pub fn sys_inode_read_data(ctx: &mut TrapContext) -> usize {
    let inum = ctx.a0 as u32;
    let off = ctx.a1 as u32;
    let u_dst = ctx.a2;
    let len = ctx.a3 as usize;

    let inode_ref = inode::inode_get(inum);
    let mut total_read = 0;
    let mut buf = [0u8; 512];
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    while total_read < len {
        let chunk_len = core::cmp::min(len - total_read, buf.len());
        let read = inode::inode_read_data(inode_ref, off + total_read as u32, chunk_len as u32, &mut buf[..chunk_len]);
        if read == 0 { break; }
        if let Err(_) = uvm::copyout(pt, u_dst + total_read, &buf[..read as usize]) {
            inode::inode_put(inode_ref);
            return if total_read > 0 { total_read } else { usize::MAX };
        }
        total_read += read as usize;
        if read < chunk_len as u32 { break; }
    }
    inode::inode_put(inode_ref);
    total_read
}

pub fn sys_dentry_create(ctx: &mut TrapContext) -> usize {
    let dir_inum = ctx.a0 as u32;
    let target_inum = ctx.a1 as u32;
    let u_name = ctx.a2;

    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut name_buf = [0u8; inode::MAXLEN_FILENAME + 1];
    let copied = match uvm::copyin_str(pt, &mut name_buf, u_name) {
        Ok(n) => n,
        Err(_) => return usize::MAX,
    };
    let name_len = copied.saturating_sub(1).min(inode::MAXLEN_FILENAME);

    let dir = inode::inode_get(dir_inum);
    let ret = dentry::dentry_create(dir, target_inum, &name_buf[..name_len]);
    inode::inode_put(dir);
    if ret < 0 {
        crate::printk!("sys_dentry_create failed: ret={}\n", ret);
        usize::MAX
    } else {
        0
    }
}

pub fn sys_dentry_search(ctx: &mut TrapContext) -> usize {
    let dir_inum = ctx.a0 as u32;
    let u_name = ctx.a1;

    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut name_buf = [0u8; inode::MAXLEN_FILENAME + 1];
    let copied = match uvm::copyin_str(pt, &mut name_buf, u_name) {
        Ok(n) => n,
        Err(_) => return usize::MAX,
    };
    let name_len = copied.saturating_sub(1).min(inode::MAXLEN_FILENAME);

    let dir = inode::inode_get(dir_inum);
    let ret = dentry::dentry_search(dir, &name_buf[..name_len]);
    inode::inode_put(dir);
    match ret { Some(inum) => inum as usize, None => usize::MAX }
}

pub fn sys_dentry_delete(ctx: &mut TrapContext) -> usize {
    let dir_inum = ctx.a0 as u32;
    let u_name = ctx.a1;

    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut name_buf = [0u8; inode::MAXLEN_FILENAME + 1];
    let copied = match uvm::copyin_str(pt, &mut name_buf, u_name) {
        Ok(n) => n,
        Err(_) => return usize::MAX,
    };
    let name_len = copied.saturating_sub(1).min(inode::MAXLEN_FILENAME);

    let dir = inode::inode_get(dir_inum);
    let ret = dentry::dentry_delete(dir, &name_buf[..name_len]);
    inode::inode_put(dir);
    if ret < 0 { usize::MAX } else { ret as usize }
}

pub fn sys_dentry_print(ctx: &mut TrapContext) -> usize {
    let dir_inum = ctx.a0 as u32;
    let dir = inode::inode_get(dir_inum);
    dentry::dentry_print(dir);
    inode::inode_put(dir);
    0
}

pub fn sys_path_to_inode(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut path_buf = [0u8; 256];
    let copied = match uvm::copyin_str(pt, &mut path_buf, u_path) {
        Ok(n) => n,
        Err(_) => return usize::MAX,
    };
    let path_len = copied.saturating_sub(1).min(255);
    crate::printk!("sys_path_to_inode: path='{}'\n", core::str::from_utf8(&path_buf[..path_len]).unwrap_or("???"));
    let res = path::path_to_inode(&path_buf[..path_len]);
    match res {
        Some(inode_ref) => {
            let inum = inode_ref.inode_num as usize;
            inode::inode_put(inode_ref);
            inum
        }
        None => {
            crate::printk!("sys_path_to_inode: failed to resolve\n");
            usize::MAX
        }
    }
}

pub fn sys_path_to_parent_inode(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let u_name_out = ctx.a1;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut path_buf = [0u8; 256];
    let copied = match uvm::copyin_str(pt, &mut path_buf, u_path) {
        Ok(n) => n,
        Err(_) => return usize::MAX,
    };
    let path_len = copied.saturating_sub(1).min(255);
    let mut name_buf = [0u8; inode::MAXLEN_FILENAME];
    let res = path::path_to_parent_inode(&path_buf[..path_len], &mut name_buf);
    match res {
        Some(parent) => {
            // Copy out name
            if let Err(_) = uvm::copyout(pt, u_name_out, &name_buf) { return usize::MAX; }
            let inum = parent.inode_num as usize;
            inode::inode_put(parent);
            inum
        }
        None => usize::MAX,
    }
}

pub fn sys_prepare_root_dir() -> usize {
    // Mirror the logic used in fs_test to ensure root inode is present and sane.
    let is_inum_set = |inum: u32| -> bool {
        let sb = crate::fs::fs::get_sb();
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
        if root_inum != inode::ROOT_INODE { return usize::MAX; }
        let root_init = inode::inode_get(inode::ROOT_INODE);
        root_init.disk.type_ = inode::INODE_TYPE_DIR;
        root_init.disk.nlink = 2;
        root_init.disk.size = 0;
        inode::inode_rw(root_init, true);
        inode::inode_put(root_init);
    } else {
        let root_init = inode::inode_get(inode::ROOT_INODE);
        let mut changed = false;
        if root_init.disk.type_ == 0 { root_init.disk.type_ = inode::INODE_TYPE_DIR; changed = true; }
        if root_init.disk.nlink == 0 { root_init.disk.nlink = 2; changed = true; }
        if changed { inode::inode_rw(root_init, true); }
        inode::inode_put(root_init);
    }
    0
}
