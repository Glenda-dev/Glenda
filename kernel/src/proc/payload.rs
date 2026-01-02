use crate::dtb;
use crate::mem::pmem;
use crate::mem::pte::perms;
use crate::mem::{EMPTY_VA, PGSIZE};
use crate::mem::{PageTable, PhysAddr, PteFlags, VirtAddr};
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET};
use spin::Once;

/*
Payload结构体
0x00 - 0x03: magic number (0x99999999)
0x04 - 0x07: number of entries (u32)
0x08 - ... : entries
Each entry:
0x00: type (u8)
0x01 - 0x04: offset (u32)
0x05 - 0x08: size (u32)
0x09 - 0x28: name (32 bytes, null-padded)
0x29 - 0x2F: reserved (7 bytes)
*/

#[repr(C)]
struct Header {
    magic: u32,
    count: u32,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadType {
    RootTask = 0,
    Driver = 1,
    Server = 2,
    Test = 3,
    File = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    info: PayloadType,
    offset: u32,
    size: u32,
    name: [u8; 32],
    _padding: [u8; 7],
}

pub struct ProcPayload {
    pub metadata: Entry,
    pub data: &'static [u8],
}

const PAYLOAD_MAGIC: u32 = 0x99999999;

static ROOT_TASK: Once<ProcPayload> = Once::new();

pub fn init() {
    let initrd = dtb::initrd_range();
    if initrd.is_none() {
        printk!(
            "{}[WARN] No initrd found in DTB, skipping payload parsing.{}\n",
            ANSI_RED,
            ANSI_RESET
        );
        return;
    }

    let range = initrd.unwrap();
    let payload_ptr = range.start.as_ptr::<u8>();
    let total_size = range.end().as_usize() - range.start.as_usize();

    // Read header bytes (safely, avoid alignment assumptions)
    let b0 = unsafe { *payload_ptr.add(0) };
    let b1 = unsafe { *payload_ptr.add(1) };
    let b2 = unsafe { *payload_ptr.add(2) };
    let b3 = unsafe { *payload_ptr.add(3) };
    let magic = u32::from_le_bytes([b0, b1, b2, b3]);
    let c0 = unsafe { *payload_ptr.add(4) };
    let c1 = unsafe { *payload_ptr.add(5) };
    let c2 = unsafe { *payload_ptr.add(6) };
    let c3 = unsafe { *payload_ptr.add(7) };
    let count = u32::from_le_bytes([c0, c1, c2, c3]);

    if magic != PAYLOAD_MAGIC {
        printk!("{}[WARN] Invalid payload magic: {:#x}{}\n", ANSI_RED, magic, ANSI_RESET);
        return;
    }
    printk!("proc: Initrd found, {} entries\n", count);

    if count == 0 {
        printk!("{}[WARN] Initrd is empty{}\n", ANSI_RED, ANSI_RESET);
        return;
    }

    // Entries start at offset 12, each ENTRY is 48 bytes
    let entry_base = 12usize;

    // Parse ONLY the first entry (Root Task)
    let ent_off = entry_base;

    // read fields from payload_ptr + ent_off
    let t = unsafe { *payload_ptr.add(ent_off) };
    let o0 = unsafe { *payload_ptr.add(ent_off + 1) };
    let o1 = unsafe { *payload_ptr.add(ent_off + 2) };
    let o2 = unsafe { *payload_ptr.add(ent_off + 3) };
    let o3 = unsafe { *payload_ptr.add(ent_off + 4) };
    let offset = u32::from_le_bytes([o0, o1, o2, o3]);

    let s0 = unsafe { *payload_ptr.add(ent_off + 5) };
    let s1 = unsafe { *payload_ptr.add(ent_off + 6) };
    let s2 = unsafe { *payload_ptr.add(ent_off + 7) };
    let s3 = unsafe { *payload_ptr.add(ent_off + 8) };
    let size = u32::from_le_bytes([s0, s1, s2, s3]);

    // name: bytes 9..40 (32 bytes)
    let mut name_buf = [0u8; 32];
    for j in 0..32 {
        name_buf[j] = unsafe { *payload_ptr.add(ent_off + 9 + j) };
    }
    // trim at first null
    let name_end = name_buf.iter().position(|&c| c == 0).unwrap_or(32);
    let name = core::str::from_utf8(&name_buf[..name_end]).unwrap_or("<invalid utf8>");

    printk!("proc: Found Root Task: type={} offset={} size={} name={}\n", t, offset, size, name);

    if t != 0 {
        // 0 is RootTask
        printk!("{}[WARN] First entry is not Root Task (type={}){}\n", ANSI_RED, t, ANSI_RESET);
    }

    // create slice
    let data = if size > 0 {
        let data_start = offset as usize;
        let end = data_start.checked_add(size as usize).unwrap_or(usize::MAX);
        if end > total_size {
            printk!(
                "{}[WARN] Root Task data out of bounds: {} + {} > {}{}\n",
                ANSI_RED,
                data_start,
                size,
                total_size,
                ANSI_RESET
            );
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(payload_ptr.add(data_start), size as usize) }
        }
    } else {
        &[]
    };

