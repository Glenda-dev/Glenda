use crate::boot::uefi::{MEM_MAP, MEM_MAP_COUNT};
use crate::hal;
use uefi::Identify;
use uefi::proto::Protocol;

pub const RISCV_BOOT_PROTOCOL_GUID: uefi::Guid =
    uefi::guid!("415fcd91-5f21-4f11-9f65-22e391bb37e7");

#[repr(C)]
struct RiscvBootProtocol {
    revision: u64,
    get_boot_hartid: unsafe extern "efiapi" fn(
        this: *const RiscvBootProtocol,
        hartid: *mut usize,
    ) -> uefi::Status,
}

unsafe impl Identify for RiscvBootProtocol {
    const GUID: uefi::Guid = RISCV_BOOT_PROTOCOL_GUID;
}

impl Protocol for RiscvBootProtocol {}

pub fn get_boot_hartid() -> usize {
    let mut hartid = 0;
    // 尝试获取 EFI_RISCV_BOOT_PROTOCOL
    if let Ok(proto_handle) = uefi::boot::get_handle_for_protocol::<RiscvBootProtocol>() {
        if let Ok(proto) = unsafe {
            uefi::boot::open_protocol::<RiscvBootProtocol>(
                uefi::boot::OpenProtocolParams {
                    handle: proto_handle,
                    agent: uefi::boot::image_handle(),
                    controller: None,
                },
                uefi::boot::OpenProtocolAttributes::GetProtocol,
            )
        } {
            let proto_ptr = core::ptr::from_ref::<RiscvBootProtocol>(&*proto);
            let status = unsafe { (proto.get_boot_hartid)(proto_ptr, &mut hartid) };
            if status.is_success() {
                return hartid;
            }
        }
    }
    // 回退默认 0
    panic!("Failed to get boot hart ID from EFI_RISCV_BOOT_PROTOCOL");
}

pub fn jump_to_kernel(hartid: usize) {
    // 设置 CPU ID 并建立页表
    hal::cpu::set_cpuid(hartid);

    let regions = unsafe { &MEM_MAP[..MEM_MAP_COUNT] };
    let satp = unsafe { hal::mem::setup_boot_pagetable(regions) };
    unsafe { hal::mem::activate_vspace(satp) };

    // 跳转到内核主入口
    crate::glenda_boot();
}

pub unsafe fn init() {
    // Already populated in efi_main
}

pub fn bootstrap() {
    let hartid = hal::cpu::cpu_id();
    jump_to_kernel(hartid);
}
