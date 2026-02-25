mod arch;

use super::BOOT_LOADER_INFO;
use crate::boot::{BootLoaderInfo, MemoryMapEntry};
use crate::hal;
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;
use limine::memory_map::EntryType;
use limine::request::*;

pub const MAX_MEM_ENTRIES: usize = 256;
static mut MEM_MAP: [MemoryMapEntry; MAX_MEM_ENTRIES] =
    [MemoryMapEntry { base: PhysAddr::null(), length: 0, kind: MemoryType::Reserved };
        MAX_MEM_ENTRIES];
static mut MEM_MAP_COUNT: usize = 0;

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[used]
#[unsafe(link_section = ".requests")]
static DTB_REQUEST: DeviceTreeBlobRequest = DeviceTreeBlobRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static PAGING_MODE_REQUEST: PagingModeRequest =
    PagingModeRequest::new().with_mode(limine::paging::Mode::MIN);

#[used]
#[unsafe(link_section = ".requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static KERNEL_FILE_REQUEST: ExecutableFileRequest = ExecutableFileRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static BOOTARGS_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MP_REQUEST: MpRequest = MpRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

pub unsafe fn init() {
    let mp_response = MP_REQUEST.get_response().expect("limine: MP request failed!");
    let bsp_id = arch::bspid(mp_response);
    log!("limine: SMP info found. BSP ID: {}", bsp_id);
    hal::cpu::set_cpuid(bsp_id);
    let hhdm_offset = HHDM_REQUEST.get_response().map(|res| res.offset() as usize).unwrap_or(0);
    let kernel_addr = KERNEL_ADDRESS_REQUEST
        .get_response()
        .map(|res| {
            (
                PhysAddr::from(res.physical_base() as usize),
                VirtAddr::from(res.virtual_base() as usize),
            )
        })
        .expect("limine: Kernel address request failed!");
    // Debug logging for critical boot info
    let (pbase, vbase) = kernel_addr;
    log!("limine: HHDM offset: {:#x}", hhdm_offset);
    log!("limine: Kernel PBase: {}, VBase: {}", pbase, vbase);

    let mut count = 0;
    if let Some(res) = MEMORY_MAP_REQUEST.get_response() {
        for entry in res.entries() {
            if count >= MAX_MEM_ENTRIES {
                break;
            }
            unsafe {
                MEM_MAP[count] = MemoryMapEntry {
                    base: PhysAddr::from(entry.base as usize),
                    length: entry.length as usize,
                    kind: match entry.entry_type {
                        EntryType::USABLE => MemoryType::Ram,
                        EntryType::RESERVED => MemoryType::Reserved,
                        EntryType::BAD_MEMORY => MemoryType::Reserved,
                        EntryType::ACPI_RECLAIMABLE => MemoryType::Reclaimable,
                        EntryType::BOOTLOADER_RECLAIMABLE => MemoryType::Reclaimable,
                        EntryType::ACPI_NVS => MemoryType::Mmio,
                        EntryType::FRAMEBUFFER => MemoryType::Mmio,
                        _ => MemoryType::Reserved,
                    },
                };
            }
            count += 1;
        }
    }
    unsafe { MEM_MAP_COUNT = count };
    let kernel_size =
        KERNEL_FILE_REQUEST.get_response().map(|res| res.file().size() as usize).unwrap_or(0);

    let initrd_addr =
        MODULE_REQUEST.get_response().and_then(|res| res.modules().first()).map(|file| {
            let va = file.addr() as usize;
            (VirtAddr::from(va), file.size() as usize)
        });

    let cmdline =
        BOOTARGS_REQUEST.get_response().map(|res| res.cmdline()).and_then(|s| s.to_str().ok());

    let mp_response = MP_REQUEST.get_response();
    let cpu_count = mp_response.map(|res| res.cpus().len()).unwrap_or(1);

    let framebuffer = FRAMEBUFFER_REQUEST
        .get_response()
        .and_then(|res| res.framebuffers().next())
        .map(|fb| crate::boot::FrameBufferInfo {
            address: VirtAddr::from(fb.addr() as usize),
            width: fb.width() as u32,
            height: fb.height() as u32,
            pitch: fb.pitch() as u32,
            bpp: fb.bpp() as u32,
        });

    BOOT_LOADER_INFO.call_once(|| BootLoaderInfo {
        dtb_addr: DTB_REQUEST.get_response().map(|res| (VirtAddr::from(res.dtb_ptr() as usize), 0)), // Limine doesn't provide size?
        rsdp_addr: RSDP_REQUEST.get_response().map(|res| VirtAddr::from(res.address() as usize)),
        hhdm_offset,
        memory_map: unsafe { &MEM_MAP[..MEM_MAP_COUNT] },
        framebuffer,
        kernel_address: kernel_addr,
        kernel_size,
        initrd_addr,
        cmdline,
        cpu_count,
    });
}

use limine::mp::Cpu;

unsafe extern "C" fn secondary_trampoline(cpu: &Cpu) -> ! {
    let cpuid = arch::cpuid(cpu);
    crate::glenda_secondary(cpuid)
}

pub fn bootstrap() {
    if let Some(response) = MP_REQUEST.get_response() {
        let bsp_id = arch::bspid(response);
        log!("limine: Bootstrapping from CPU {}", bsp_id);
        for cpu in response.cpus() {
            let cpuid = arch::cpuid(cpu);
            if cpuid != bsp_id {
                log!("limine: Starting CPU {}", cpuid);
                #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
                cpu.goto_address.write(secondary_trampoline);
                #[cfg(target_arch = "loongarch64")]
                {
                    // TODO
                }
            }
        }
    } else {
        log!("limine: No MP response found");
    }
}