    // construct Entry metadata (packed interpretation)
    let metadata =
        Entry { info: PayloadType::RootTask, offset, size, name: name_buf, _padding: [0u8; 7] };

    let root_task = ProcPayload { metadata, data };
    let _ = ROOT_TASK.call_once(|| root_task);
}

pub fn get_root_task() -> Option<&'static ProcPayload> {
    ROOT_TASK.get()
}

impl ProcPayload {
    /// 简单的 ELF 解析 (Sv39)
    pub fn info(&self) -> (usize, usize) {
        if self.data.len() < 64 || &self.data[0..4] != b"\x7FELF" {
            return (0, 0);
        }
        // Entry point at offset 24 (64-bit ELF)
        let entry = u64::from_le_bytes(self.data[24..32].try_into().unwrap()) as usize;

        // 默认栈顶 (Sv39 用户空间高地址)
        let stack_top = 0x4000000000;
        (entry, stack_top)
    }

    /// 遍历 ELF Program Headers 并映射 LOAD 段
    pub fn map_segments(&self, vspace: &mut PageTable) {
        if self.data.len() < 64 || &self.data[0..4] != b"\x7FELF" {
            return;
        }
        let phoff = u64::from_le_bytes(self.data[32..40].try_into().unwrap()) as usize;
        let phnum = u16::from_le_bytes(self.data[56..58].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(self.data[54..56].try_into().unwrap()) as usize;

        for i in 0..phnum {
            let off = phoff + i * phentsize;
            let p_type = u32::from_le_bytes(self.data[off..off + 4].try_into().unwrap());
            if p_type == 1 {
                // PT_LOAD
                let p_offset =
                    u64::from_le_bytes(self.data[off + 8..off + 16].try_into().unwrap()) as usize;
                let p_vaddr =
                    u64::from_le_bytes(self.data[off + 16..off + 24].try_into().unwrap()) as usize;
                let p_filesz =
                    u64::from_le_bytes(self.data[off + 32..off + 40].try_into().unwrap()) as usize;
                let p_memsz =
                    u64::from_le_bytes(self.data[off + 40..off + 48].try_into().unwrap()) as usize;
                let p_flags = u32::from_le_bytes(self.data[off + 4..off + 8].try_into().unwrap());

                let mut flags = PteFlags::from(perms::USER | perms::VALID);
                if p_flags & perms::EXECUTE as u32 != 0 {
                    flags |= perms::EXECUTE;
                }
                if p_flags & perms::WRITE as u32 != 0 {
                    flags |= perms::WRITE;
                }
                if p_flags & perms::READ as u32 != 0 {
                    flags |= perms::READ;
                }

                let num_pages = (p_memsz + PGSIZE - 1) / PGSIZE;
                for j in 0..num_pages {
                    let frame_cap =
                        pmem::alloc_frame_cap(1).expect("Failed to alloc frame for segment");

                    let va = VirtAddr::from(p_vaddr) + j * PGSIZE;
                    let copy_size = if (j + 1) * PGSIZE <= p_filesz {
                        PGSIZE
                    } else if j * PGSIZE < p_filesz {
                        p_filesz - j * PGSIZE
                    } else {
                        0
                    };

                    if copy_size > 0 {
                        let src =
                            &self.data[p_offset + j * PGSIZE..p_offset + j * PGSIZE + copy_size];
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                src.as_ptr(),
                                frame_cap.obj_ptr().as_mut_ptr::<u8>(),
                                copy_size,
                            );
                        }
                    }

                    vspace.map_with_alloc(va, frame_cap.obj_ptr().to_pa(), PGSIZE, flags);
                    core::mem::forget(frame_cap);
                }
            }
        }
    }

    // Map Flat Entire Binary
    pub fn map(&self, vspace: &mut PageTable) {
        // Copy data into newly allocated frames
        let flags = PteFlags::from(perms::USER | perms::READ | perms::EXECUTE | perms::VALID);
        let num_pages = (self.data.len() + PGSIZE - 1) / PGSIZE;
        for j in 0..num_pages {
            let frame_cap =
                pmem::alloc_frame_cap(1).expect("Failed to alloc frame for flat mapping");
            let va = VirtAddr::from(EMPTY_VA + j * PGSIZE);
            let src_pa = PhysAddr::from(self.data.as_ptr() as usize + j * PGSIZE);
            let src_va = src_pa.to_va();
            let copy_size = if (j + 1) * PGSIZE <= self.data.len() {
                PGSIZE
            } else {
                self.data.len() - j * PGSIZE
            };
            if copy_size > 0 {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        src_va.as_ptr::<u8>(),
                        frame_cap.obj_ptr().as_mut_ptr::<u8>(),
                        copy_size,
                    );
                }
            }

            vspace.map_with_alloc(va, frame_cap.obj_ptr().to_pa(), PGSIZE, flags);
            core::mem::forget(frame_cap);
        }
    }
}
