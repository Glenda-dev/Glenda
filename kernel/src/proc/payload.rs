use crate::mem::PhysFrame;
use crate::mem::{PGSIZE, pte};
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET};
use alloc::vec::Vec;
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
enum PayloadType {
    RootTask = 0,
    Driver = 1,
    Server = 2,
    Test = 3,
    File = 4,
}

#[repr(C)]
struct Entry {
    info: PayloadType,
    offset: u32,
    size: u32,
    name: [u8; 32],
    _padding: [u8; 7],
}

pub struct ProcBinary {
    num_entries: u32,
    entries: Vec<ProcPayload>,
}

pub struct ProcPayload {
    pub metadata: Entry,
    pub data: Vec<u8>,
}

const PAYLOAD_MAGIC: u32 = 0x99999999;

static PAYLOAD: Once<ProcBinary> = Once::new();

unsafe extern "C" {
    static __payload_start: u8;
}

pub fn init() {
    let payload_ptr = unsafe { &__payload_start as *const u8 };
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
    let t0 = unsafe { *payload_ptr.add(8) };
    let t1 = unsafe { *payload_ptr.add(9) };
    let t2 = unsafe { *payload_ptr.add(10) };
    let t3 = unsafe { *payload_ptr.add(11) };
    let total_size = u32::from_le_bytes([t0, t1, t2, t3]);

    if magic != PAYLOAD_MAGIC {
        printk!("{}[WARN] Invalid payload magic: {:#x}{}\n", ANSI_RED, magic, ANSI_RESET);
    }
    printk!("proc: Loading process binary with {} entries\n", count);

    let mut parsed = ProcBinary { num_entries: count, entries: Vec::new() };

    // Entries start at offset 12, each ENTRY is 48 bytes
    let entry_base = 12usize;
    let total_size_usize = total_size as usize;

    // basic sanity check
    if (total_size as usize) < entry_base + (count as usize) * 48 {
        printk!("{}[WARN] payload total_size too small: {}{}\n", ANSI_RED, total_size, ANSI_RESET);
        return;
    }
    for i in 0..(count as usize) {
        let ent_off = entry_base + i * 48;
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

        printk!("proc: entry {} type={} offset={} size={} name={}\n", i, t, offset, size, name);

        // copy data with bounds check using total_size
        let mut data = Vec::new();
        if size > 0 {
            let data_start = offset as usize;
            let end = data_start.checked_add(size as usize).unwrap_or(usize::MAX);
            if end > total_size_usize {
                printk!(
                    "{}[WARN] entry {} data out of bounds: {} + {} > {}{}\n",
                    ANSI_RED,
                    i,
                    data_start,
                    size,
                    total_size,
                    ANSI_RESET
                );
                continue;
            }
            for k in 0..(size as usize) {
                let v = unsafe { *payload_ptr.add(data_start + k) };
                data.push(v);
            }
        }

        // construct Entry metadata (packed interpretation)
        let metadata = Entry {
            info: match t {
                0 => PayloadType::RootTask,
                1 => PayloadType::Driver,
                2 => PayloadType::Server,
                3 => PayloadType::Test,
                4 => PayloadType::File,
                _ => PayloadType::File,
            },
            offset,
            size,
            name: name_buf,
            _padding: [0u8; 7],
        };

        parsed.entries.push(ProcPayload { metadata, data });
    }

    // initialize static once with parsed payload
    let _ = PAYLOAD.call_once(|| parsed);
}

pub fn get_root_task() -> Option<&'static ProcPayload> {
    let payload = PAYLOAD.get().expect("Payload not initialized");
    for entry in &payload.entries {
        if let PayloadType::RootTask = entry.metadata.info {
            return Some(entry);
        }
    }
    None
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
    pub fn map_segments(&self, vspace: &mut crate::mem::PageTable) {
        if self.data.len() < 64 || &self.data[0..4] != b"\x7FELF" {
            return;
        }
        let phoff = u64::from_le_bytes(self.data[32..40].try_into().unwrap()) as usize;
        let phnum = u16::from_le_bytes(self.data[56..58].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(self.data[54..56].try_into().unwrap()) as usize;

        for i in 0..phnum {
            let off = phoff + i * phentsize;
            let p_type = u32::from_le_bytes(self.data[off..off+4].try_into().unwrap());
            if p_type == 1 { // PT_LOAD
                let p_offset = u64::from_le_bytes(self.data[off+8..off+16].try_into().unwrap()) as usize;
                let p_vaddr = u64::from_le_bytes(self.data[off+16..off+24].try_into().unwrap()) as usize;
                let p_filesz = u64::from_le_bytes(self.data[off+32..off+40].try_into().unwrap()) as usize;
                let p_memsz = u64::from_le_bytes(self.data[off+40..off+48].try_into().unwrap()) as usize;
                let p_flags = u32::from_le_bytes(self.data[off+4..off+8].try_into().unwrap());

                let mut flags = pte::PTE_U | pte::PTE_V;
                if p_flags & 1 != 0 { flags |= pte::PTE_X; }
                if p_flags & 2 != 0 { flags |= pte::PTE_W; }
                if p_flags & 4 != 0 { flags |= pte::PTE_R; }

                let num_pages = (p_memsz + PGSIZE - 1) / PGSIZE;
                for j in 0..num_pages {
                    let mut frame = PhysFrame::alloc().expect("Failed to alloc frame for segment");
                    frame.zero();
                    
                    let va = p_vaddr + j * PGSIZE;
                    let copy_size = if (j + 1) * PGSIZE <= p_filesz {
                        PGSIZE
                    } else if j * PGSIZE < p_filesz {
                        p_filesz - j * PGSIZE
                    } else {
                        0
                    };

                    if copy_size > 0 {
                        let src = &self.data[p_offset + j * PGSIZE .. p_offset + j * PGSIZE + copy_size];
                        unsafe {
                            core::ptr::copy_nonoverlapping(src.as_ptr(), frame.va() as *mut u8, copy_size);
                        }
                    }

                    // 建立中间页表 (如果需要)
                    // 注意：vspace.map 假设中间页表已存在，所以我们需要先 map_table
                    // 这里简化处理，假设 root task 的代码段在同一个 1GB/2MB 范围内
                    // 或者我们应该在 map 内部自动处理 (但微内核原则是不自动处理)
                    // 为了 Root Task 启动，我们在这里手动处理一下
                    for level in (1..3).rev() {
                        let _ = vspace.map_table(va, PhysFrame::alloc().unwrap().leak(), level);
                    }

                    vspace.map(va, frame.addr(), PGSIZE, flags).expect("Failed to map segment");
                    frame.leak();
                }
            }
        }
    }
}
