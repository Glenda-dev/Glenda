use super::KSTACK_PAGES;
use super::scheduler;
use super::{TCB, ThreadState};
use crate::boot::{BootInfo, UntypedDesc};
use crate::cap::CNODE_BITS;
use crate::cap::CNode;
use crate::cap::Capability;
use crate::cap::rights;
use crate::hal;
use crate::hal::mem::{PGSIZE, PageTable, PteFlags, PtePerms};
use crate::initrd;
use crate::mem::pmem;
use crate::mem::{
    HEAP_SIZE, HEAP_VA, RES_VA_BASE, STACK_SIZE, STACK_VA, TRAMPOLINE_VA, TRAPFRAME_VA, UTCB_VA,
};
use crate::mem::{PhysAddr, VirtAddr};
use crate::printk;

pub const NULL_SLOT: usize = 0;
pub const CSPACE_SLOT: usize = 1;
pub const VSPACE_SLOT: usize = 2;
pub const TCB_SLOT: usize = 3;
pub const CONSOLE_SLOT: usize = 6;
pub const UTCB_SLOT: usize = 7;
pub const INITRD_SLOT: usize = 8;
pub const PLATFORM_SLOT: usize = 9;

pub const BOOTINFO_SLOT_START: usize = 32;

pub const BOOTINFO_VA: usize = RES_VA_BASE; // Bootinfo映射地址
pub const INITRD_VA: usize = BOOTINFO_VA + PGSIZE; // Initrd 映射地址 (Root Task)

unsafe extern "C" {
    static __trampoline: u8;
}
struct RootCaps {
    vspace: Capability,
    cspace: Capability,
    tcb: Capability,
    utcb: Capability,
    tf: Capability,
    kstack: Capability,
    bootinfo: Capability,
    console: Capability,
}

fn alloc_root_caps() -> RootCaps {
    RootCaps {
        vspace: pmem::alloc_pagetable_cap(2).expect("Failed to alloc root VSpace"),
        cspace: pmem::alloc_cnode_cap().expect("Failed to alloc root CSpace"),
        tcb: pmem::alloc_tcb_cap().expect("Failed to alloc root TCB"),
        utcb: pmem::alloc_frame_cap(1).expect("Failed to alloc root UTCB"),
        tf: pmem::alloc_frame_cap(1).expect("Failed to alloc root TrapFrame"),
        kstack: pmem::alloc_frame_cap(KSTACK_PAGES).expect("Failed to alloc root Kernel Stack"),
        bootinfo: pmem::alloc_frame_cap(1).expect("Failed to alloc root BootInfo"),
        console: Capability::create_console(rights::ALL),
    }
}

fn setup_root_vspace(vspace: &mut PageTable, caps: &RootCaps, root_task: &initrd::ProcPayload) {
    init_vspace(
        vspace,
        caps.tf.obj_ptr().to_pa(),
        caps.utcb.obj_ptr().to_pa(),
        caps.bootinfo.obj_ptr().to_pa(),
    );
    root_task.map(vspace);
}

fn fill_root_cspace(cspace: &mut CNode, caps: &RootCaps) {
    cspace.insert(CSPACE_SLOT, &caps.cspace);
    cspace.insert(VSPACE_SLOT, &caps.vspace);
    cspace.insert(TCB_SLOT, &caps.tcb);
    cspace.insert(UTCB_SLOT, &caps.utcb);
    cspace.insert(CONSOLE_SLOT, &caps.console);

    let initrd_range = hal::platform::initrd().expect("Initrd range not found");
    let initrd_start = initrd_range.start.align_down(PGSIZE);
    let initrd_page_count = (initrd_range.size + PGSIZE - 1) / PGSIZE;
    let initrd_cap = Capability::create_frame(
        initrd_start,
        initrd_page_count,
        rights::READ | rights::WRITE | rights::GRANT,
    );
    cspace.insert(INITRD_SLOT, &initrd_cap);

    match hal::platform::range() {
        Some(range) => {
            let platform_start = range.start.align_down(PGSIZE);
            let platform_page_count = (range.size + PGSIZE - 1) / PGSIZE;
            let platform_cap = Capability::create_frame(
                platform_start,
                platform_page_count,
                rights::READ | rights::WRITE | rights::GRANT,
            );
            cspace.insert(PLATFORM_SLOT, &platform_cap);
        }
        None => printk!("proc: Warning: Platform range not found\n"),
    }
}

