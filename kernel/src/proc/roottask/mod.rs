mod bootinfo;
mod init;
mod initrd;
mod layout;

pub use initrd::cat_file;
pub use initrd::print_files;
pub use layout::STACK_VA;

use super::scheduler;
use super::{TCB, ThreadState};
use crate::cap::CNode;
use crate::mem::PageTable;
use crate::printk;
use bootinfo::BootInfo;
use init::*;
use layout::*;

use initrd::ProcPayload;

/// 初始化进程子系统
pub fn init() {
    initrd::init();
}

/// 创建 Root Task
pub fn spawn(name: &str) {
    if let Some(task) = initrd::find(name) {
        spawn_payload(task);
    } else {
        panic!("proc: Root task '{}' not found", name);
    }
}

pub fn spawn_first() {
    if let Some(task) = initrd::get_by_index(0) {
        let len = task.metadata.name.iter().position(|&c| c == 0).unwrap_or(32);
        let name = core::str::from_utf8(&task.metadata.name[..len]).unwrap_or("unknown");
        printk!("proc: Spawning default root task '{}'\n", name);
        spawn_payload(task);
    } else {
        panic!("proc: No root task found in initrd");
    }
}

fn spawn_payload(root_task: ProcPayload) {
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
