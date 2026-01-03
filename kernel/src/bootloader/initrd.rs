use crate::dtb::{self, MemoryRange};
use crate::mem::pmem;
use crate::mem::pte::perms;
use crate::mem::{BOOTINFO_VA, PGSIZE};
use crate::mem::{PageTable, PteFlags, VirtAddr};
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET};
use crate::proc::ElfFile;
use spin::Once;

use super::{BOOT_LOADER_TYPE, BootLoaderType};
#[cfg(feature = "multiboot2")]
use super::multiboot2;

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

static INITRD_RANGE: Once<dtb::MemoryRange> = Once::new();

pub fn init() {
    let initrd = unsafe {
        match BOOT_LOADER_TYPE {
            BootLoaderType::OpenSBI => dtb::initrd_range(),
            #[cfg(feature = "multiboot2")]
            BootLoaderType::Multiboot2 => {
                let (start, end) = multiboot2::MULTIBOOT_INITRD;
                if start != 0 && end != 0 {
                    Some(dtb::MemoryRange {
                        start: crate::mem::PhysAddr::from(start),
                        size: end - start,
                    })
                } else {
                    None
                }
            }
        }
    };

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
    printk!("initrd: Initrd found, {} entries\n", count);

    if count == 0 {
        printk!("{}[WARN] Initrd is empty{}\n", ANSI_RED, ANSI_RESET);
        return;
    }

    // Entries start at offset 16 (magic + count + total_size + padding)
    let entry_base = 16usize;

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

    printk!(
        "initrd: Found Root Task: type={} offset={} size={}KB name={}\n",
        t,
        offset,
        size / 1024,
        name
    );

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
    let _ = INITRD_RANGE.call_once(|| range);
}

pub fn get_root_task() -> Option<&'static ProcPayload> {
    ROOT_TASK.get()
}

pub fn range() -> Option<MemoryRange> {
    let range = INITRD_RANGE.get();
    if range.is_none() {
        return None;
    }
    Some(*range.unwrap())
}

impl ProcPayload {
    pub fn as_elf(&self) -> Option<ElfFile<'_>> {
        // Check magic number
        if self.data.len() < 4 {
            return None;
        }
        let magic = &self.data[0..4];
        if magic != b"\x7FELF" {
            return None;
        }
        ElfFile::new(self.data).ok()
    }

    pub fn info(&self) -> (usize, usize) {
        let entry = if let Some(elf) = self.as_elf() {
            elf.entry_point()
        } else {
            0x10000 // Default for flat binary
        };

        // 默认栈顶 (BootInfo 下方)
        let stack_top = BOOTINFO_VA;
        (entry, stack_top)
    }

    pub fn map(&self, vspace: &mut PageTable) {
        if let Some(elf) = self.as_elf() {
            let _ = elf.map(vspace);
        } else {
            self.map_flat(vspace);
        }
    }

    // Map Flat Entire Binary
    pub fn map_flat(&self, vspace: &mut PageTable) {
        // Copy data into newly allocated frames
        let flags = PteFlags::from(
            perms::USER | perms::READ | perms::EXECUTE | perms::WRITE | perms::VALID,
        );
        let num_pages = (self.data.len() + PGSIZE - 1) / PGSIZE;

        for j in 0..num_pages {
            // 1. 分配一个新的物理页
            let frame_cap =
                pmem::alloc_frame_cap(1).expect("Failed to alloc frame for flat mapping");

            // 2. 获取该物理页在内核中的虚拟地址（用于写入数据）
            let dst_va = frame_cap.obj_ptr();
            let dst_slice =
                unsafe { core::slice::from_raw_parts_mut(dst_va.as_mut_ptr::<u8>(), PGSIZE) };

            // 3. 计算源数据范围
            let start = j * PGSIZE;
            let end = core::cmp::min(start + PGSIZE, self.data.len());
            let src_slice = &self.data[start..end];

            // 4. 拷贝数据 (先清零，再拷贝)
            dst_slice.fill(0);
            dst_slice[0..src_slice.len()].copy_from_slice(src_slice);

            // 5. 映射到用户空间 (0x10000 + offset)
            let user_va = VirtAddr::from(0x10000 + j * PGSIZE);
            vspace.map_with_alloc(user_va, frame_cap.obj_ptr().to_pa(), PGSIZE, flags);

            // 6. 忘记 Capability，防止 Drop 时释放物理页（因为已经移交给页表管理了）
            core::mem::forget(frame_cap);
        }
    }
}
