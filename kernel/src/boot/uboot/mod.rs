pub mod arch;

use crate::boot::BOOT_LOADER_INFO;
use crate::boot::{BootLoaderInfo, MemoryMapEntry};
use crate::hal;
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;

#[unsafe(no_mangle)]
static mut UBOOT_DTB_ADDR: usize = 0;

unsafe extern "C" {
    static __kernel_pbase: u8;
    static __alloc_start: u8;
}

pub const MAX_MEM_ENTRIES: usize = 256;
static mut MEM_MAP: [MemoryMapEntry; MAX_MEM_ENTRIES] =
    [MemoryMapEntry { base: PhysAddr::null(), length: 0, kind: MemoryType::Reserved };
        MAX_MEM_ENTRIES];
static mut MEM_MAP_COUNT: usize = 0;

pub unsafe fn init_mem_map() -> &'static [MemoryMapEntry] {
    let dtb_pa = unsafe { UBOOT_DTB_ADDR };
    if dtb_pa == 0 {
        return &[];
    }
    let fdt =
        unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8) }.expect("uboot: Failed to parse FDT");
    // 解析内存映射 (仅在第一次调用时)
    unsafe {
        if MEM_MAP_COUNT == 0 {
            let mut count = 0;
            // 1. 获取保留区域
            let mut reserved_regions = [(0usize, 0usize, MemoryType::Reserved); 16];
            let mut reserved_count = 0;

            // DTB
            let fdt_size = fdt.total_size();
            reserved_regions[reserved_count] = (dtb_pa, fdt_size, MemoryType::Reclaimable);
            reserved_count += 1;

            // Initrd
            if let Some(chosen) = fdt.find_node("/chosen") {
                let start = chosen.property("linux,initrd-start").and_then(|p| {
                    if p.value.len() == 8 {
                        Some(u64::from_be_bytes(p.value.try_into().unwrap()) as usize)
                    } else if p.value.len() == 4 {
                        Some(u32::from_be_bytes(p.value.try_into().unwrap()) as usize)
                    } else {
                        None
                    }
                });
                let end = chosen.property("linux,initrd-end").and_then(|p| {
                    if p.value.len() == 8 {
                        Some(u64::from_be_bytes(p.value.try_into().unwrap()) as usize)
                    } else if p.value.len() == 4 {
                        Some(u32::from_be_bytes(p.value.try_into().unwrap()) as usize)
                    } else {
                        None
                    }
                });

                if let (Some(s), Some(e)) = (start, end) {
                    if e > s {
                        reserved_regions[reserved_count] = (s, e - s, MemoryType::Reclaimable);
                        reserved_count += 1;
                        log!("uboot: Initrd at {:#x} - {:#x}", s, e);
                    }
                }
            }

            // /reserved-memory
            if let Some(res_mem) = fdt.find_node("/reserved-memory") {
                for child in res_mem.children() {
                    if let Some(reg) = child.reg().and_then(|mut r| r.next()) {
                        reserved_regions[reserved_count] = (
                            reg.starting_address as usize,
                            reg.size.unwrap_or(0),
                            MemoryType::Reserved,
                        );
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
                if count < MAX_MEM_ENTRIES {
                    MEM_MAP[count] =
                        MemoryMapEntry { base: PhysAddr::from(base), length: size, kind };
                    count += 1;
                }
            }

            // 3. 处理 RAM，剔除保留区域
            for region in fdt.memory().regions() {
                let base = region.starting_address as usize;
                let size = region.size.unwrap_or(0);

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
                    if size > 0 && count < MAX_MEM_ENTRIES {
                        log!(
                            "dtb: Found RAM: {:#x} - {:#x} ({} MB)",
                            base,
                            base + size,
                            size / 1024 / 1024
                        );
                        MEM_MAP[count] = MemoryMapEntry {
                            base: PhysAddr::from(base),
                            length: size,
                            kind: MemoryType::Ram,
                        };
                        count += 1;
                    }
                }
            }
            MEM_MAP_COUNT = count;
        }
    }
    unsafe { &MEM_MAP[..MEM_MAP_COUNT] }
}

