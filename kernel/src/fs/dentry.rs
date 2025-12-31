use crate::fs::inode::{self, Inode, DentryDisk, MAXLEN_FILENAME};
use crate::fs::buffer::BLOCK_SIZE;
use crate::printk;
use core::mem::size_of;
use core::slice;

pub fn dentry_search(dir: &mut Inode, name: &[u8]) -> Option<u32> {
    let mut off = 0;
    let size = dir.disk.size;
    let dentry_size = size_of::<DentryDisk>() as u32;

    let mut buf = [0u8; size_of::<DentryDisk>()];

    while off < size {
        if inode::inode_read_data(dir, off, dentry_size, &mut buf) != dentry_size {
            break;
        }

        let dentry = unsafe { &*(buf.as_ptr() as *const DentryDisk) };
        if dentry.name[0] != 0 {
            // Check match
            let mut match_ = true;
            for i in 0..MAXLEN_FILENAME {
                if name.len() > i {
                     if dentry.name[i] != name[i] {
                        match_ = false;
                        break;
                     }
                } else if dentry.name[i] != 0 {
                    match_ = false;
                    break;
                }
            }
            if match_ {
                return Some(dentry.inode_num);
            }
        }
        off += dentry_size;
    }
    None
}

pub fn dentry_create(dir: &mut Inode, target_inum: u32, name: &[u8]) -> i32 {
    // Check if name already exists
    if dentry_search(dir, name).is_some() {
        return -1;
    }

    let mut off = 0;
    let size = dir.disk.size;
    let dentry_size = size_of::<DentryDisk>() as u32;
    let mut buf = [0u8; size_of::<DentryDisk>()];
    
    // Find empty slot
    let mut target_off = size;
    let mut found_empty = false;

    // Linear scan for empty slot
    while off < size {
        if inode::inode_read_data(dir, off, dentry_size, &mut buf) != dentry_size {
             break;
        }
        let dentry = unsafe { &*(buf.as_ptr() as *const DentryDisk) };
        if dentry.name[0] == 0 {
            target_off = off;
            found_empty = true;
            break;
        }
        off += dentry_size;
    }

    // Construct new dentry
    let mut new_dentry = DentryDisk {
        name: [0; MAXLEN_FILENAME],
        inode_num: target_inum,
    };
    
    let len = if name.len() > MAXLEN_FILENAME { MAXLEN_FILENAME } else { name.len() };
    for i in 0..len {
        new_dentry.name[i] = name[i];
    }

    let src = unsafe {
        slice::from_raw_parts(&new_dentry as *const DentryDisk as *const u8, size_of::<DentryDisk>())
    };

    if inode::inode_write_data(dir, target_off, dentry_size, src) != dentry_size {
        return -1;
    }

    0
}

pub fn dentry_delete(dir: &mut Inode, name: &[u8]) -> i32 {
    let mut off = 0;
    let size = dir.disk.size;
    let dentry_size = size_of::<DentryDisk>() as u32;
    let mut buf = [0u8; size_of::<DentryDisk>()];

    while off < size {
        if inode::inode_read_data(dir, off, dentry_size, &mut buf) != dentry_size {
            break;
        }

        let dentry = unsafe { &mut *(buf.as_mut_ptr() as *mut DentryDisk) };
        if dentry.name[0] != 0 {
             let mut match_ = true;
            for i in 0..MAXLEN_FILENAME {
                if name.len() > i {
                     if dentry.name[i] != name[i] {
                        match_ = false;
                        break;
                     }
                } else if dentry.name[i] != 0 {
                    match_ = false;
                    break;
                }
            }
            
            if match_ {
                let inum = dentry.inode_num;
                // Zero out
                unsafe {
                    core::ptr::write_bytes(buf.as_mut_ptr(), 0, size_of::<DentryDisk>());
                }
                inode::inode_write_data(dir, off, dentry_size, &buf);
                return inum as i32;
            }
        }
        off += dentry_size;
    }
    -1
}

pub fn dentry_print(dir: &mut Inode) {
    let mut off = 0;
    let size = dir.disk.size;
    let dentry_size = size_of::<DentryDisk>() as u32;
    let mut buf = [0u8; size_of::<DentryDisk>()];

    printk!("Directory content (inode {}):\n", dir.inode_num);
    while off < size {
        if inode::inode_read_data(dir, off, dentry_size, &mut buf) != dentry_size {
            break;
        }

        let dentry = unsafe { &*(buf.as_ptr() as *const DentryDisk) };
        if dentry.name[0] != 0 {
            // Print name safely
            let mut len = 0;
            while len < MAXLEN_FILENAME && dentry.name[len] != 0 {
                len += 1;
            }
            let name_str = core::str::from_utf8(&dentry.name[..len]).unwrap_or("???");
            printk!("  entry: '{}', inode: {}\n", name_str, dentry.inode_num);
        }
        off += dentry_size;
    }
}
