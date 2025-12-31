use crate::fs::{bitmap, buffer, inode, dentry, path};
use crate::fs::file::{self, FileType, File};
use crate::fs::inode::{Inode, INODE_TYPE_DIR, INODE_TYPE_DATA};
use crate::irq::TrapContext;
use crate::mem::{PageTable, uvm};
use crate::proc::{current_proc, process::Process};

// --- Core Internal Interfaces (Step 4) ---

pub fn fs_open(p: &mut Process, path: &[u8], flags: u32) -> Result<usize, ()> {
    // flags: O_RDONLY=0, O_WRONLY=1, O_RDWR=2, O_CREAT=0x40, O_TRUNC=0x200
    let o_creat = (flags & 0x40) != 0;
    let o_trunc = (flags & 0x200) != 0;

    let inode_ref = if o_creat {
        let mut name = [0u8; inode::MAXLEN_FILENAME];
        match path::path_to_parent_inode_at(p.cwd, path, &mut name) {
            Some(parent) => {
                let name_len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
                if name_len == 0 {
                    inode::inode_put(parent);
                    return Err(());
                }

                // Check if exists
                match dentry::dentry_search(parent, &name[..name_len]) {
                    Some(inum) => {
                        inode::inode_put(parent);
                        inode::inode_get(inum)
                    }
                    None => {
                        let new_inode = inode::inode_create(INODE_TYPE_DATA, 0, 0);
                        dentry::dentry_create(parent, new_inode.inode_num, &name[..name_len]);
                        inode::inode_put(parent);
                        new_inode
                    }
                }
            }
            None => return Err(()),
        }
    } else {
        match path::path_to_inode_at(p.cwd, path) {
            Some(ip) => ip,
            None => return Err(()),
        }
    };

    if inode_ref.disk.type_ == INODE_TYPE_DIR && (flags & 3) != 0 {
        // Cannot open directory for writing
        inode::inode_put(inode_ref);
        return Err(());
    }

    if o_trunc && inode_ref.disk.type_ == INODE_TYPE_DATA {
        // inode::inode_trunc(inode_ref); // TODO: implement trunc
        inode_ref.disk.size = 0;
        inode::inode_rw(inode_ref, true);
    }

    let (f_idx, f) = file::file_alloc().ok_or(())?;
    f.ty = FileType::Inode;
    f.inum = inode_ref.inode_num;
    f.readable = (flags & 3) != 1; // Not WRONLY
    f.writable = (flags & 3) != 0; // Not RDONLY
    f.off = 0;

    // Find FD
    for fd in 0..crate::proc::process::NOFILE {
        if p.open_files[fd].is_none() {
            p.open_files[fd] = Some(f_idx);
            return Ok(fd);
        }
    }

    file::file_close(f_idx);
    Err(())
}

pub fn fs_close(p: &mut Process, fd: usize) -> Result<(), ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;
    file::file_close(f_idx);
    p.open_files[fd] = None;
    Ok(())
}

pub fn fs_read(p: &mut Process, fd: usize, u_dst: usize, len: usize) -> Result<usize, ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;

    // We need to mutate File.off, so we lock table or get ref
    let mut table = file::FILE_TABLE.lock();
    let f = &mut table.files[f_idx];
    if !f.readable { return Err(()); }

    let ip = inode::inode_get(f.inum);
    let mut total_read = 0;
    let mut buf = [0u8; 512];
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    while total_read < len {
        let chunk_len = core::cmp::min(len - total_read, buf.len());
        let read = inode::inode_read_data(ip, f.off, chunk_len as u32, &mut buf[..chunk_len]);
        if read == 0 { break; }
        if let Err(_) = uvm::copyout(pt, u_dst + total_read, &buf[..read as usize]) {
            inode::inode_put(ip);
            return Err(());
        }
        total_read += read as usize;
        f.off += read;
        if read < chunk_len as u32 { break; }
    }
    inode::inode_put(ip);
    Ok(total_read)
}

