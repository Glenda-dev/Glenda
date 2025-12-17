use super::ProcState;
use super::context::ProcContext;
use crate::cap::{CSpace, Capability};
use crate::irq::TrapFrame;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::{KernelStack, PageTable, PhysFrame, VSpace, VirtAddr};
use crate::proc::process::Pid;

#[repr(C)]
pub struct TCB {
    pub state: ProcState,               // 进程状态
    pub pid: Pid,                       // 进程ID
    pub vspace: VSpace,                 // 进程虚拟地址空间
    pub cspace: CSpace,                 // 进程能力空间
    pub trapframe: Option<PhysFrame>,   // RAII frame
    pub kstack: Option<KernelStack>,    // 内核栈 RAII
    pub trapframe_va: VirtAddr,         // TrapFrame 的用户可见虚拟地址
    pub context: ProcContext,           // 用户态上下文
    pub entry_va: VirtAddr,             // 用户入口地址
    pub user_sp_va: VirtAddr,           // 用户栈顶 VA
    pub stack_pages: usize,             // 用户栈页数
    pub utcb_frame: Option<PhysFrame>,  // UTCB 的物理页框
    pub utcb_va: VirtAddr,              // UTCB 虚拟地址
    pub irqhandler: Option<Capability>, // IRQ 处理能力
}

impl TCB {
    pub const fn new() -> Self {
        Self {
            state: ProcState::Unused,
            pid: 0,
            vspace: VSpace::new(),
            cspace: CSpace::new(),
            trapframe: None,
            trapframe_va: 0,
            kstack: None,
            context: ProcContext::new(),
            entry_va: 0,
            user_sp_va: 0,
            stack_pages: 0,
            utcb_frame: None,
            utcb_va: 0,
            irqhandler: None,
        }
    }
}
