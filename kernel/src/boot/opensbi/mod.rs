use crate::boot::BOOT_LOADER_INFO;
use crate::boot::BootLoaderInfo;
use crate::boot::MemoryMapEntry;
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;

mod boot;

static mut MEM_MAP: [MemoryMapEntry; 64] =
    [MemoryMapEntry { base: PhysAddr::null(), length: 0, kind: MemoryType::Ram }; 64];
static mut MEM_MAP_COUNT: usize = 0;
static mut OPENSBI_DTB_ADDR: usize = 0;

unsafe extern "C" {
    static __kernel_pbase: u8;
    static __alloc_start: u8;
}

pub unsafe fn init_mem_map() -> &'static [MemoryMapEntry] {
    let dtb_pa = unsafe { OPENSBI_DTB_ADDR };
    if dtb_pa == 0 {
        panic!("opensbi: No DTB address provided");
    }
    let fdt =
        unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8) }.expect("opensbi: Failed to parse FDT");
    // 解析内存映射 (仅在第一次调用时)
    unsafe {
        let mut count = 0;
        // 1. 获取保留区域
        let mut reserved_regions = [(0usize, 0usize, MemoryType::Reserved); 16];
        let mut reserved_count = 0;

        // DTB
        let fdt_size = fdt.total_size();
        let start = dtb_pa & !(PGSIZE - 1);
        let end = (dtb_pa + fdt_size + PGSIZE - 1) & !(PGSIZE - 1);
        reserved_regions[reserved_count] = (start, end - start, MemoryType::Reclaimable);
        reserved_count += 1;

        // Initrd
        if let Some(chosen) = fdt.find_node("/chosen") {
            let start = chosen.property("linux,initrd-start").and_then(|p| {
                #[cfg(target_pointer_width = "64")]
                if p.value.len() == 8 {
                    Some(u64::from_be_bytes(p.value.try_into().unwrap()) as usize)
                } else {
                    None
                }
                #[cfg(target_pointer_width = "32")]
                if p.value.len() == 4 {
                    Some(u32::from_be_bytes(p.value.try_into().unwrap()) as usize)
                } else {
                    None
                }
            });
            let end = chosen.property("linux,initrd-end").and_then(|p| {
                #[cfg(target_pointer_width = "64")]
                if p.value.len() == 8 {
                    Some(u64::from_be_bytes(p.value.try_into().unwrap()) as usize)
                } else {
                    None
                }
                #[cfg(target_pointer_width = "32")]
                if p.value.len() == 4 {
                    Some(u32::from_be_bytes(p.value.try_into().unwrap()) as usize)
                } else {
                    None
                }
            });

            if let (Some(s), Some(e)) = (start, end) {
                if e > s {
                    let start = s & !(PGSIZE - 1);
                    let end = (e + PGSIZE - 1) & !(PGSIZE - 1);
                    reserved_regions[reserved_count] =
                        (start, end - start, MemoryType::Reclaimable);
                    reserved_count += 1;
                    log!("opensbi: Initrd at {:#x} - {:#x}", start, end);
                }
            }
        }

        // /reserved-memory
        if let Some(res_mem) = fdt.find_node("/reserved-memory") {
            for child in res_mem.children() {
                if let Some(reg) = child.reg().and_then(|mut r| r.next()) {
                    let start = (reg.starting_address as usize) & !(PGSIZE - 1);
                    let size = reg.size.unwrap_or(0);
                    let end = (reg.starting_address as usize + size + PGSIZE - 1) & !(PGSIZE - 1);
                    reserved_regions[reserved_count] = (start, end - start, MemoryType::Reserved);
                    reserved_count += 1;
                    if reserved_count >= 16 {
                        break;
                    }
                }
            }
        }

        // 2. 添加所有保留区域到 MEM_MAP
        for i in 0..reserved_count {
            let (base, size, kind) = reserved_regions[i];
            if count < 64 {
                MEM_MAP[count] = MemoryMapEntry { base: PhysAddr::from(base), length: size, kind };
                count += 1;
            }
        }

        // 3. 处理 RAM，剔除保留区域
        let process_region = |r_base: usize, r_size: usize, count: &mut usize| {
            let start = (r_base + PGSIZE - 1) & !(PGSIZE - 1);
            let end = (r_base + r_size) & !(PGSIZE - 1);

            if end <= start {
                return;
            }

            let base = start;
            let size = end - start;

            // 我们采用“切割”法：对每个 RAM region，减去所有 reserved regions
            // 这需要一个临时的 ranges 列表
            let mut ranges = [(base, size); 8]; // 每个 RAM region 最多被切分成 8 块
            let mut range_count = 1;

            for r_idx in 0..reserved_count {
                let (r_base, r_size, _) = reserved_regions[r_idx];
                let r_end = r_base + r_size;

                let mut next_ranges = [(0, 0); 8];
                let mut next_count = 0;

                for k in 0..range_count {
                    let (c_base, c_size) = ranges[k];
                    let c_end = c_base + c_size;

                    // Check overlap
                    let max_start = core::cmp::max(c_base, r_base);
                    let min_end = core::cmp::min(c_end, r_end);

                    if max_start < min_end {
                        // Overlap found
                        // Part before
                        if c_base < max_start {
                            next_ranges[next_count] = (c_base, max_start - c_base);
                            next_count += 1;
                        }
                        // Part after
                        if c_end > min_end {
                            next_ranges[next_count] = (min_end, c_end - min_end);
                            next_count += 1;
                        }
                    } else {
                        // No overlap, keep original
                        if next_count < 8 {
                            next_ranges[next_count] = (c_base, c_size);
                            next_count += 1;
                        }
                    }
                }
                ranges = next_ranges;
                range_count = next_count;
            }

            // Add resulting RAM ranges
            for k in 0..range_count {
                let (base, size) = ranges[k];
                if size > 0 && *count < 64 {
                    log!(
                        "opensbi: Found RAM: {:#x} - {:#x} ({} MB)",
                        base,
                        base + size,
                        size / 1024 / 1024
                    );
                    MEM_MAP[*count] = MemoryMapEntry {
                        base: PhysAddr::from(base),
                        length: size,
                        kind: MemoryType::Ram,
                    };
                    *count += 1;
                }
            }
        };

        for node in fdt.all_nodes() {
            if node.name.starts_with("memory@") {
                if let Some(reg) = node.property("reg") {
                    let data = reg.value;
                    // Support both #address-cells=1/2 and #size-cells=1/2
                    if data.len() == 16 {
                        // Assume 2 cells for base and 2 cells for size (64-bit)
                        let r_base = u64::from_be_bytes(data[0..8].try_into().unwrap()) as usize;
                        let r_size = u64::from_be_bytes(data[8..16].try_into().unwrap()) as usize;
                        process_region(r_base, r_size, &mut count);
                    } else if data.len() == 8 {
                        // Assume 1 cell for base and 1 cell for size (32-bit)
                        let r_base = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
                        let r_size = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                        process_region(r_base, r_size, &mut count);
                    }
                }
            }
        }

        if count == 0 {
            // Fallback: use fdt-rs helper but it might fail on some DTBs
            for region in fdt.memory().regions() {
                process_region(
                    region.starting_address as usize,
                    region.size.unwrap_or(0),
                    &mut count,
                );
            }
        }
        MEM_MAP_COUNT = count;
    }
    unsafe { &MEM_MAP[..MEM_MAP_COUNT] }
}