pub fn fs_write(p: &mut Process, fd: usize, u_src: usize, len: usize) -> Result<usize, ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;

    let mut table = file::FILE_TABLE.lock();
    let f = &mut table.files[f_idx];
    if !f.writable { return Err(()); }

    let ip = inode::inode_get(f.inum);
    let mut total_written = 0;
    let mut buf = [0u8; 512];
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    while total_written < len {
        let chunk_len = core::cmp::min(len - total_written, buf.len());
        if let Err(_) = uvm::copyin(pt, &mut buf[..chunk_len], u_src + total_written) {
            inode::inode_put(ip);
            return Err(());
        }
        let written = inode::inode_write_data(ip, f.off, chunk_len as u32, &buf[..chunk_len]);
        total_written += written as usize;
        f.off += written;
        if written < chunk_len as u32 { break; }
    }
    inode::inode_put(ip);
    Ok(total_written)
}

pub fn fs_lseek(p: &mut Process, fd: usize, off: i32, whence: u32) -> Result<usize, ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;
    let mut table = file::FILE_TABLE.lock();
    let f = &mut table.files[f_idx];

    let ip = inode::inode_get(f.inum);
    let size = ip.disk.size as i32;
    inode::inode_put(ip);

    let new_off = match whence {
        0 => off,              // SEEK_SET
        1 => f.off as i32 + off, // SEEK_CUR
        2 => size + off,       // SEEK_END
        _ => return Err(()),
    };

    if new_off < 0 { return Err(()); }
    f.off = new_off as u32;
    Ok(f.off as usize)
}

pub fn fs_dup(p: &mut Process, fd: usize) -> Result<usize, ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;

    // Increment global refcnt
    {
        let mut table = file::FILE_TABLE.lock();
        table.files[f_idx].refcnt += 1;
    }

    // Find new FD
    for new_fd in 0..crate::proc::process::NOFILE {
        if p.open_files[new_fd].is_none() {
            p.open_files[new_fd] = Some(f_idx);
            return Ok(new_fd);
        }
    }

    // Failed to find FD, revert refcnt
    {
        let mut table = file::FILE_TABLE.lock();
        table.files[f_idx].refcnt -= 1;
    }
    Err(())
}

pub fn fs_fstat(p: &mut Process, fd: usize, u_stat: usize) -> Result<(), ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;
    let f = {
        let table = file::FILE_TABLE.lock();
        let f = &table.files[f_idx];
        (f.inum, f.ty)
    };

    let ip = inode::inode_get(f.0);
    let stat = file::Stat {
        type_: ip.disk.type_,
        nlink: ip.disk.nlink,
        size: ip.disk.size,
        major: ip.disk.major,
        minor: ip.disk.minor,
        inum: ip.inode_num,
    };
    inode::inode_put(ip);

    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let src = unsafe {
        core::slice::from_raw_parts(&stat as *const file::Stat as *const u8, core::mem::size_of::<file::Stat>())
    };
    uvm::copyout(pt, u_stat, src).map_err(|_| ())
}

pub fn fs_mkdir(p: &mut Process, path: &[u8]) -> Result<(), ()> {
    let mut name = [0u8; inode::MAXLEN_FILENAME];
    match path::path_to_parent_inode_at(p.cwd, path, &mut name) {
        Some(parent) => {
            let name_len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            if name_len == 0 {
                inode::inode_put(parent);
                return Err(());
            }
            if dentry::dentry_search(parent, &name[..name_len]).is_some() {
                inode::inode_put(parent);
                return Err(());
            }
            let new_inode = inode::inode_create(INODE_TYPE_DIR, 0, 0);
            new_inode.disk.nlink = 2; // . and ..
            inode::inode_rw(new_inode, true);
            dentry::dentry_create(parent, new_inode.inode_num, &name[..name_len]);
            inode::inode_put(new_inode);
            inode::inode_put(parent);
            Ok(())
        }
        None => Err(()),
    }
}

pub fn fs_chdir(p: &mut Process, path: &[u8]) -> Result<(), ()> {
    match path::path_to_inode_at(p.cwd, path) {
        Some(ip) => {
            if ip.disk.type_ != INODE_TYPE_DIR {
                inode::inode_put(ip);
                return Err(());
            }
            p.cwd = ip.inode_num;
            inode::inode_put(ip);
            Ok(())
        }
        None => Err(()),
    }
}

