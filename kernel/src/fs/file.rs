use crate::fs::inode::{self, Inode};
use spin::Mutex;

pub const NFILE: usize = 128; // 全局最大文件数

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    None,
    Inode,
    Device { major: u16, minor: u16 },
    Pipe,
}

pub struct File {
    pub ty: FileType,
    pub readable: bool,
    pub writable: bool,
    pub off: u32,
    pub inum: u32,
    pub refcnt: u32,
}

impl File {
    pub const fn new() -> Self {
        Self {
            ty: FileType::None,
            readable: false,
            writable: false,
            off: 0,
            inum: 0,
            refcnt: 0,
        }
    }
}

pub struct FileTable {
    pub files: [File; NFILE],
}

pub static FILE_TABLE: Mutex<FileTable> = Mutex::new(FileTable {
    files: [const { File::new() }; NFILE],
});

pub fn file_alloc() -> Option<(usize, &'static mut File)> {
    let mut table = FILE_TABLE.lock();
    for i in 0..NFILE {
        if table.files[i].refcnt == 0 {
            let f_ptr = &mut table.files[i] as *mut File;
            let f = unsafe { &mut *f_ptr };
            f.refcnt = 1;
            f.off = 0;
            return Some((i, f));
        }
    }
    None
}

pub fn file_dup(f: &mut File) {
    let _guard = FILE_TABLE.lock();
    if f.refcnt < 1 {
        panic!("file_dup: trying to dup a free file");
    }
    f.refcnt += 1;
}

pub fn file_close(f_idx: usize) {
    let mut table = FILE_TABLE.lock();
    let f_ptr = &mut table.files[f_idx] as *mut File;
    let f = unsafe { &mut *f_ptr };

    if f.refcnt < 1 {
        panic!("file_close: already free");
    }
    f.refcnt -= 1;
    if f.refcnt > 0 {
        return;
    }

    // Truly close
    let ty = f.ty;
    let inum = f.inum;
    f.ty = FileType::None;

    drop(table); // Release table lock before calling inode_put which might lock other things

    if let FileType::Inode = ty {
        let inode_ref = inode::inode_get(inum);
        inode::inode_put(inode_ref);
        inode::inode_put(inode_ref);
    }
}

// --- Userspace Compatibility Structures ---

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stat {
    pub type_: u16,
    pub nlink: u16,
    pub size: u32,
    pub major: u16,
    pub minor: u16,
    pub inum: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Dirent {
    pub name: [u8; 60],
    pub inum: u32,
}
