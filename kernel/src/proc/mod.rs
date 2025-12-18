pub mod context;
pub mod payload;
pub mod scheduler;
pub mod thread;

pub use context::ProcContext;
pub use thread::{TCB, ThreadState, UTCB_VA};

use crate::cap::CNode;
use crate::cap::Capability;
use crate::cap::rights;
use crate::hart;
use crate::mem::addr;
use crate::mem::pmem;
use crate::mem::pte;
use crate::mem::{PGSIZE, PageTable, PhysFrame, VirtAddr};
use crate::printk;

/// 初始化进程子系统并创建 Root Task
pub fn init() {
    let root_task = payload::get_root_task().expect("proc: Root task not found");
    // 1. 加载 Root Task 的 ELF 文件 (获取入口点和段信息)
    let (entry_point, stack_top) = root_task.info();

    // 2. 手动分配 Root Task 的核心对象
    // 注意：这里直接调用内存分配器，因为此时还没有 Capability 限制
    let root_vspace_frame = PhysFrame::alloc().expect("Failed to alloc root VSpace");
    let root_cspace_frame = PhysFrame::alloc().expect("Failed to alloc root CSpace");
    let root_tcb_frame = PhysFrame::alloc().expect("Failed to alloc root TCB");
    let root_utcb_frame = PhysFrame::alloc().expect("Failed to alloc root UTCB");

    // 3. 构建 Root VSpace (页表)
    // 必须映射内核空间和 Root Task 自身的代码/数据段
    let mut vspace = PageTable::from_frame(&root_vspace_frame);
    vspace.map_kernel();
    root_task.map_segments(&mut vspace);
    let utcb_base = UTCB_VA;
    // 映射 UTCB 到固定位置
    vspace
        .map(utcb_base, root_utcb_frame.addr(), PGSIZE, pte::PTE_R | pte::PTE_W)
        .expect("Failed to map UTCB");

    // 4. 构建 Root CSpace (CNode)
    // 这是 Root Task 权力的来源。我们需要把所有剩余的物理内存
    // 转化为 Untyped Capability 并放入这个 CNode。
    let mut cspace = CNode::from_frame(&root_cspace_frame);
    populate_root_cnode(&mut cspace);

    // 5. 初始化 TCB
    // 这里我们将物理帧转换为内核对象引用
    let tcb = unsafe { &mut *(addr::phys_to_virt(root_tcb_frame.addr()) as *mut TCB) };
    *tcb = TCB::new();

    // 6. 配置 TCB (绑定资源)
    // 创建指向刚才分配的 CNode 和 PageTable 的 Capability
    let cap_cspace = Capability::create_cnode(root_cspace_frame.addr(), 12, rights::ALL);
    let cap_vspace = Capability::create_pagetable(
        root_vspace_frame.addr(),
        addr::phys_to_virt(root_vspace_frame.addr()),
        2,
        rights::ALL,
    );

    tcb.configure(
        cap_cspace,
        cap_vspace,
        root_utcb_frame,
        utcb_base,
        None, // Root Task 暂时没有 Fault Handler，或者指向内核默认处理
    );

    // 7. 设置初始寄存器
    tcb.set_registers(entry_point, stack_top);

    // 8. 激活线程
    tcb.state = ThreadState::Ready;
    scheduler::add_thread(tcb);

    printk!("Root Task created. Entry: {:#x}, SP: {:#x}\n", entry_point, stack_top);
}

/// 填充 Root CNode
/// 将所有空闲物理内存作为 Untyped Capability 授予 Root Task
fn populate_root_cnode(cnode: &mut CNode) {
    let free_regions = pmem::get_untyped_regions();
    let mut slot = 1; // Slot 0 通常保留

    for region in free_regions {
        let cap = Capability::create_untyped(region.start, region.end - region.start, rights::ALL);
        cnode.insert(slot, cap);
        slot += 1;
    }

    // TODO: 还需要插入设备内存 (MMIO) 和 IRQ Capability
    // ...
    unimplemented!()
}

pub fn current() -> &'static mut TCB {
    let hart = hart::get();
    let tcb_ptr = hart.proc;
    unsafe { &mut *tcb_ptr }
}