fn start_root_task(tcb: &mut TCB, entry_point: usize, stack_top: usize) {
    tcb.set_priority(253);
    tcb.set_registers(entry_point, stack_top);
    tcb.state = ThreadState::Ready;
    scheduler::add_thread(tcb);
    printk!("proc: Root Task created. Entry: {:#x}, SP: {:#x}\n", entry_point, stack_top);
}

/// 初始化进程子系统并创建 Root Task
pub fn init() {
    let root_task = initrd::get_root_task().expect("proc: Root task not found");
    let (entry_point, stack_top) = root_task.info();

    // 1. Allocate Capabilities
    let caps = alloc_root_caps();

    // 2. Setup TCB basic fields
    let tcb = caps.tcb.obj_ptr().as_mut::<TCB>();

    // 3. Setup VSpace
    let pt_pa = caps.vspace.obj_ptr().to_pa();
    let mut vspace = PageTable::from_addr(pt_pa);
    setup_root_vspace(&mut vspace, &caps, root_task);

    // 4. Setup BootInfo
    let bootinfo = caps.bootinfo.obj_ptr().as_mut::<BootInfo>();
    init_bootinfo(bootinfo);

    // 5. Setup CSpace
    let mut cspace = CNode::from_addr(caps.cspace.obj_ptr());
    init_cspace(&mut cspace, bootinfo);
    fill_root_cspace(&mut cspace, &caps);

    // 6. Configure TCB resources
    tcb.configure(
        Some(&caps.cspace),
        Some(&caps.vspace),
        Some(&caps.utcb),
        Some(&caps.tf),
        Some(&caps.kstack),
    );

    // 7. Start Task
    start_root_task(tcb, entry_point, stack_top);

    //vspace.debug_print();
}
/*
用户地址空间布局：
trampoline  (1 page) 映射在最高地址
trapframe   (1 page)
UTCB        (1 page)
ustack      (N pages)
------------
BootInfo    (1 page)  0x40000000
Initrd      (N pages) 0x40001000
————————————
heap        (M pages) 0x20000000
-------------
code + data (N pages)
empty space (1 page) 最低的4096字节 不分配物理页，同时不可访问
*/

fn init_vspace(
    vspace: &mut PageTable,
    tf_paddr: PhysAddr,
    utcb_paddr: PhysAddr,
    bootinfo_paddr: PhysAddr,
) {
    printk!("proc: Setting up Root Task VSpace at {:#x}\n", vspace as *const _ as usize);
    // 1. 映射 Trampoline (最高地址)
    // 物理地址是 vector::user_vector 的地址 (需对齐)
    // 注意：Trampoline 代码运行在 S 态 (user_return/user_vector)，
    // 因此不能设置 USER 权限 (S 态无法执行 U 页面代码)
    let tramp_pa = PhysAddr::from(unsafe { &__trampoline as *const u8 as usize });
    vspace.map_with_alloc(
        VirtAddr::from(TRAMPOLINE_VA),
        tramp_pa,
        PGSIZE,
        PteFlags::from(PtePerms::READ | PtePerms::EXECUTE),
    );
    // 2. 映射 TrapFrame (Trampoline 下方)
    // TrapFrame 仅由 S 态的 user_vector/user_return 访问
    vspace.map_with_alloc(
        VirtAddr::from(TRAPFRAME_VA),
        tf_paddr,
        PGSIZE,
        PteFlags::from(PtePerms::READ | PtePerms::WRITE),
    );

    // 映射 UTCB 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(UTCB_VA),
        utcb_paddr,
        PGSIZE,
        PteFlags::from(PtePerms::USER | PtePerms::READ | PtePerms::WRITE),
    );

    // 映射 BootInfo 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(BOOTINFO_VA),
        bootinfo_paddr,
        PGSIZE,
        PteFlags::from(PtePerms::USER | PtePerms::READ), // 只读
    );

    // 映射 Initrd 到固定位置
    let range = hal::platform::initrd().expect("Initrd range not found");
    vspace.map_with_alloc(
        VirtAddr::from(INITRD_VA),
        range.start.align_down(PGSIZE),
        range.size,
        PteFlags::from(PtePerms::USER | PtePerms::READ),
    );

    let stack_va_start = STACK_VA;
    let stack_size = STACK_SIZE;
    let stack_pages = stack_size / PGSIZE;
    for i in 1..=stack_pages {
        let frame = pmem::alloc_frame_cap(1).expect("Failed to alloc user stack");
        let va = VirtAddr::from(stack_va_start - i * PGSIZE);
        vspace.map_with_alloc(
            va,
            frame.obj_ptr().to_pa(),
            PGSIZE,
            PteFlags::from(PtePerms::USER | PtePerms::READ | PtePerms::WRITE),
        );
        core::mem::forget(frame);
    }

    // 映射用户堆 (1MB)
    // HEAP_VA = 0x2000_0000 (Defined in libglenda-rs/src/crt0.rs)
    let heap_va_start = HEAP_VA;
    let heap_size = HEAP_SIZE; // 1MB
    let heap_pages = heap_size / PGSIZE;

    for i in 0..heap_pages {
        let frame = pmem::alloc_frame_cap(1).expect("Failed to alloc user heap");
        let va = VirtAddr::from(heap_va_start + i * PGSIZE);
        vspace.map_with_alloc(
            va,
            frame.obj_ptr().to_pa(),
            PGSIZE,
            PteFlags::from(PtePerms::USER | PtePerms::READ | PtePerms::WRITE),
        );
        core::mem::forget(frame);
    }
}

