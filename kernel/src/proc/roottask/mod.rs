mod bootinfo;
mod layout;

pub use layout::STACK_VA;

use super::scheduler;
use super::{TCB, ThreadState};
use crate::cap::{CNODE_BITS, ROOT_BITS};
use crate::cap::{CNode, Capability, Rights};
use crate::hal;
use crate::hal::irq::MAX_IRQS;
use crate::hal::mem::PageTable;
use crate::hal::mem::{KSTACK_PAGES, PGSIZE};
use crate::initrd;
use crate::irq::IRQ;
use crate::mem::pmem;
use crate::mem::{Perms, PhysAddr, PhysFrame, VirtAddr};
use crate::mem::{TRAPFRAME_VA, UTCB_VA};
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use bootinfo::BootInfo;
use layout::*;
struct RootCaps {
    vspace: Capability,
    cspace: Capability,
    tcb: Capability,
    utcb: Capability,
    tf: Capability,
    kstack: Capability,
    bootinfo: Capability,
    console: Capability,
    untyped_cspace: Capability,
    mmio_cspace: Capability,
    irq_cspace: Capability,
}

fn alloc_root_caps() -> RootCaps {
    RootCaps {
        vspace: pmem::alloc_vspace_cap().expect("Failed to alloc root VSpace"),
        cspace: pmem::alloc_cnode_cap(ROOT_BITS).expect("Failed to alloc root CSpace"),
        tcb: pmem::alloc_tcb_cap().expect("Failed to alloc root TCB"),
        utcb: pmem::alloc_frame_cap(1).expect("Failed to alloc root UTCB"),
        tf: pmem::alloc_frame_cap(1).expect("Failed to alloc root TrapFrame"),
        kstack: pmem::alloc_frame_cap(KSTACK_PAGES).expect("Failed to alloc root Kernel Stack"),
        bootinfo: pmem::alloc_frame_cap(1).expect("Failed to alloc root BootInfo"),
        console: Capability::create_console(Rights::ALL),
        untyped_cspace: pmem::alloc_cnode_cap(ROOT_BITS - CNODE_BITS)
            .expect("Failed to alloc Untyped CNode"),
        mmio_cspace: pmem::alloc_cnode_cap(ROOT_BITS - CNODE_BITS)
            .expect("Failed to alloc MMIO CNode"),
        irq_cspace: pmem::alloc_cnode_cap(ROOT_BITS - CNODE_BITS)
            .expect("Failed to alloc IRQ CNode"),
    }
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
    let pt_pa = caps.vspace.paddr();
    let vspace = PageTable::from_addr(pt_pa);
    init_vspace(vspace, caps.tf.paddr(), caps.utcb.paddr(), caps.bootinfo.paddr());
    root_task.map(vspace);

    // 4. Setup BootInfo
    let bootinfo = caps.bootinfo.obj_ptr().as_mut::<BootInfo>();
    init_bootinfo(bootinfo);

    // 5. Setup CSpace
    let cspace = caps.cspace.obj_ptr().as_mut::<CNode>();
    init_cspace(cspace, &caps, bootinfo);

    // 6. Configure TCB resources
    tcb.configure(
        Some(&caps.cspace),
        Some(&caps.vspace),
        Some(&caps.utcb),
        Some(&caps.tf),
        Some(&caps.kstack),
    );
    tcb.set_priority(ROOT_TASK_PRIORITY);
    tcb.set_registers(entry_point, stack_top);
    tcb.state = ThreadState::Ready;
    scheduler::add_thread(tcb);
    printk!("proc: Root Task created. Entry: {:#x}, SP: {:#x}\n", entry_point, stack_top);

    //cspace.debug_print();
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
    // 2. 映射 TrapFrame (Trampoline 下方)
    // TrapFrame 仅由 S 态的 user_vector/user_return 访问
    vspace.map_with_alloc(
        VirtAddr::from(TRAPFRAME_VA),
        tf_paddr,
        PGSIZE,
        Perms::READ | Perms::WRITE,
    );

    // 映射 UTCB 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(UTCB_VA),
        utcb_paddr,
        PGSIZE,
        Perms::USER | Perms::READ | Perms::WRITE,
    );

    // 映射 BootInfo 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(BOOTINFO_VA),
        bootinfo_paddr,
        PGSIZE,
        Perms::USER | Perms::READ, // 只读
    );

    // 映射 Initrd 到固定位置
    let range = hal::platform::initrd().expect("Initrd range not found");
    vspace.map_with_alloc(
        VirtAddr::from(INITRD_VA),
        range.start.align_down(PGSIZE),
        range.size,
        Perms::USER | Perms::READ,
    );

    // 映射用户栈
    let stack_va_start = STACK_VA;
    let stack_pages = STACK_PAGES;
    for i in 1..=stack_pages {
        let frame_pa = pmem::alloc_page().expect("Failed to alloc user stack");
        let va = VirtAddr::from(stack_va_start - i * PGSIZE);
        vspace.map_with_alloc(va, frame_pa, PGSIZE, Perms::USER | Perms::READ | Perms::WRITE);
    }

    // 映射用户堆 (256KB)
    // HEAP_VA = 0x2000_0000 (Defined in libglenda-rs/src/runtime.rs)
    let heap_va_start = HEAP_VA;
    let heap_pages = HEAP_PAGES;
    for i in 0..heap_pages {
        let frame_pa = pmem::alloc_page().expect("Failed to alloc user heap");
        let va = VirtAddr::from(heap_va_start + i * PGSIZE);
        vspace.map_with_alloc(va, frame_pa, PGSIZE, Perms::USER | Perms::READ | Perms::WRITE);
    }

    // 设置 Trampoline 映射
    vspace.setup().expect("Failed to setup VSpace for root task");
}
fn init_bootinfo(bootinfo: &mut BootInfo) {
    // 初始化 BootInfo
    *bootinfo = BootInfo::new();

    // 填充 PLATFORM 信息
    if let Some(range) = hal::platform::range() {
        bootinfo.info_desc = range;
    }

    // 填充启动参数
    if let Some(args) = hal::platform::bootargs() {
        let bytes = args.as_bytes();
        let len = core::cmp::min(bytes.len(), bootinfo.cmdline.len() - 1);
        bootinfo.cmdline[..len].copy_from_slice(&bytes[..len]);
        bootinfo.cmdline[len] = 0;
    }
}
fn init_cspace(cspace: &mut CNode, caps: &RootCaps, bootinfo: &mut BootInfo) {
    cspace.insert(CSPACE_SLOT, &caps.cspace);
    cspace.insert(VSPACE_SLOT, &caps.vspace);
    cspace.insert(TCB_SLOT, &caps.tcb);
    cspace.insert(CONSOLE_SLOT, &caps.console);
    cspace.insert(UNTYPED_SLOT, &caps.untyped_cspace);
    cspace.insert(MMIO_SLOT, &caps.mmio_cspace);
    cspace.insert(IRQ_SLOT, &caps.irq_cspace);

    match hal::platform::range() {
        Some(range) => {
            let frame = PhysFrame { paddr: range.start, pages: range.size / PGSIZE };
            let platform_cap =
                Capability::create_frame(&frame, Rights::READ | Rights::WRITE | Rights::GRANT);
            cspace.insert(PLATFORM_SLOT, &platform_cap);
        }
        None => printk!("proc: {}Warning{}: Platform range not found\n", ANSI_YELLOW, ANSI_RESET),
    }

    // === 1. MMIO Caps (Stored in MMIO CNode at slot 7) ===
    let mut slot = 1;
    let mmio_cnode = caps.mmio_cspace.obj_ptr().as_mut::<CNode>();
    match hal::platform::mmio_ranges() {
        Some(ranges) => {
            for mmio_region in ranges {
                let mmio_size = mmio_region.size;
                if mmio_size > 0 {
                    let cap = Capability::create_mmio(&mmio_region, Rights::ALL);
                    // 插入到 MMIO 子 CNode
                    mmio_cnode.insert(slot, &cap);

                    if bootinfo.mmio_count < bootinfo.mmio_list.len() {
                        bootinfo.mmio_list[bootinfo.mmio_count] = *mmio_region;
                        bootinfo.mmio_count += 1;
                    }
                    slot += 1;
                }
            }
        }
        None => printk!("proc: {}Warning{}: No MMIO ranges found\n", ANSI_YELLOW, ANSI_RESET),
    }

    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let slot = 1;
    let untyped_cnode = caps.untyped_cspace.obj_ptr().as_mut::<CNode>();
    let untyped_region = pmem::get_untyped();
    let cap = Capability::create_untyped(&untyped_region, Rights::ALL);
    // 插入到 Untyped 子 CNode
    untyped_cnode.insert(slot, &cap);

    if bootinfo.untyped_count < bootinfo.untyped_list.len() {
        bootinfo.untyped_list[bootinfo.untyped_count] = untyped_region;
        bootinfo.untyped_count += 1;
    }

    // === 3. IRQ Caps (Stored in Root CNode L1 directly, starting slot 8) ===
    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let mut slot = 1;
    let irq_cnode = caps.irq_cspace.obj_ptr().as_mut::<CNode>();
    for irq in 0..MAX_IRQS {
        let irq_obj = IRQ::new(irq);
        let cap = Capability::create_irqhandler(&irq_obj, Rights::ALL);
        // 插入到 IRQ 子 CNode
        irq_cnode.insert(slot, &cap);
        slot += 1;
    }
    bootinfo.irq_count = MAX_IRQS;
}