pub fn init() {
    let dtb_pa = unsafe { UBOOT_DTB_ADDR };
    if dtb_pa == 0 {
        return;
    }

    // 解析设备树
    let fdt =
        unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8).expect("uboot: Failed to parse FDT") };

    // 确定 CPU 数量
    let cpu_count = fdt.cpus().count();

    // 确定 initrd 和命令行
    let mut initrd_addr = None;
    let cmdline = fdt.chosen().bootargs();

    if let Some(chosen_node) = fdt.find_node("/chosen") {
        if let (Some(start), Some(end)) =
            (chosen_node.property("linux,initrd-start"), chosen_node.property("linux,initrd-end"))
        {
            let parse_be = |data: &[u8]| -> usize {
                if data.len() == 8 {
                    u64::from_be_bytes(data.try_into().unwrap()) as usize
                } else if data.len() == 4 {
                    u32::from_be_bytes(data.try_into().unwrap()) as usize
                } else {
                    0
                }
            };

            let start_val = parse_be(start.value);
            let end_val = parse_be(end.value);

            if start_val != 0 && end_val > start_val {
                let size = end_val - start_val;
                // Currently identity mapping
                initrd_addr = Some((VirtAddr::from(start_val), size));
                log!(
                    "uboot: Found initrd in DTB: {:#x} - {:#x} ({} bytes)",
                    start_val,
                    end_val,
                    size
                );
            }
        }
    }

    let pbase = &raw const __kernel_pbase as usize;
    let pend = &raw const __alloc_start as usize;
    let kernel_size = pend - pbase;
    let fdt_size = fdt.total_size();

    BOOT_LOADER_INFO.call_once(|| BootLoaderInfo {
        dtb_addr: Some((VirtAddr::from(dtb_pa), fdt_size)),
        rsdp_addr: None,
        hhdm_offset: 0,
        memory_map: unsafe { &MEM_MAP[..MEM_MAP_COUNT] },
        kernel_address: (PhysAddr::from(pbase), VirtAddr::from(pbase)),
        kernel_size,
        initrd_addr,
        cmdline,
        cpu_count,
    });
}

pub fn bootstrap() {
    hal::platform::bootstrap_cpus();
}

pub unsafe fn bootstrap_kernel(hartid: usize, dtb_pa: usize) -> ! {
    // 0. 清零 BSS (主核负责)
    unsafe {
        unsafe extern "C" {
            static mut __bss_start: u8;
            static mut __bss_end: u8;
        }
        let start = &raw mut __bss_start;
        let end = &raw mut __bss_end;
        let len = (end as usize).saturating_sub(start as usize);
        if len > 0 {
            core::ptr::write_bytes(start, 0, len);
        }
    }

    // 0. 设置当前核 ID 到 tp 寄存器
    hal::cpu::set_cpuid(hartid);

    // 1. 保存 DTB 物理地址
    unsafe {
        UBOOT_DTB_ADDR = dtb_pa;
    }

    // 2. 通过 HAL 构造初始页表并开启 MMU
    log!("uboot: Setting up boot page tables...");

    let regions = unsafe { init_mem_map() };
    let satp = unsafe { hal::mem::setup_boot_pagetable(regions) };

    log!("uboot: Enabling MMU...");
    unsafe {
        hal::mem::activate_vspace(satp);
    }

    // 3. 跳转到内核入口
    log!("uboot: Jumping to kernel main...");
    crate::glenda_boot();
}

#[unsafe(no_mangle)]
pub unsafe fn uboot_secondary_bootstrap(hartid: usize) -> ! {
    // 复用主核建立的 BOOT_PAGE_TABLE
    let root_pa = PhysAddr::from(&raw const hal::mem::BOOT_PAGE_TABLE as usize);
    let satp = hal::mem::get_mmu_register(root_pa, 0);
    unsafe {
        hal::mem::activate_vspace(satp);
    }
    crate::glenda_secondary(hartid);
}

pub fn get_dtb_addr() -> Option<VirtAddr> {
    unsafe { if UBOOT_DTB_ADDR != 0 { Some(VirtAddr::from(UBOOT_DTB_ADDR)) } else { None } }
}
