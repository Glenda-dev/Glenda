use crate::ipc::MsgArgs;
use crate::proc::scheduler;
use crate::trap::TrapCause;
use crate::trap::syscall;
use core::arch::{asm, naked_asm};

const KERNEL_CS: u64 = 0x08;
const KERNEL_DS: u64 = 0x10;
const USER_DS: u64 = 0x1b;
const USER_CS: u64 = 0x23;
const TSS_SELECTOR: u16 = 0x28;
const INT_SYSCALL: usize = 0x80;

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_lo: u16,
    selector: u16,
    ist: u8,
    attrs: u8,
    offset_mid: u16,
    offset_hi: u32,
    zero: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            offset_lo: 0,
            selector: 0,
            ist: 0,
            attrs: 0,
            offset_mid: 0,
            offset_hi: 0,
            zero: 0,
        }
    }

    fn new(handler: unsafe extern "C" fn(), dpl: u8) -> Self {
        let addr = handler as usize as u64;
        Self {
            offset_lo: addr as u16,
            selector: KERNEL_CS as u16,
            ist: 0,
            attrs: 0x8e | ((dpl & 0x3) << 5),
            offset_mid: (addr >> 16) as u16,
            offset_hi: (addr >> 32) as u32,
            zero: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InterruptFrame {
    rax: usize,
    rbx: usize,
    rcx: usize,
    rdx: usize,
    rbp: usize,
    rsi: usize,
    rdi: usize,
    r8: usize,
    r9: usize,
    r10: usize,
    r11: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    rip: usize,
    cs: usize,
    rflags: usize,
    rsp: usize,
    ss: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TrapFrame {
    pub kernel_rsp: usize,
    pub user_rip: usize,
    pub user_rsp: usize,
    pub rflags: usize,
    pub user_cs: usize,
    pub user_ss: usize,
    pub fsbase: usize,
    pub rax: usize,
    pub rbx: usize,
    pub rcx: usize,
    pub rdx: usize,
    pub rbp: usize,
    pub rsi: usize,
    pub rdi: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
}

#[repr(C, align(16))]
struct Gdt([u64; 8]);

static mut GDT: Gdt = Gdt([
    0,
    0x00af9a000000ffff,
    0x00af92000000ffff,
    0x00aff2000000ffff,
    0x00affa000000ffff,
    0,
    0,
    0,
]);

static mut TSS: Tss = Tss {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    iomap_base: core::mem::size_of::<Tss>() as u16,
};

static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

impl TrapFrame {
    pub const fn new() -> Self {
        Self {
            kernel_rsp: 0,
            user_rip: 0,
            user_rsp: 0,
            rflags: 0x202,
            user_cs: USER_CS as usize,
            user_ss: USER_DS as usize,
            fsbase: 0,
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rbp: 0,
            rsi: 0,
            rdi: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }

    pub const fn get_epc(&self) -> usize {
        self.user_rip
    }

    pub fn set_epc(&mut self, epc: usize) {
        self.user_rip = epc;
    }

    pub fn set_cpuid(&mut self, _id: usize) {}

    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize, thread_pointer: usize) {
        self.user_rip = entry_point;
        self.user_rsp = stack_pointer;
        self.fsbase = thread_pointer;
        self.user_cs = USER_CS as usize;
        self.user_ss = USER_DS as usize;
        self.rflags = 0x202;
    }

    pub fn configure_kernel(
        &mut self,
        _mmu: usize,
        _hartid: usize,
        kstack_top: usize,
        _kernel_vec: usize,
    ) {
        self.kernel_rsp = kstack_top;
    }

    pub fn set_return_value(&mut self, value: usize) {
        self.rax = value;
    }

    pub fn set_fast_ipc_hint(&mut self, _enabled: bool) {}

    pub const fn get_syscall_args(&self) -> (isize, usize) {
        (self.rax as isize, self.rdi)
    }

    pub const fn get_syscall_ipc_args(&self) -> (usize, usize, [usize; 4]) {
        (self.rsi, self.rdx, [self.r10, self.r8, self.r9, self.r11])
    }

    pub fn set_syscall_ipc_ret(&mut self, msgtag: usize, badge: usize, mrs: [usize; 4]) {
        self.rsi = msgtag;
        self.rdx = badge;
        self.r10 = mrs[0];
        self.r8 = mrs[1];
        self.r9 = mrs[2];
        self.r11 = mrs[3];
    }

    pub fn advance_pc(&mut self) {}

    pub const fn get_ra(&self) -> usize {
        self.rbx
    }

    pub fn set_ra(&mut self, ra: usize) {
        self.rbx = ra;
    }

    pub const fn get_sp(&self) -> usize {
        self.user_rsp
    }

    pub fn get_registers(&self) -> MsgArgs {
        [
            self.rax,
            self.rbx,
            self.rcx,
            self.rdx,
            self.rsi,
            self.rdi,
            self.r8,
            self.r9,
        ]
    }

    pub fn get_syscall_registers(&self) -> MsgArgs {
        self.get_registers()
    }

    pub fn set_registers(&mut self, regs: &MsgArgs) {
        self.rax = regs[0];
        self.rbx = regs[1];
        self.rcx = regs[2];
        self.rdx = regs[3];
        self.rsi = regs[4];
        self.rdi = regs[5];
        self.r8 = regs[6];
        self.r9 = regs[7];
    }

    pub fn for_each_register<F>(&self, mut f: F)
    where
        F: FnMut(usize),
    {
        for reg in [
            self.rax, self.rbx, self.rcx, self.rdx, self.rsi, self.rdi, self.r8, self.r9,
            self.r10, self.r11, self.r12, self.r13, self.r14, self.r15,
        ] {
            f(reg);
        }
        f(self.user_rip);
        f(self.user_rsp);
        f(self.rflags);
    }
}

pub unsafe fn vector_init() {
    unsafe {
        init_gdt();
        IDT[INT_SYSCALL] = IdtEntry::new(kernel_vector_thunk, 3);
        let idtr = DescriptorTablePointer {
            limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };
        asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack));
    }
}

pub fn match_cause(_cause: usize) -> TrapCause {
    TrapCause::Unknown(_cause)
}

pub fn get_cause() -> usize {
    0
}

pub fn get_pc() -> usize {
    0
}

pub fn get_value() -> usize {
    0
}

pub fn get_status() -> usize {
    0
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector() {
    naked_asm!(
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rdi",
        "push rsi",
        "push rbp",
        "push rdx",
        "push rcx",
        "push rbx",
        "push rax",
        "mov rdi, rsp",
        "call {dispatch}",
        dispatch = sym x86_syscall_dispatch,
    );
}

#[unsafe(naked)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector() {
    naked_asm!("ud2");
}

pub extern "C" fn trap_user_handler() {
    if let Some(ptr) = scheduler::current() {
        let tcb = unsafe { &mut *ptr };
        let tf = tcb.get_tf();
        let (method, cptr) = tf.get_syscall_args();
        let ret = syscall::dispatch(cptr, method as usize);
        tf.set_return_value(ret);
        trap_user_return();
    }
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_return(_trapframe: usize, _satp: usize) {
    trap_user_return();
}

pub fn trap_user_return() {
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };
    let tf = tcb.get_tf_ref();
    unsafe {
        TSS.rsp0 = tcb.get_kstack_top().as_usize() as u64;
        asm!(
            "mov rdx, {tf}",
            "mov rax, [rdx + {fsbase}]",
            "wrfsbase rax",
            "push QWORD PTR [rdx + {user_ss}]",
            "push QWORD PTR [rdx + {user_rsp}]",
            "push QWORD PTR [rdx + {rflags}]",
            "push QWORD PTR [rdx + {user_cs}]",
            "push QWORD PTR [rdx + {user_rip}]",
            "mov r15, [rdx + {r15}]",
            "mov r14, [rdx + {r14}]",
            "mov r13, [rdx + {r13}]",
            "mov r12, [rdx + {r12}]",
            "mov r11, [rdx + {r11}]",
            "mov r10, [rdx + {r10}]",
            "mov r9, [rdx + {r9}]",
            "mov r8, [rdx + {r8}]",
            "mov rdi, [rdx + {rdi}]",
            "mov rsi, [rdx + {rsi}]",
            "mov rbp, [rdx + {rbp}]",
            "mov rcx, [rdx + {rcx}]",
            "mov rbx, [rdx + {rbx}]",
            "mov rax, [rdx + {rax}]",
            "mov rdx, [rdx + {rdx_off}]",
            "iretq",
            tf = in(reg) tf as *const TrapFrame,
            fsbase = const offset_of!(TrapFrame, fsbase),
            user_ss = const offset_of!(TrapFrame, user_ss),
            user_rsp = const offset_of!(TrapFrame, user_rsp),
            rflags = const offset_of!(TrapFrame, rflags),
            user_cs = const offset_of!(TrapFrame, user_cs),
            user_rip = const offset_of!(TrapFrame, user_rip),
            rax = const offset_of!(TrapFrame, rax),
            rbx = const offset_of!(TrapFrame, rbx),
            rcx = const offset_of!(TrapFrame, rcx),
            rdx_off = const offset_of!(TrapFrame, rdx),
            rbp = const offset_of!(TrapFrame, rbp),
            rsi = const offset_of!(TrapFrame, rsi),
            rdi = const offset_of!(TrapFrame, rdi),
            r8 = const offset_of!(TrapFrame, r8),
            r9 = const offset_of!(TrapFrame, r9),
            r10 = const offset_of!(TrapFrame, r10),
            r11 = const offset_of!(TrapFrame, r11),
            r12 = const offset_of!(TrapFrame, r12),
            r13 = const offset_of!(TrapFrame, r13),
            r14 = const offset_of!(TrapFrame, r14),
            r15 = const offset_of!(TrapFrame, r15),
            options(noreturn)
        );
    }
}

pub fn is_user_mode(status: usize) -> bool {
    (status & 0x3) == 0x3
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kernel_vector_thunk() {
    unsafe { kernel_vector() }
}

unsafe fn init_gdt() {
    let tss_base = core::ptr::addr_of!(TSS) as u64;
    let tss_limit = (core::mem::size_of::<Tss>() - 1) as u64;
    let low = (tss_limit & 0xffff)
        | ((tss_base & 0x00ff_ffff) << 16)
        | (0x89u64 << 40)
        | (((tss_limit >> 16) & 0xf) << 48)
        | (((tss_base >> 24) & 0xff) << 56);
    let high = tss_base >> 32;
    GDT.0[5] = low;
    GDT.0[6] = high;

    let gdtr = DescriptorTablePointer {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };
    asm!("lgdt [{}]", in(reg) &gdtr, options(readonly, nostack));
    asm!(
        "push {kcode}",
        "lea rax, [rip + 2f]",
        "push rax",
        "lretq",
        "2:",
        "mov ax, {kdata}",
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "mov fs, ax",
        "mov gs, ax",
        "ltr {tss:x}",
        kcode = const KERNEL_CS,
        kdata = const KERNEL_DS,
        tss = in(reg) TSS_SELECTOR,
        out("rax") _,
        options(nostack)
    );
}

unsafe extern "C" fn x86_syscall_dispatch(frame: *const InterruptFrame) -> ! {
    let raw = unsafe { &*frame };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let tf = tcb.get_tf();
    tf.rax = raw.rax;
    tf.rbx = raw.rbx;
    tf.rcx = raw.rcx;
    tf.rdx = raw.rdx;
    tf.rbp = raw.rbp;
    tf.rsi = raw.rsi;
    tf.rdi = raw.rdi;
    tf.r8 = raw.r8;
    tf.r9 = raw.r9;
    tf.r10 = raw.r10;
    tf.r11 = raw.r11;
    tf.r12 = raw.r12;
    tf.r13 = raw.r13;
    tf.r14 = raw.r14;
    tf.r15 = raw.r15;
    tf.user_rip = raw.rip;
    tf.user_cs = raw.cs;
    tf.rflags = raw.rflags;
    tf.user_rsp = raw.rsp;
    tf.user_ss = raw.ss;
    trap_user_handler();
}
