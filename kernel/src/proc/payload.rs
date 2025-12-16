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
    File = 3,
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
                3 => PayloadType::File,
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
