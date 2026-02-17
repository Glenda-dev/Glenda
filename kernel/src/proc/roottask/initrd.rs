use crate::boot;
use crate::hal::mem::{PGSIZE, USER_VA};
use crate::mem::addr::phys_to_virt;
use crate::mem::pmem;
use crate::mem::{PageTable, Perms, VirtAddr};
// use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use crate::proc::ElfFile;
use crate::proc::roottask::STACK_BASE;
use crate::sync::Once;

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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadType {
    RootTask = 0,
    Driver = 1,
    Server = 2,
    Test = 3,
    File = 4,
    Unknown = 255,
}

impl From<u8> for PayloadType {
    fn from(val: u8) -> Self {
        match val {
            0 => PayloadType::RootTask,
            1 => PayloadType::Driver,
            2 => PayloadType::Server,
            3 => PayloadType::Test,
            4 => PayloadType::File,
            _ => PayloadType::Unknown,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub info: PayloadType,
    pub offset: u32,
    pub size: u32,
    pub name: [u8; 32],
    pub _padding: [u8; 7],
}

pub struct ProcPayload {
    pub metadata: Entry,
    pub data: &'static [u8],
}

const PAYLOAD_MAGIC: u32 = 0x99999999;

static INITRD_REGION: Once<(VirtAddr, usize)> = Once::new();

pub fn init() {
    log!("proc: Initializing initrd...");
    let (vaddr, size) = boot::get_initrd().expect("No initrd found");
    let payload_va = vaddr;
    log!("proc: Found initrd: va={:?}, size={}", payload_va, size);

    let ptr = payload_va.as_ptr::<u8>();
    let b0 = unsafe { *ptr.add(0) };
    let b1 = unsafe { *ptr.add(1) };
    let b2 = unsafe { *ptr.add(2) };
    let b3 = unsafe { *ptr.add(3) };
    let magic = u32::from_le_bytes([b0, b1, b2, b3]);

    if magic != PAYLOAD_MAGIC {
        warn!("proc: {}Warning{}: Invalid payload magic: {:#x}", ANSI_YELLOW, ANSI_RESET, magic);
        return;
    }

    // Store region
    INITRD_REGION.call_once(|| (payload_va, size));
    let count = count_entries();
    log!("proc: Initrd found, {} entries, {} KB", count, size / 1024);
}

fn count_entries() -> u32 {
    if let Some((base, _)) = INITRD_REGION.get() {
        let ptr = base.as_ptr::<u8>();
        let c0 = unsafe { *ptr.add(4) };
        let c1 = unsafe { *ptr.add(5) };
        let c2 = unsafe { *ptr.add(6) };
        let c3 = unsafe { *ptr.add(7) };
        u32::from_le_bytes([c0, c1, c2, c3])
    } else {
        0
    }
}

pub fn print_files() {
    let count = count_entries();
    if count == 0 {
        printk!("  (empty)\n");
        return;
    }
    let (base, _) = INITRD_REGION.get().unwrap();
    let ptr = base.as_ptr::<u8>();
    let entry_base = 16;
    let entry_size = 48;

    for i in 0..count {
        let off = entry_base + (i as usize) * entry_size;
        let mut name_buf = [0u8; 32];
        for j in 0..32 {
            name_buf[j] = unsafe { *ptr.add(off + 9 + j) };
        }
        let len = name_buf.iter().position(|&c| c == 0).unwrap_or(32);
        let name = core::str::from_utf8(&name_buf[..len]).unwrap_or("<invalid>");

        let type_byte = unsafe { *ptr.add(off) };
        let type_name = match PayloadType::from(type_byte) {
            PayloadType::RootTask => "RootTask",
            PayloadType::Driver => "Driver",
            PayloadType::Server => "Server",
            PayloadType::Test => "Test",
            PayloadType::File => "File",
            PayloadType::Unknown => "Unknown",
        };

        let s0 = unsafe { *ptr.add(off + 5) };
        let s1 = unsafe { *ptr.add(off + 6) };
        let s2 = unsafe { *ptr.add(off + 7) };
        let s3 = unsafe { *ptr.add(off + 8) };
        let size = u32::from_le_bytes([s0, s1, s2, s3]);

        printk!("  - {} ({} KB) [{}]\n", name, size / 1024, type_name);
    }
}

pub fn cat_file(name: &str) {
    if let Some(payload) = find(name) {
        if let Ok(s) = core::str::from_utf8(payload.data) {
            printk!("{}\n", s);
        } else {
            printk!("(Binary file, {} bytes)\n", payload.data.len());
        }
    } else {
        printk!("cat: {}: No such file\n", name);
    }
}

pub fn find(name: &str) -> Option<ProcPayload> {
    let count = count_entries();
    if count == 0 {
        return None;
    }

    // Scan by name
    for i in 0..count {
        if let Some(payload) = get_by_index(i) {
            let len = payload.metadata.name.iter().position(|&c| c == 0).unwrap_or(32);
            let entry_name = core::str::from_utf8(&payload.metadata.name[..len]).unwrap_or("");
            if entry_name == name {
                return Some(payload);
            }
        }
    }
    None
}

pub fn get_by_index(index: u32) -> Option<ProcPayload> {
    let count = count_entries();
    if index >= count {
        return None;
    }

    let (base, total_size) = INITRD_REGION.get()?;
    let ptr = base.as_ptr::<u8>();
    let entry_base = 16;
    let entry_size = 48;

    let off = entry_base + (index as usize) * entry_size;

    let mut name_buf = [0u8; 32];
    for j in 0..32 {
        name_buf[j] = unsafe { *ptr.add(off + 9 + j) };
    }

    let o0 = unsafe { *ptr.add(off + 1) };
    let o1 = unsafe { *ptr.add(off + 2) };
    let o2 = unsafe { *ptr.add(off + 3) };
    let o3 = unsafe { *ptr.add(off + 4) };
    let offset = u32::from_le_bytes([o0, o1, o2, o3]);

    let s0 = unsafe { *ptr.add(off + 5) };
    let s1 = unsafe { *ptr.add(off + 6) };
    let s2 = unsafe { *ptr.add(off + 7) };
    let s3 = unsafe { *ptr.add(off + 8) };
    let size = u32::from_le_bytes([s0, s1, s2, s3]);

    let _type_byte = unsafe { *ptr.add(off) };

    let data = if size > 0 {
        if (offset as usize) + (size as usize) > *total_size {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(ptr.add(offset as usize), size as usize) }
        }
    } else {
        &[]
    };

    Some(ProcPayload {
        metadata: Entry {
            info: PayloadType::from(_type_byte),
            offset,
            size,
            name: name_buf,
            _padding: [0; 7],
        },
        data,
    })
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

        // 默认栈起始地址 (BootInfo 下方)
        let stack_top = STACK_BASE;
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
        let flags = Perms::USER | Perms::READ | Perms::EXECUTE | Perms::WRITE | Perms::VALID;
        let num_pages = (self.data.len() + PGSIZE - 1) / PGSIZE;
        // 1. 分配一个新的物理页

        for j in 0..num_pages {
            let frame_pa = pmem::alloc_page().expect("Failed to allocate page for flat binary");
            let frame_va = phys_to_virt(frame_pa);
            // 2. 获取该物理页在内核中的虚拟地址（用于写入数据）
            let dst_va = frame_va + j * PGSIZE;
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
            let user_va = VirtAddr::from(USER_VA + start);
            vspace.map_with_alloc(user_va, frame_pa, PGSIZE, flags);
        }
    }
}