pub fn fs_link(p: &mut Process, old_path: &[u8], new_path: &[u8]) -> Result<(), ()> {
    let old_ip = path::path_to_inode_at(p.cwd, old_path).ok_or(())?;
    if old_ip.disk.type_ == INODE_TYPE_DIR {
        inode::inode_put(old_ip);
        return Err(());
    }

    let mut name = [0u8; inode::MAXLEN_FILENAME];
    match path::path_to_parent_inode_at(p.cwd, new_path, &mut name) {
        Some(parent) => {
            let name_len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            if name_len == 0 || dentry::dentry_search(parent, &name[..name_len]).is_some() {
                inode::inode_put(parent);
                inode::inode_put(old_ip);
                return Err(());
            }
            dentry::dentry_create(parent, old_ip.inode_num, &name[..name_len]);
            old_ip.disk.nlink += 1;
            inode::inode_rw(old_ip, true);
            inode::inode_put(parent);
            inode::inode_put(old_ip);
            Ok(())
        }
        None => {
            inode::inode_put(old_ip);
            Err(())
        }
    }
}

pub fn fs_unlink(p: &mut Process, path: &[u8]) -> Result<(), ()> {
    let mut name = [0u8; inode::MAXLEN_FILENAME];
    match path::path_to_parent_inode_at(p.cwd, path, &mut name) {
        Some(parent) => {
            let name_len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            if name_len == 0 {
                inode::inode_put(parent);
                return Err(());
            }
            let inum = match dentry::dentry_search(parent, &name[..name_len]) {
                Some(n) => n,
                None => {
                    inode::inode_put(parent);
                    return Err(());
                }
            };
            let ip = inode::inode_get(inum);
            if ip.disk.type_ == INODE_TYPE_DIR {
                inode::inode_put(ip);
                inode::inode_put(parent);
                return Err(());
            }
            dentry::dentry_delete(parent, &name[..name_len]);
            ip.disk.nlink -= 1;
            inode::inode_rw(ip, true);
            inode::inode_put(ip);
            inode::inode_put(parent);
            Ok(())
        }
        None => Err(()),
    }
}

pub fn fs_get_dentries(p: &mut Process, fd: usize, u_buf: usize, max: usize) -> Result<usize, ()> {
    if fd >= crate::proc::process::NOFILE { return Err(()); }
    let f_idx = p.open_files[fd].ok_or(())?;
    let f = {
        let table = file::FILE_TABLE.lock();
        let f = &table.files[f_idx];
        (f.inum, f.ty)
    };

    let ip = inode::inode_get(f.0);
    if ip.disk.type_ != INODE_TYPE_DIR {
        inode::inode_put(ip);
        return Err(());
    }

    let mut count = 0;
    let mut off = 0;
    let dentry_size = core::mem::size_of::<inode::DentryDisk>() as u32;
    let mut buf = [0u8; core::mem::size_of::<inode::DentryDisk>()];
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };

    while off < ip.disk.size && count < max {
        if inode::inode_read_data(ip, off, dentry_size, &mut buf) != dentry_size {
            break;
        }
        let dd = unsafe { &*(buf.as_ptr() as *const inode::DentryDisk) };
        if dd.name[0] != 0 {
            let mut ud = file::Dirent { name: [0; 60], inum: dd.inode_num };
            ud.name.copy_from_slice(&dd.name);
            let src = unsafe {
                core::slice::from_raw_parts(&ud as *const file::Dirent as *const u8, core::mem::size_of::<file::Dirent>())
            };
            if let Err(_) = uvm::copyout(pt, u_buf + count * core::mem::size_of::<file::Dirent>(), src) {
                break;
            }
            count += 1;
        }
        off += dentry_size;
    }
    inode::inode_put(ip);
    Ok(count)
}

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

pub fn sys_show_bitmap(_ctx: &mut TrapContext) -> usize {
    0
}

pub fn sys_get_block(ctx: &mut TrapContext) -> usize {
    let block_no = ctx.a0 as u32;
    buffer::read(0, block_no)
}