pub unsafe fn init() {
    let dtb_pa = unsafe { OPENSBI_DTB_ADDR };

    let fdt =
        unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8) }.expect("opensbi: Failed to parse FDT");
    // 确定 CPU 数量
    let cpu_count = fdt.cpus().count();
    log!("opensbi: Detected {} CPUs from DTB", cpu_count);
    // 确定 initrd 和命令行
    let mut initrd_addr = None;
    let cmdline = fdt.chosen().bootargs();
    log!("opensbi: Kernel cmdline from DTB: {:?}", cmdline);
    if let Some(chosen_node) = fdt.find_node("/chosen") {
        if let (Some(start), Some(end)) =
            (chosen_node.property("linux,initrd-start"), chosen_node.property("linux,initrd-end"))
        {
            let parse_be = |data: &[u8]| -> usize {
                #[cfg(target_pointer_width = "64")]
                if data.len() == 8 {
                    return u64::from_be_bytes(data.try_into().unwrap()) as usize;
                }
                #[cfg(target_pointer_width = "32")]
                if data.len() == 4 {
                    return u32::from_be_bytes(data.try_into().unwrap()) as usize;
                }
                0
            };

            let start_val = parse_be(start.value);
            let end_val = parse_be(end.value);

            if start_val != 0 && end_val > start_val {
                let size = end_val - start_val;
                // Currently identity mapping
                initrd_addr = Some((VirtAddr::from(start_val), size));
                log!(
                    "opensbi: Found initrd in DTB: {:#x} - {:#x} ({} KB)",
                    start_val,
                    end_val,
                    size / 1024
                );
            }
        }
    }
    unsafe {
        init_mem_map();
    }

    let pbase = &raw const __kernel_pbase as usize;
    let pend = &raw const __alloc_start as usize;
    let kernel_size = pend - pbase;
    let fdt_size = fdt.total_size();
    log!(
        "opensbi: Kernel at {:#x} - {:#x} ({} KB), DTB size {} KB",
        pbase,
        pend,
        kernel_size / 1024,
        fdt_size / 1024
    );

    BOOT_LOADER_INFO.call_once(|| BootLoaderInfo {
        dtb_addr: Some((VirtAddr::from(dtb_pa), fdt_size)),
        rsdp_addr: None,
        hhdm_offset: 0,
        memory_map: unsafe { &MEM_MAP[..MEM_MAP_COUNT] },
        framebuffer: None,
        kernel_address: (PhysAddr::from(pbase), VirtAddr::from(pbase)),
        kernel_size,
        initrd_addr,
        cmdline,
        cpu_count,
    });
}

pub fn set_boot_info(dtb_pa: usize) {
    unsafe {
        OPENSBI_DTB_ADDR = dtb_pa;
    }
}

pub fn bootstrap() {
    hal::platform::bootstrap_cpus();
}
