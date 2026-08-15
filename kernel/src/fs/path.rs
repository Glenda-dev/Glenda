use crate::fs::inode::{self, Inode, ROOT_INODE, INODE_TYPE_DIR, MAXLEN_FILENAME};
use crate::fs::dentry;

fn get_element(path: &[u8], mut pos: usize) -> Option<(&[u8], usize)> {
    // Skip leading slashes
    while pos < path.len() && path[pos] == b'/' {
        pos += 1;
    }

    if pos >= path.len() {
        return None;
    }

    let start = pos;
    while pos < path.len() && path[pos] != b'/' {
        pos += 1;
    }

    let len = pos - start;
    if len > MAXLEN_FILENAME {
        // Ignore this for now..
    }

    Some((&path[start..pos], pos))
}

fn __path_to_inode_at(cwd_inum: u32, path: &[u8]) -> Option<&'static mut Inode> {
    let start_inum = if path.starts_with(b"/") {
        inode::ROOT_INODE
    } else {
        cwd_inum
    };

    let mut inode = inode::inode_get(start_inum);
    let mut pos = 0;

    loop {
        let (name, next_pos) = match get_element(path, pos) {
            Some(res) => res,
            None => return Some(inode), // End of path
        };
        pos = next_pos;

        if inode.disk.type_ != INODE_TYPE_DIR {
            inode::inode_put(inode);
            return None;
        }

        match dentry::dentry_search(inode, name) {
            Some(inum) => {
                let next_inode = inode::inode_get(inum);
                inode::inode_put(inode);
                inode = next_inode;
            }
            None => {
                inode::inode_put(inode);
                return None;
            }
        }
    }
}

pub fn path_to_inode(path: &[u8]) -> Option<&'static mut Inode> {
    __path_to_inode_at(inode::ROOT_INODE, path)
}

pub fn path_to_inode_at(cwd_inum: u32, path: &[u8]) -> Option<&'static mut Inode> {
    __path_to_inode_at(cwd_inum, path)
}

pub fn path_to_parent_inode_at(cwd_inum: u32, path: &[u8], name_buf: &mut [u8]) -> Option<&'static mut Inode> {
    let start_inum = if path.starts_with(b"/") {
        inode::ROOT_INODE
    } else {
        cwd_inum
    };

    let mut inode = inode::inode_get(start_inum);
    let mut pos = 0;

    // Check if path is empty or just slashes
    let (mut name, mut next_pos) = match get_element(path, pos) {
        Some(res) => res,
        None => return Some(inode),
    };
    pos = next_pos;

    loop {
        // Look ahead
        let (next_name, next_next_pos) = match get_element(path, pos) {
            Some(res) => res,
            None => {
                let len = if name.len() > MAXLEN_FILENAME { MAXLEN_FILENAME } else { name.len() };
                for i in 0..len {
                    name_buf[i] = name[i];
                }
                if len < name_buf.len() {
                    for i in len..name_buf.len() { name_buf[i] = 0; }
                }

                return Some(inode);
            }
        };

        // 'name' is a directory we need to traverse
        if inode.disk.type_ != INODE_TYPE_DIR {
            inode::inode_put(inode);
            return None;
        }

        match dentry::dentry_search(inode, name) {
            Some(inum) => {
                let next_inode = inode::inode_get(inum);
                inode::inode_put(inode);
                inode = next_inode;
            }
            None => {
                inode::inode_put(inode);
                return None;
            }
        }

        name = next_name;
        pos = next_next_pos;
    }
}