pub fn sys_read_block(ctx: &mut TrapContext) -> usize {
    let buf_idx = ctx.a0;
    let u_dst = ctx.a1;
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

pub fn sys_flush_buffer(_ctx: &mut TrapContext) -> usize {
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
    if ret < 0 { usize::MAX } else { 0 }
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
    let res = path::path_to_inode_at(p.cwd, &path_buf[..path_len]);
    match res {
        Some(inode_ref) => {
            let inum = inode_ref.inode_num as usize;
            inode::inode_put(inode_ref);
            inum
        }
        None => usize::MAX,
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
    let res = path::path_to_parent_inode_at(p.cwd, &path_buf[..path_len], &mut name_buf);
    match res {
        Some(parent) => {
            if let Err(_) = uvm::copyout(pt, u_name_out, &name_buf) {
                inode::inode_put(parent);
                return usize::MAX;
            }
            let inum = parent.inode_num as usize;
            inode::inode_put(parent);
            inum
        }
        None => usize::MAX,
    }
}

pub fn sys_prepare_root_dir() -> usize {
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

// --- LAB-9 Syscalls ---

pub fn sys_open(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let flags = ctx.a1 as u32;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut path_buf = [0u8; 256];
    let copied = match uvm::copyin_str(pt, &mut path_buf, u_path) {
        Ok(n) => n,
        Err(_) => return usize::MAX,
    };
    let path_len = copied.saturating_sub(1).min(255);
    match fs_open(p, &path_buf[..path_len], flags) {
        Ok(fd) => fd,
        Err(_) => usize::MAX,
    }
}

pub fn sys_close(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let p = current_proc();
    match fs_close(p, fd) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_read(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let u_dst = ctx.a1;
    let len = ctx.a2;
    let p = current_proc();
    match fs_read(p, fd, u_dst, len) {
        Ok(n) => n,
        Err(_) => usize::MAX,
    }
}

pub fn sys_write(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let u_src = ctx.a1;
    let len = ctx.a2;
    let p = current_proc();
    match fs_write(p, fd, u_src, len) {
        Ok(n) => n,
        Err(_) => usize::MAX,
    }
}

pub fn sys_lseek(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let off = ctx.a1 as i32;
    let whence = ctx.a2 as u32;
    let p = current_proc();
    match fs_lseek(p, fd, off, whence) {
        Ok(n) => n,
        Err(_) => usize::MAX,
    }
}

pub fn sys_dup(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let p = current_proc();
    match fs_dup(p, fd) {
        Ok(n) => n,
        Err(_) => usize::MAX,
    }
}

pub fn sys_fstat(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let u_stat = ctx.a1;
    let p = current_proc();
    match fs_fstat(p, fd, u_stat) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_get_dentries(ctx: &mut TrapContext) -> usize {
    let fd = ctx.a0;
    let u_buf = ctx.a1;
    let max = ctx.a2;
    let p = current_proc();
    match fs_get_dentries(p, fd, u_buf, max) {
        Ok(n) => n,
        Err(_) => usize::MAX,
    }
}

pub fn sys_mkdir(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut path_buf = [0u8; 256];
    if let Err(_) = uvm::copyin_str(pt, &mut path_buf, u_path) { return usize::MAX; }
    let path_len = path_buf.iter().position(|&b| b == 0).unwrap_or(path_buf.len());
    match fs_mkdir(p, &path_buf[..path_len]) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_chdir(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut path_buf = [0u8; 256];
    if let Err(_) = uvm::copyin_str(pt, &mut path_buf, u_path) { return usize::MAX; }
    let path_len = path_buf.iter().position(|&b| b == 0).unwrap_or(path_buf.len());
    match fs_chdir(p, &path_buf[..path_len]) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_link(ctx: &mut TrapContext) -> usize {
    let u_old = ctx.a0;
    let u_new = ctx.a1;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut old_buf = [0u8; 256];
    let mut new_buf = [0u8; 256];
    if let Err(_) = uvm::copyin_str(pt, &mut old_buf, u_old) { return usize::MAX; }
    if let Err(_) = uvm::copyin_str(pt, &mut new_buf, u_new) { return usize::MAX; }
    let old_len = old_buf.iter().position(|&b| b == 0).unwrap_or(old_buf.len());
    let new_len = new_buf.iter().position(|&b| b == 0).unwrap_or(new_buf.len());
    match fs_link(p, &old_buf[..old_len], &new_buf[..new_len]) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_unlink(ctx: &mut TrapContext) -> usize {
    let u_path = ctx.a0;
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    let mut path_buf = [0u8; 256];
    if let Err(_) = uvm::copyin_str(pt, &mut path_buf, u_path) { return usize::MAX; }
    let path_len = path_buf.iter().position(|&b| b == 0).unwrap_or(path_buf.len());
    match fs_unlink(p, &path_buf[..path_len]) {
        Ok(_) => 0,
        Err(_) => usize::MAX,
    }
}

pub fn sys_print_cwd() -> usize {
    let p = current_proc();
    crate::printk!("CWD Inode: {}\n", p.cwd);
    0
}
