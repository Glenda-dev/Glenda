use crate::boot::BOOT_LOADER_INFO;
use crate::boot::{BootLoaderInfo, MemoryMapEntry};
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;
use uefi::mem::memory_map::{MemoryMap, MemoryType as uefi_mem_type};
use uefi::table::cfg::ACPI2_GUID;
use uefi::{Handle, Status};

use uefi::proto::console::gop::GraphicsOutput;

pub mod arch;

pub use arch::{bootstrap, init};

pub const DEVICE_TREE_GUID: uefi::Guid = uefi::guid!("b1b621d5-f19c-41a5-830b-d9152c69aae0");

pub const MAX_MEM_ENTRIES: usize = 256;
pub static mut MEM_MAP: [MemoryMapEntry; MAX_MEM_ENTRIES] =
    [MemoryMapEntry { base: PhysAddr::from(0), length: 0, kind: MemoryType::Ram }; MAX_MEM_ENTRIES];
pub static mut MEM_MAP_COUNT: usize = 0;

pub fn find_dtb() -> Option<(VirtAddr, usize)> {
    // ... existing find_dtb ...
    uefi::system::with_config_table(|entries| {
        for entry in entries {
            if entry.guid == DEVICE_TREE_GUID {
                return Some((VirtAddr::from(entry.address as usize), 0));
            }
        }
        None
    })
}

pub fn find_rsdp() -> Option<VirtAddr> {
    // ... existing find_rsdp ...
    uefi::system::with_config_table(|entries| {
        for entry in entries {
            if entry.guid == ACPI2_GUID {
                return Some(VirtAddr::from(entry.address as usize));
            }
        }
        None
    })
}

pub fn collect_memory_map() -> usize {
    let mmap =
        uefi::boot::memory_map(uefi_mem_type::LOADER_DATA).expect("Failed to get memory map");

    let mut count = 0;
    for desc in mmap.entries() {
        let kind = match desc.ty {
            uefi_mem_type::CONVENTIONAL
            | uefi_mem_type::BOOT_SERVICES_CODE
            | uefi_mem_type::BOOT_SERVICES_DATA => MemoryType::Ram,
            _ => MemoryType::Reserved,
        };

        if count < MAX_MEM_ENTRIES {
            unsafe {
                MEM_MAP[count] = MemoryMapEntry {
                    base: PhysAddr::from(desc.phys_start as usize),
                    length: desc.page_count as usize * 4096,
                    kind,
                };
            }
            count += 1;
        }
    }
    unsafe { MEM_MAP_COUNT = count };
    count
}

#[unsafe(no_mangle)]
pub unsafe extern "efiapi" fn efi_main(
    _handle: Handle,
    system_table: *const core::ffi::c_void,
) -> Status {
    unsafe { uefi::table::set_system_table(system_table as *const _) };

    // 1. 获取启动核心 ID
    let hartid = arch::get_boot_hartid();

    // 2. 发现配置表 (DTB/RSDP)
    let dtb_addr = find_dtb();
    let rsdp_addr = find_rsdp();

    // 3. 收集内存映射
    let _count = collect_memory_map();

    // 4. 获取 Framebuffer
    let framebuffer = (|| {
        let handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
        let mut gop = unsafe {
            uefi::boot::open_protocol::<GraphicsOutput>(
                uefi::boot::OpenProtocolParams {
                    handle,
                    agent: uefi::boot::image_handle(),
                    controller: None,
                },
                uefi::boot::OpenProtocolAttributes::GetProtocol,
            )
            .ok()?
        };
        let info = gop.current_mode_info();
        let (width, height) = info.resolution();
        let stride = info.stride();
        Some(crate::boot::FrameBufferInfo {
            address: VirtAddr::from(gop.frame_buffer().as_mut_ptr() as usize),
            width: width as u32,
            height: height as u32,
            pitch: (stride * 4) as u32,
            bpp: 32,
        })
    })();

    // 5. 填充 BootLoaderInfo
    BOOT_LOADER_INFO.call_once(|| BootLoaderInfo {
        dtb_addr,
        rsdp_addr,
        hhdm_offset: 0,
        memory_map: unsafe { &MEM_MAP[..MEM_MAP_COUNT] },
        framebuffer,
        kernel_address: (PhysAddr::from(0), VirtAddr::from(0)),
        kernel_size: 0,
        initrd_addr: None,
        cmdline: None,
        cpu_count: 1,
    });

    // 5. 退出 Boot Services
    let _ = unsafe { uefi::boot::exit_boot_services(uefi_mem_type::LOADER_DATA) };

    // 6. 开启 MMU 并跳转到内核
    crate::glenda_boot(cpuid);

    // Should not return
    Status::SUCCESS
}