/// 填充 Root CNode
/// 将所有空闲物理内存作为 Untyped Capability 授予 Root Task
fn init_cspace(cnode: &mut CNode, bootinfo: &mut BootInfo) {
    let mut slot = BOOTINFO_SLOT_START;

    // 记录 Untyped 区域的起始槽位
    bootinfo.mmio.start = slot;

    // 1. mmio Region (OpenSBI, Kernel, etc.)
    for mmio_region in hal::platform::mmio_ranges() {
        let mmio_size = mmio_region.size;
        if mmio_size > 0 {
            let cap = Capability::create_mmio(mmio_region.start, mmio_size, rights::ALL);
            cnode.insert(slot, &cap);

            if bootinfo.mmio_count < bootinfo.mmio_list.len() {
                bootinfo.mmio_list[bootinfo.mmio_count] =
                    UntypedDesc { paddr: mmio_region.start, size: mmio_size };
                bootinfo.mmio_count += 1;
            }
            slot += 1;
        }
    }

    bootinfo.mmio.end = slot;

    // 记录 Untyped 区域的起始槽位
    bootinfo.untyped.start = slot;

    // 1. mmio Region (OpenSBI, Kernel, etc.)
    for untyped_region in pmem::get_untyped() {
        let untyped_size = (untyped_region.end - untyped_region.start).as_usize();
        if untyped_size > 0 {
            let cap = Capability::create_untyped(
                untyped_region.start,
                untyped_size / PGSIZE,
                rights::ALL,
            );
            cnode.insert(slot, &cap);

            if bootinfo.untyped_count < bootinfo.untyped_list.len() {
                bootinfo.untyped_list[bootinfo.untyped_count] =
                    UntypedDesc { paddr: untyped_region.start, size: untyped_size };
                bootinfo.untyped_count += 1;
            }
            slot += 1;
        }
    }

    bootinfo.untyped.end = slot;

    // 插入 IRQ Handler Capabilities
    // 假设系统支持 64 个中断 (与 IRQ_TABLE 大小一致)
    bootinfo.irq.start = slot;
    for irq in 0..64 {
        let cap = Capability::create_irqhandler(irq, rights::ALL);
        cnode.insert(slot, &cap);
        slot += 1;
    }
    bootinfo.irq.end = slot;

    // 记录空闲槽位
    bootinfo.empty.start = slot;
    bootinfo.empty.end = 1 << CNODE_BITS; // CNode size bits = 12
}

fn init_bootinfo(bootinfo: &mut BootInfo) {
    // 初始化 BootInfo
    *bootinfo = BootInfo::new();

    // 填充 PLATFORM 信息
    if let Some(range) = hal::platform::range() {
        bootinfo.info_paddr = range.start.as_usize();
        bootinfo.info_size = range.size;
    }

    // 填充启动参数
    if let Some(args) = hal::platform::bootargs() {
        let bytes = args.as_bytes();
        let len = core::cmp::min(bytes.len(), bootinfo.cmdline.len() - 1);
        bootinfo.cmdline[..len].copy_from_slice(&bytes[..len]);
        bootinfo.cmdline[len] = 0;
    }
}
