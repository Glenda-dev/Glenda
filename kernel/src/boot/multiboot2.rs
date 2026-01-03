use crate::printk;

#[repr(C)]
struct MbiHeader {
    total_size: u32,
    reserved: u32,
}

#[repr(C)]
struct TagHeader {
    typ: u32,
    size: u32,
}

#[repr(C)]
struct MemoryMapTag {
    typ: u32,
    size: u32,
    entry_size: u32,
    entry_version: u32,
}

#[repr(C)]
struct MemoryMapEntry {
    base_addr: u64,
    length: u64,
    typ: u32,
    reserved: u32,
}

#[repr(C)]
struct ModuleTag {
    typ: u32,
    size: u32,
    mod_start: u32,
    mod_end: u32,
}

pub const MULTIBOOT2_MAGIC: u32 = 0x36d76289;

pub struct MultibootInfo {
    pub dtb: Option<*const u8>,
    pub initrd_start: Option<usize>,
    pub initrd_end: Option<usize>,
}

pub static mut MULTIBOOT_INITRD: (usize, usize) = (0, 0);

pub fn parse(magic: usize, addr: usize) -> MultibootInfo {
    let mut info = MultibootInfo { dtb: None, initrd_start: None, initrd_end: None };

    if magic != MULTIBOOT2_MAGIC as usize {
        return info;
    }

    printk!("boot: Multiboot2 detected at {:#x}\n", addr);

    let header = unsafe { &*(addr as *const MbiHeader) };
    let mut current_addr = addr + 8;
    let end_addr = addr + header.total_size as usize;

    while current_addr < end_addr {
        let tag = unsafe { &*(current_addr as *const TagHeader) };
        if tag.typ == 0 && tag.size == 8 {
            break;
        }

        match tag.typ {
            1 => {
                // Command line
                let s = unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        (current_addr + 8) as *const u8,
                        (tag.size - 8) as usize,
                    ))
                };
                printk!("boot: Multiboot2 command line: {}\n", s);
            }
            2 => {
                // Boot loader name
                let s = unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        (current_addr + 8) as *const u8,
                        (tag.size - 8) as usize,
                    ))
                };
                printk!("boot: Multiboot2 boot loader: {}\n", s);
            }
            3 => {
                // Module (initrd)
                let mod_tag = unsafe { &*(current_addr as *const ModuleTag) };
                info.initrd_start = Some(mod_tag.mod_start as usize);
                info.initrd_end = Some(mod_tag.mod_end as usize);
                printk!(
                    "boot: Multiboot2 module found: {:#x} - {:#x}\n",
                    mod_tag.mod_start,
                    mod_tag.mod_end
                );
            }
            6 => {
                // Memory map
                let mmap_tag = unsafe { &*(current_addr as *const MemoryMapTag) };
                let entry_count = (mmap_tag.size - 16) / mmap_tag.entry_size;
                printk!("boot: Multiboot2 memory map ({} entries):\n", entry_count);
                for i in 0..entry_count {
                    let entry = unsafe {
                        &*((current_addr + 16 + (i * mmap_tag.entry_size) as usize)
                            as *const MemoryMapEntry)
                    };
                    printk!(
                        "  base: {:#x}, len: {:#x}, type: {}\n",
                        entry.base_addr,
                        entry.length,
                        entry.typ
                    );
                }
            }
            // Tag type for DTB in Multiboot2 is often 11 (EFI DTB) or something else.
            11 => {
                info.dtb = Some((current_addr + 8) as *const u8);
            }
            _ => {}
        }

        current_addr = (current_addr + tag.size as usize + 7) & !7;
    }
    info
}
